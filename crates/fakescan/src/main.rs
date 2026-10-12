//! fakescan binary: stable lab identity + control plane + optional WSD discovery.
//!
//! ## Start-up order
//!
//! 1. Parse args (CLI > env `FAKESCAN_NAME` > registry row > compiled defaults).
//! 2. Acquire the per-port instance lock (fail fast if already running).
//! 3. Load profile and build World state.
//! 4. Build axum router (existing `/v1/*` + new `/v1/ctl`).
//! 5. Start optional WSD discovery task.
//! 6. Start Windows named-pipe server task.
//! 7. Bind and serve; log the attach hint on success.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{ConnectInfo, Query, State};
use axum::http::Request;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use clap::Parser;
use fakescan::ctl::{get_ctl, post_ctl};
use fakescan::discovery::DiscoveryConfig;
use fakescan::effects::EffectsBody;
use fakescan::lab;
use fakescan::profile::Profile;
use fakescan::registry::Registry;
use fakescan::scenarios;
use fakescan::state::{
    FaultBody, FeederBody, JpegComBody, LoopbackBody, LoopbackStatus, PagesBody, PatternBody,
    ScenarioBody, SlowBody, Status, World,
};
use fakescan::{discovery, wsd};
use serde::Deserialize;
use tokio::sync::RwLock;
use tower_http::trace::TraceLayer;
use tracing::{info, info_span};
use uuid::Uuid;

#[derive(Debug, Parser)]
#[command(name = "fakescan", about = "Fake scanner for WIA / WSD testing")]
struct Args {
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

    /// Lab instance name — resolves listen/uuid/advertise from devices.json.
    /// Env: `FAKESCAN_NAME`.  Required for `POST /v1/ctl save`.
    #[arg(long, env = "FAKESCAN_NAME")]
    lab_name: Option<String>,

    /// Bind UDP :3702 to answer WS-Discovery Probes.
    /// Default: on for non-Windows, off on Windows (`dasHost` owns the port).
    #[arg(long, default_value_t = !cfg!(windows))]
    probe_listen: bool,

    /// Force Hello-only mode (never bind :3702), even on Linux.
    #[arg(long)]
    no_probe_listen: bool,

    /// Verbose logs (`fakescan=debug,tower_http=debug`). `RUST_LOG` still wins.
    #[arg(short = 'v', long)]
    verbose: bool,
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

    // ── 1. Resolve listen / device_uuid / advertise ───────────────────────────
    // Priority: CLI > registry row (via --lab-name) > compiled defaults.

    let lab_name = args.lab_name.clone();

    // Load the registry row once (if a name is given) to fill missing CLI values.
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

    let advertise_ip_override: Option<IpAddr> = args.advertise.or_else(|| {
        reg_row
            .as_ref()
            .filter(|r| !r.advertise.is_empty())
            .and_then(|r| r.advertise.parse().ok())
    });

    // ── 2. Single-instance lock ───────────────────────────────────────────────
    // Stale locks (dead PIDs) are reclaimed automatically.
    let _lock = lab::acquire_instance_lock(listen.port())?;

    // ── 3. Profile + World ────────────────────────────────────────────────────
    let wsd_on = !args.no_wsd;
    let profile = Profile::load(&args.profiles, &args.profile)?;
    info!(id = %profile.id, "loaded profile");

    let discovery_stats = Arc::new(fakescan::discovery::DiscoveryStats::default());
    let mut world_inner =
        World::with_discovery(args.profiles.clone(), profile, Arc::clone(&discovery_stats));

    let mut endpoint = None;
    if wsd_on {
        let adv_ip = advertise_ip_override.unwrap_or_else(|| listen.ip());
        let adv_ip = if adv_ip.is_unspecified() {
            IpAddr::V4(Ipv4Addr::LOCALHOST)
        } else {
            adv_ip
        };
        let device_ep = format!("http://{}:{}/WebServices/Device", adv_ip, listen.port());
        let scan_ep = format!("http://{}:{}/ScannerService", adv_ip, listen.port());
        world_inner.set_identity(device_ep.clone(), scan_ep, device_uuid);
        endpoint = Some(device_ep);
    }

    // Record lab identity so /v1/ctl and save know who we are.
    world_inner.set_lab_identity(lab_name.clone(), listen);
    let (exit_tx, exit_rx) = tokio::sync::watch::channel(false);
    world_inner.arm_exit(exit_tx);

    let world = Arc::new(RwLock::new(world_inner));
    let listen_probes = args.probe_listen && !args.no_probe_listen;

    // ── 4. Router ─────────────────────────────────────────────────────────────
    let mut app = Router::new()
        .route("/health", get(health))
        // Unified ctl surface (new).
        .route("/v1/ctl", get(get_ctl).post(post_ctl))
        // Existing fine-grained routes (kept for backward compat).
        .route("/v1/status", get(status))
        .route("/v1/jobs", get(jobs))
        .route("/v1/activity", get(trace))
        .route("/v1/trace", get(trace))
        .route("/v1/log", get(activity_log))
        .route("/v1/profile", post(set_profile))
        .route("/v1/feeder", post(set_feeder))
        .route("/v1/pages", post(set_pages))
        .route("/v1/slow", post(set_slow))
        .route("/v1/pattern", post(set_pattern))
        .route("/v1/jpeg-com", post(set_jpeg_com))
        .route("/v1/effects", post(set_effects))
        .route("/v1/scenarios", get(list_scenarios))
        .route("/v1/scenario", post(set_scenario))
        .route("/v1/fault", post(set_fault))
        .route("/v1/faults", post(set_fault))
        .route("/v1/condition", post(set_fault))
        .route(
            "/v1/loopback",
            get(get_loopback).post(set_loopback).delete(clear_loopback),
        )
        .route("/v1/reset", post(reset));

    if wsd_on {
        app = app
            .route("/WebServices/Device", post(wsd::scanner_service))
            .route("/WebServices/Device/", post(wsd::scanner_service))
            .route("/ScannerService", post(wsd::scanner_service))
            .route("/ScannerService/", post(wsd::scanner_service))
            .route("/WebServices/ScannerService", post(wsd::scanner_service))
            .route("/WebServices/ScannerService/", post(wsd::scanner_service));
    }

    let app = app
        .layer(
            TraceLayer::new_for_http().make_span_with(|req: &Request<_>| {
                let peer = req
                    .extensions()
                    .get::<ConnectInfo<SocketAddr>>()
                    .map(|ConnectInfo(a)| a.to_string())
                    .unwrap_or_else(|| "-".into());
                info_span!(
                    "http",
                    method = %req.method(),
                    uri = %req.uri(),
                    peer = %peer,
                )
            }),
        )
        .with_state(world.clone());

    // ── 5. WSD discovery task ─────────────────────────────────────────────────
    let mut discovery_task = None;
    let mut discovery_stop = None;
    if let Some(ep) = endpoint {
        let multicast_iface = match advertise_ip_override.unwrap_or_else(|| listen.ip()) {
            IpAddr::V4(v4) if !v4.is_unspecified() => v4,
            _ => Ipv4Addr::UNSPECIFIED,
        };
        // Resolve before spawn so GET /v1/status sees nonzero ids immediately
        // (A/B restart without waiting for the first Hello).
        let instance_id = fakescan::discovery::boot_sequence_id();
        let metadata_version = instance_id;
        discovery_stats.set_sequence(instance_id, metadata_version);
        let cfg = DiscoveryConfig {
            endpoint: ep.clone(),
            device_uuid,
            friendly_name: "fakescan".into(),
            multicast_iface,
            listen_probes,
            stats: discovery_stats,
            instance_id,
            metadata_version,
        };
        info!(
            %ep,
            %multicast_iface,
            listen_probes,
            "WSD Scan Service + WS-Discovery enabled"
        );
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();
        discovery_stop = Some(stop_tx);
        discovery_task = Some(tokio::spawn(async move {
            if let Err(e) = discovery::run(cfg, stop_rx).await {
                tracing::error!(error = %e, "WS-Discovery stopped");
            }
        }));
    }

    // ── 6. Named pipe task (Windows) ──────────────────────────────────────────
    {
        let pipe_world = Arc::clone(&world);
        let pipe_name = lab_name.clone().unwrap_or_else(|| "default".to_string());
        tokio::spawn(async move {
            fakescan::pipe::serve(&pipe_name, pipe_world).await;
        });
    }

    // ── 7. Bind and serve ─────────────────────────────────────────────────────
    let listener = lab::bind_listener(listen).await?;
    info!(
        %listen,
        uuid = %device_uuid,
        lab_name = ?lab_name,
        verbose = args.verbose,
        "fakescan listening — attach: http://127.0.0.1:{}/v1/ctl", listen.port()
    );

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        shutdown_signal(exit_rx).await;
        if let Some(tx) = discovery_stop {
            let _ = tx.send(());
        }
    })
    .await?;

    // Let discovery broadcast a WSD Bye before exit.
    if let Some(task) = discovery_task {
        match tokio::time::timeout(std::time::Duration::from_millis(1500), task).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => tracing::warn!(error = %e, "discovery task join failed"),
            Err(_) => tracing::warn!("discovery Bye timed out"),
        }
    }
    Ok(())
}

// ── HTTP handlers (unchanged from original) ───────────────────────────────────

async fn health() -> &'static str {
    "ok"
}

async fn status(State(world): State<Arc<RwLock<World>>>) -> Json<Status> {
    Json(world.read().await.status())
}

#[derive(Debug, Deserialize)]
struct TraceQuery {
    /// Return events with `seq` greater than this. Omit for the whole ring.
    /// A string so `?after=5.0` and `?after=` do not 400 the probe.
    after: Option<String>,
}

fn query_after(after: Option<&str>) -> u64 {
    after.map(fakescan::state::parse_cursor).unwrap_or(0)
}

async fn trace(
    State(world): State<Arc<RwLock<World>>>,
    Query(query): Query<TraceQuery>,
) -> Json<fakescan::state::TraceView> {
    Json(
        world
            .read()
            .await
            .trace_since(query_after(query.after.as_deref())),
    )
}

async fn jobs(State(world): State<Arc<RwLock<World>>>) -> Json<fakescan::state::JobsView> {
    Json(world.read().await.jobs_view())
}

async fn activity_log(
    State(world): State<Arc<RwLock<World>>>,
    Query(query): Query<TraceQuery>,
) -> axum::response::Response {
    let text = world
        .read()
        .await
        .log_since(query_after(query.after.as_deref()));
    (
        axum::http::StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; charset=utf-8",
        )],
        text,
    )
        .into_response()
}

#[derive(Debug, Deserialize)]
struct ProfileBody {
    id: String,
}

async fn set_profile(
    State(world): State<Arc<RwLock<World>>>,
    Json(body): Json<ProfileBody>,
) -> Result<Json<Status>, (axum::http::StatusCode, String)> {
    let mut w = world.write().await;
    w.load_profile(&body.id)
        .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e.to_string()))?;
    wsd::job_events::maybe_emit_elements_change(&w, wsd::soap::SCAN_NS_AIRSCAN, w.profile());
    Ok(Json(w.status()))
}

async fn set_feeder(
    State(world): State<Arc<RwLock<World>>>,
    Json(body): Json<FeederBody>,
) -> Json<Status> {
    let mut w = world.write().await;
    w.set_feeder(body.ready);
    wsd::job_events::maybe_emit_elements_change(&w, wsd::soap::SCAN_NS_AIRSCAN, w.profile());
    Json(w.status())
}

async fn set_pages(
    State(world): State<Arc<RwLock<World>>>,
    Json(body): Json<PagesBody>,
) -> Result<Json<Status>, (axum::http::StatusCode, String)> {
    let mut w = world.write().await;
    w.apply_pages(&body)
        .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e.to_string()))?;
    wsd::job_events::maybe_emit_elements_change(&w, wsd::soap::SCAN_NS_AIRSCAN, w.profile());
    Ok(Json(w.status()))
}

async fn set_slow(
    State(world): State<Arc<RwLock<World>>>,
    Json(body): Json<SlowBody>,
) -> Json<Status> {
    let mut w = world.write().await;
    w.apply_slow(&body);
    Json(w.status())
}

async fn set_pattern(
    State(world): State<Arc<RwLock<World>>>,
    Json(body): Json<PatternBody>,
) -> Result<Json<Status>, (axum::http::StatusCode, String)> {
    let mut w = world.write().await;
    w.set_image_pattern_id(&body.id)
        .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e.to_string()))?;
    wsd::job_events::maybe_emit_elements_change(&w, wsd::soap::SCAN_NS_AIRSCAN, w.profile());
    Ok(Json(w.status()))
}

async fn set_jpeg_com(
    State(world): State<Arc<RwLock<World>>>,
    Json(body): Json<JpegComBody>,
) -> Json<Status> {
    let mut w = world.write().await;
    w.set_jpeg_com(body.enabled);
    Json(w.status())
}

async fn set_effects(
    State(world): State<Arc<RwLock<World>>>,
    Json(body): Json<EffectsBody>,
) -> Result<Json<Status>, (axum::http::StatusCode, String)> {
    let mut w = world.write().await;
    w.apply_effects(&body)
        .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e.to_string()))?;
    wsd::job_events::maybe_emit_elements_change(&w, wsd::soap::SCAN_NS_AIRSCAN, w.profile());
    Ok(Json(w.status()))
}

async fn list_scenarios() -> Json<&'static [scenarios::Scenario]> {
    Json(scenarios::PROGRESSION)
}

async fn set_scenario(
    State(world): State<Arc<RwLock<World>>>,
    Json(body): Json<ScenarioBody>,
) -> Result<Json<Status>, (axum::http::StatusCode, String)> {
    let mut w = world.write().await;
    w.apply_scenario(&body.id)
        .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e.to_string()))?;
    wsd::job_events::maybe_emit_elements_change(&w, wsd::soap::SCAN_NS_AIRSCAN, w.profile());
    Ok(Json(w.status()))
}

async fn set_fault(
    State(world): State<Arc<RwLock<World>>>,
    Json(body): Json<FaultBody>,
) -> Json<Status> {
    let mut w = world.write().await;
    let changes = w.apply_faults(&body);
    wsd::job_events::maybe_emit_condition_changes(&w, wsd::soap::SCAN_NS_AIRSCAN, &changes);
    if !changes.is_empty() {
        wsd::job_events::maybe_emit_status_summary(&w, wsd::soap::SCAN_NS_AIRSCAN);
    }
    Json(w.status())
}

async fn get_loopback(State(world): State<Arc<RwLock<World>>>) -> Json<LoopbackStatus> {
    Json(world.read().await.loopback_status())
}

async fn set_loopback(
    State(world): State<Arc<RwLock<World>>>,
    Json(body): Json<LoopbackBody>,
) -> Result<Json<Status>, (axum::http::StatusCode, String)> {
    let mut w = world.write().await;
    w.set_loopback(&body)
        .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e.to_string()))?;
    Ok(Json(w.status()))
}

async fn clear_loopback(State(world): State<Arc<RwLock<World>>>) -> Json<Status> {
    let mut w = world.write().await;
    w.clear_loopback();
    Json(w.status())
}

async fn reset(State(world): State<Arc<RwLock<World>>>) -> Json<Status> {
    let mut w = world.write().await;
    w.reset();
    Json(w.status())
}

async fn shutdown_signal(mut exit_rx: tokio::sync::watch::Receiver<bool>) {
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        result = exit_rx.changed() => {
            let _ = result;
        }
    }
    info!("shutting down");
}
