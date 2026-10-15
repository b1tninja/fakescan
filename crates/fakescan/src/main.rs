//! fakescan binary: stable lab identity + control plane + optional WSD discovery.
//!
//! Start-up order: parse args (CLI > env `FAKESCAN_NAME` > registry row > compiled defaults),
//! then hand the resolved [`ServerConfig`] to [`fakescan::server::serve`], which holds the
//! instance lock, loads the profile, builds the router, starts discovery and serves.

use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use fakescan::lab;
use fakescan::registry::{registry_path, Registry};
use fakescan::server::{serve, DeviceConfig, ServerConfig};
use uuid::Uuid;

#[derive(Debug, Parser)]
#[command(name = "fakescan", about = "Fake scanner for WIA / WSD testing")]
struct Args {
    #[command(subcommand)]
    command: Option<Cmd>,

    /// Daemon mode: with no --device flags, bring up every registry row (devices.json, or
    /// $FAKESCAN_REGISTRY) whose `listen` equals this process's listen, with its stored UUID and
    /// profile, and persist install tracking. Add/remove scanners at runtime via POST /v1/devices.
    #[arg(long)]
    manage: bool,

    /// Listen address for the control API and WSD Scan Service stub.
    ///
    /// Default 0.0.0.0:3920 (or the registry row's listen when --lab-name is set).
    #[arg(long)]
    listen: Option<SocketAddr>,

    /// Advertise this host in WS-Discovery XAddrs (defaults to listen IP, or registry row).
    /// Use a LAN address (not 127.0.0.1) so another PC can discover and install this scanner.
    #[arg(long)]
    advertise: Option<IpAddr>,

    /// Disable WS-Discovery / Scan Service stub (control API only).
    #[arg(long)]
    no_wsd: bool,

    /// Load N sheets into every device's feeder at start and on reset (devices that set their
    /// own `initial_pages` keep it; clamped to tray capacity). Default: each profile's
    /// `feeder.initial_pages`, which is 0 (empty) in every shipped profile.
    #[arg(long, value_name = "N")]
    initial_pages: Option<u32>,

    /// Directory of capability profiles (JSON).
    #[arg(long, default_value = "profiles")]
    profiles: PathBuf,

    /// Profile id to load at start (`brother-usb`, `brother-wsd`, …).
    #[arg(long, default_value = "brother-wsd")]
    profile: String,

    /// Stable device UUID for WS-Discovery / PnP association.
    ///
    /// Default: `LAB_DEVICE_UUID` (00000000-0000-4000-8000-0000000000f2).
    /// Override to run a parallel instance with a distinct UUID.
    #[arg(long)]
    device_uuid: Option<Uuid>,

    /// Lab-safety override: accept device UUIDs outside the fake namespace
    /// (`00000000-0000-4000-8000-*`). Without it such a UUID is refused at start.
    #[arg(long)]
    allow_foreign_uuid: bool,

    /// Lab instance name — resolves listen/uuid/advertise from devices.json.
    /// Env: `FAKESCAN_NAME`.  Required for `POST /v1/ctl save`.
    #[arg(long, env = "FAKESCAN_NAME")]
    lab_name: Option<String>,

    /// Extra emulated scanner, repeatable: `ID=PROFILE[@UUID]` (served under `/d/ID/...`).
    /// Without `@UUID` a stable UUID is derived from the id.
    #[arg(long = "device", value_name = "ID=PROFILE[@UUID]")]
    devices: Vec<String>,

    /// Bind UDP :3702 to answer WS-Discovery Probes and Resolves.
    /// Default: on everywhere. `dasHost` shares the port; a Hello-only daemon is never found by
    /// Windows (its Probe/Resolve for our UUID go unanswered, so an installed device stays
    /// offline). Use `--no-probe-listen` to opt out.
    #[arg(long, default_value_t = true)]
    probe_listen: bool,

    /// Force Hello-only mode (never bind :3702), even on Linux.
    #[arg(long)]
    no_probe_listen: bool,

    /// Do not show the system-tray icon (Windows). The tray is also skipped for `--no-wsd`
    /// control-only runs and for runs with neither `--manage` nor `--lab-name`.
    #[arg(long)]
    no_tray: bool,

    /// Verbose logs (`fakescan=debug,tower_http=debug`). `RUST_LOG` still wins.
    #[arg(short = 'v', long)]
    verbose: bool,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Manage the per-user logon task that runs `fakescan --manage` (not a Windows service).
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
}

#[derive(Debug, Subcommand)]
enum DaemonAction {
    /// Create the logon task (uses --profiles, --listen, --advertise given on this command line).
    Install,
    /// Delete the logon task.
    Uninstall,
    /// Show the logon task.
    Status,
}

const STARTUP_BANNER: &str = r#"
   .-----------------------------.     fakescan
  |  _________________________  |     lab fake WSD scanner
  | | ####  PAGE ON GLASS  ## | |     not malware — test harness
  | | ======================= | |     sibling of fakeprint
  | |__________ Fs ___________| |
   '-----------------------------'
  https://github.com/b1tninja/fakescan
"#;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if let Some(Cmd::Daemon { action }) = &args.command {
        let out = match action {
            DaemonAction::Install => fakescan::daemon::install(
                &args.profiles,
                args.listen.map(|l| l.to_string()).as_deref(),
                args.advertise.map(|a| a.to_string()).as_deref(),
            )?,
            DaemonAction::Uninstall => fakescan::daemon::uninstall()?,
            DaemonAction::Status => fakescan::daemon::status()?,
        };
        println!("{out}");
        return Ok(());
    }
    // Print before tracing so a person opening this console sees the lab mark,
    // not a blank elevated-looking process with only log lines.
    eprintln!("{STARTUP_BANNER}");
    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        if args.verbose {
            "fakescan=debug,tower_http=debug".into()
        } else {
            "fakescan=info,tower_http=info".into()
        }
    });
    tracing_subscriber::fmt().with_env_filter(filter).init();

    // Priority: CLI > registry row (via --lab-name) > compiled defaults.
    let lab_name = args.lab_name.clone();
    let reg_row = lab_name
        .as_deref()
        .and_then(|name| Registry::load().ok().and_then(|r| r.find(name).cloned()));

    let listen: SocketAddr = args.listen.unwrap_or_else(|| {
        reg_row
            .as_ref()
            .and_then(|r| r.listen.parse().ok())
            .unwrap_or_else(lab::default_listen)
    });
    let device_uuid: Uuid = args.device_uuid.unwrap_or_else(|| {
        reg_row
            .as_ref()
            .and_then(|r| Uuid::parse_str(&r.device_uuid).ok())
            .unwrap_or_else(lab::lab_device_uuid)
    });
    let manage_rows = if args.manage {
        Registry::load()
            .map(|r| {
                r.rows_for_listen(&listen.to_string())
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let advertise: Option<IpAddr> = args.advertise.or_else(|| {
        reg_row
            .as_ref()
            .or(manage_rows.first())
            .filter(|r| !r.advertise.is_empty())
            .and_then(|r| r.advertise.parse().ok())
    });

    // --manage with no --device flags: the registry rows are the fleet (loaded by `serve`).
    let from_registry = args.manage && args.devices.is_empty();
    let mut devices = Vec::new();
    if !from_registry {
        devices.push(DeviceConfig::new("default", args.profile, device_uuid));
        for spec in &args.devices {
            devices.push(parse_device(spec)?);
        }
        for d in &mut devices {
            d.allow_foreign_uuid = args.allow_foreign_uuid;
        }
    }
    // The registry is attached in daemon-style runs only (--manage or --lab-name).
    let registry = (args.manage || lab_name.is_some()).then(registry_path);

    // Tray (Windows, feature `tray`): daemon-style runs only (--manage or --lab-name), never for
    // control-only --no-wsd runs. The embedded library path (server::serve called directly) never
    // starts one: this is main.rs only.
    let want_tray = !args.no_tray && !args.no_wsd && (args.manage || lab_name.is_some());
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<fakescan::server::Ready>();
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    #[cfg(all(windows, feature = "tray"))]
    let tray_task = want_tray.then(|| {
        tokio::spawn(async move {
            let Ok(ready) = ready_rx.await else {
                return None;
            };
            let stop_tx = std::sync::Mutex::new(Some(stop_tx));
            fakescan::tray::spawn(fakescan::tray::TrayConfig {
                port: ready.local_addr.port(),
                quit: std::sync::Arc::new(move || {
                    if let Some(tx) = stop_tx.lock().ok().and_then(|mut g| g.take()) {
                        let _ = tx.send(());
                    }
                }),
            })
        })
    });
    #[cfg(not(all(windows, feature = "tray")))]
    let (ready_rx, stop_tx) = {
        let _ = want_tray;
        (ready_rx, stop_tx)
    };

    let served = serve(
        ServerConfig {
            listen,
            advertise,
            wsd: !args.no_wsd,
            discovery: !args.no_wsd,
            profiles: args.profiles,
            devices,
            lab_name,
            probe_listen: args.probe_listen && !args.no_probe_listen,
            instance_lock: true,
            named_pipe: true,
            ctrl_c: true,
            registry,
            manage: args.manage,
            allow_foreign_uuid: args.allow_foreign_uuid,
            initial_pages: args.initial_pages,
        },
        Some(ready_tx),
        Some(stop_rx),
    )
    .await;

    #[cfg(all(windows, feature = "tray"))]
    if let Some(task) = tray_task {
        // Remove the icon before the process exits (a killed tray leaves a ghost icon).
        task.abort();
        if let Ok(Some(mut h)) = task.await {
            h.stop();
        }
    }
    #[cfg(not(all(windows, feature = "tray")))]
    let _ = (ready_rx, stop_tx);
    served
}

/// Parse `ID=PROFILE[@UUID]`.
fn parse_device(spec: &str) -> anyhow::Result<DeviceConfig> {
    let (id, rest) = spec
        .split_once('=')
        .ok_or_else(|| anyhow::anyhow!("--device {spec:?}: expected ID=PROFILE[@UUID]"))?;
    let (profile, uuid) = match rest.split_once('@') {
        Some((p, u)) => (p, Some(Uuid::parse_str(u)?)),
        None => (rest, None),
    };
    anyhow::ensure!(
        !id.is_empty() && !profile.is_empty(),
        "--device {spec:?}: empty id or profile"
    );
    Ok(match uuid {
        Some(u) => DeviceConfig::new(id, profile, u),
        None => DeviceConfig::derived(id, profile),
    })
}
