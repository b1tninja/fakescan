//! Runtime device manager: the set of emulated scanners a server hosts.
//!
//! Each [`DeviceConfig`] becomes a [`Device`]: its own `World`, its own prebuilt control/WSD
//! router and (when discovery is on) its own WS-Discovery announcer. Devices can be added and
//! removed while the server runs; HTTP routing resolves the device per request (see
//! `server::dispatch`), so nothing is nested statically at startup.
//!
//! The manager also owns install tracking (docs/multi-device.md sections 8-10): every device has
//! a registry row; peer contact, Windows installs, reconfigures and reconciles update it. With no
//! registry file attached (embedded servers) the same state lives in memory only.

use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use anyhow::{bail, ensure, Result};
use axum::Router;
use tokio::sync::{oneshot, RwLock};
use tokio::task::JoinHandle;
use tracing::info;

use crate::discovery::{self, DiscoveryConfig, DiscoveryStats};
use crate::profile::Profile;
use crate::registry::{
    config_info, ConfigInfo, DeviceRow, InstallState, Registry, RegistryStore, WiaInfo,
};
use crate::server::{device_router_ctx, DeviceConfig, DeviceInfo};
use crate::state::World;

/// Server-wide settings every device inherits.
#[derive(Debug, Clone)]
pub struct ManagerConfig {
    pub profiles: PathBuf,
    pub wsd: bool,
    pub discovery: bool,
    pub probe_listen: bool,
    /// Advertised host (already resolved; unspecified becomes loopback for endpoints).
    pub advertise: IpAddr,
    pub port: u16,
    pub lab_name: Option<String>,
    pub local_addr: std::net::SocketAddr,
    /// The configured listen address as a string (registry rows are keyed by it).
    pub listen_key: String,
    /// Registry to persist device rows and install state to. `None` = in-memory only (embedded
    /// servers never touch a registry file).
    pub registry: Option<Arc<RegistryStore>>,
}

/// Reason recorded when a reconfigure (or a changed profile file) alters what Windows cached.
pub const REASON_PROFILE_CHANGED: &str = "profile changed; remove and re-Add in Imaging";
/// Reason recorded when reconcile no longer finds the device in Windows.
pub const REASON_NOT_LISTED: &str = "not listed by Windows";

/// Default minimum interval between `last_seen_at` rewrites per device.
pub const PROBE_THROTTLE: Duration = Duration::from_secs(30);

/// Result of [`DeviceManager::reconcile_with_windows`].
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ReconcileResult {
    /// Registry device names now `installed` (matched by UUID in the WIA port string).
    pub installed: Vec<String>,
    /// Registry device names that became `stale` because of this reconcile.
    pub stale: Vec<String>,
    /// WIA entries whose port carries no registered fakescan UUID. Never touched.
    pub foreign: Vec<WiaInfo>,
}

/// Handed to each device router so request handlers can reach the manager without a back-pointer
/// cycle: the WSD path records peer contact, `/v1/ctl` runs manager commands.
#[derive(Clone)]
pub struct DeviceCtx {
    pub manager: Weak<DeviceManager>,
    pub id: String,
}

impl DeviceCtx {
    /// Record peer contact off the request path (throttled; file IO runs on a blocking thread).
    pub fn probe(&self, peer: IpAddr) {
        let Some(m) = self.manager.upgrade() else {
            return;
        };
        if !m.probe_due(&self.id, false) {
            return;
        }
        let id = self.id.clone();
        tokio::spawn(async move {
            if let Err(e) = m.mark_probed(&id, &peer.to_string()).await {
                tracing::warn!(device = %id, error = %e, "recording peer contact failed");
            }
        });
    }
}

struct Announcer {
    stop: oneshot::Sender<()>,
    task: JoinHandle<()>,
}

/// One running device.
pub struct Device {
    pub cfg: DeviceConfig,
    pub info: DeviceInfo,
    pub world: Arc<RwLock<World>>,
    router: Router,
    announcer: Option<Announcer>,
    /// Registry row name (device id, except the default device of a `--lab-name` instance).
    row: String,
}

/// What a request handler needs to serve one device.
#[derive(Clone)]
pub struct DeviceHandle {
    pub info: DeviceInfo,
    pub world: Arc<RwLock<World>>,
    pub router: Router,
}

#[derive(Default)]
struct Inner {
    /// Insertion order, for stable listings.
    order: Vec<String>,
    devices: HashMap<String, Device>,
    default_id: Option<String>,
}

pub struct DeviceManager {
    cfg: ManagerConfig,
    exit_tx: Arc<tokio::sync::watch::Sender<bool>>,
    inner: RwLock<Inner>,
    store: Arc<RegistryStore>,
    me: Weak<DeviceManager>,
    probe_last: Mutex<HashMap<String, Instant>>,
    probe_interval: Mutex<Duration>,
    launcher: Mutex<Arc<dyn SettingsLauncher>>,
}

/// Opens a Windows Settings page for the operator UI / ctl `open_settings`. Injectable so tests
/// never open a real window.
pub trait SettingsLauncher: Send + Sync {
    /// Open the settings `page` (already validated; only `printers` today). `Ok(true)` = launched.
    fn open(&self, page: &str) -> Result<bool>;
}

/// Default launcher: `cmd /C start "" ms-settings:<page>` on Windows, an error elsewhere.
pub struct SystemSettingsLauncher;

impl SettingsLauncher for SystemSettingsLauncher {
    fn open(&self, page: &str) -> Result<bool> {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            let status = std::process::Command::new("cmd")
                .args(["/C", "start", "", &format!("ms-settings:{page}")])
                .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
                .status()?;
            Ok(status.success())
        }
        #[cfg(not(windows))]
        {
            let _ = page;
            bail!("open_settings is only available on Windows")
        }
    }
}

impl DeviceManager {
    /// Replace the settings launcher (tests inject a recorder).
    pub fn set_settings_launcher(&self, l: Arc<dyn SettingsLauncher>) {
        *self.launcher.lock().unwrap_or_else(|e| e.into_inner()) = l;
    }

    pub fn settings_launcher(&self) -> Arc<dyn SettingsLauncher> {
        Arc::clone(&self.launcher.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

impl DeviceManager {
    /// Returns the manager and the receiver that fires when any device asks the server to exit.
    pub fn new(cfg: ManagerConfig) -> (Arc<Self>, tokio::sync::watch::Receiver<bool>) {
        let (tx, rx) = tokio::sync::watch::channel(false);
        let store = cfg.registry.clone().unwrap_or_else(RegistryStore::memory);
        let m = Arc::new_cyclic(|me| Self {
            cfg,
            exit_tx: Arc::new(tx),
            inner: RwLock::new(Inner::default()),
            store,
            me: me.clone(),
            probe_last: Mutex::new(HashMap::new()),
            probe_interval: Mutex::new(PROBE_THROTTLE),
            launcher: Mutex::new(Arc::new(SystemSettingsLauncher)),
        });
        (m, rx)
    }

    pub fn config(&self) -> &ManagerConfig {
        &self.cfg
    }

    /// All devices with their install/config view, in insertion order.
    pub async fn list(&self) -> Vec<DeviceInfo> {
        let reg = self.store.snapshot().unwrap_or_default();
        let g = self.inner.read().await;
        g.order
            .iter()
            .filter_map(|id| g.devices.get(id))
            .map(|d| decorate(d.info.clone(), reg.find(&d.row)))
            .collect()
    }

    /// The registry store (file-backed in daemon mode, in-memory otherwise).
    pub fn store(&self) -> &Arc<RegistryStore> {
        &self.store
    }

    /// The registry document as the manager sees it.
    pub fn registry_doc(&self) -> Result<Registry> {
        self.store.snapshot()
    }

    /// Override the per-device `last_seen_at` rewrite interval (tests).
    pub fn set_probe_interval(&self, d: Duration) {
        *self
            .probe_interval
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = d;
    }

    /// Stored UUID for a device id, if the registry has a row for it (so a runtime re-add with no
    /// explicit UUID keeps the identity Windows knows).
    pub fn stored_uuid(&self, id: &str) -> Option<uuid::Uuid> {
        let reg = self.store.snapshot().ok()?;
        reg.find(id)
            .and_then(|r| uuid::Uuid::parse_str(&r.device_uuid).ok())
    }

    fn probe_due(&self, id: &str, claim: bool) -> bool {
        let interval = *self
            .probe_interval
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut m = self.probe_last.lock().unwrap_or_else(|e| e.into_inner());
        let due = m.get(id).is_none_or(|t| t.elapsed() >= interval);
        if due && claim {
            m.insert(id.to_string(), Instant::now());
        }
        due
    }

    async fn row_key(&self, id: &str) -> String {
        self.inner
            .read()
            .await
            .devices
            .get(id)
            .map(|d| d.row.clone())
            .unwrap_or_else(|| id.to_string())
    }

    fn row_name_for(&self, id: &str, is_default: bool) -> String {
        if is_default && id == "default" {
            self.cfg.lab_name.clone().unwrap_or_else(|| id.to_string())
        } else {
            id.to_string()
        }
    }

    pub async fn get(&self, id: &str) -> Option<DeviceHandle> {
        self.inner.read().await.devices.get(id).map(Device::handle)
    }

    /// The legacy-route device: the first one added that is still present.
    pub async fn default_device(&self) -> Option<DeviceHandle> {
        let g = self.inner.read().await;
        g.default_id
            .as_ref()
            .and_then(|id| g.devices.get(id))
            .map(Device::handle)
    }

    /// Add and start a device. The first device added becomes the default and keeps the legacy
    /// endpoint paths; later ones live under `/d/{id}`.
    pub async fn add(&self, dc: DeviceConfig) -> Result<DeviceInfo> {
        ensure!(
            valid_id(&dc.id),
            "invalid device id {:?} (use [A-Za-z0-9_-]+)",
            dc.id
        );
        // Identity is the UUID, never the name: only the fake namespace is ours.
        ensure!(
            dc.allow_foreign_uuid || crate::lab::uuid_is_fake(&dc.device_uuid),
            "device UUID {} is outside the fake namespace {}*",
            dc.device_uuid,
            crate::lab::FAKE_UUID_PREFIX
        );
        let mut g = self.inner.write().await;
        ensure!(
            !g.devices.contains_key(&dc.id),
            "duplicate device id {:?}",
            dc.id
        );
        ensure!(
            !g.devices
                .values()
                .any(|d| d.cfg.device_uuid == dc.device_uuid),
            "device UUID {} already in use",
            dc.device_uuid
        );
        let profile = Profile::load(&self.cfg.profiles, &dc.profile)?;
        info!(device = %dc.id, id = %profile.id, "loaded profile");

        let is_default = g.default_id.is_none();
        let stats = Arc::new(DiscoveryStats::default());
        let mut w = World::with_discovery(
            self.cfg.profiles.clone(),
            profile.clone(),
            Arc::clone(&stats),
        );
        let adv = if self.cfg.advertise.is_unspecified() {
            IpAddr::V4(Ipv4Addr::LOCALHOST)
        } else {
            self.cfg.advertise
        };
        let prefix = if is_default {
            String::new()
        } else {
            format!("/d/{}", dc.id)
        };
        let (mut device_ep, mut scan_ep) = (None, None);
        if self.cfg.wsd {
            let de = format!(
                "http://{}:{}{}/WebServices/Device",
                adv, self.cfg.port, prefix
            );
            let se = format!("http://{}:{}{}/ScannerService", adv, self.cfg.port, prefix);
            w.set_identity(de.clone(), se.clone(), dc.device_uuid);
            device_ep = Some(de);
            scan_ep = Some(se);
        }
        w.set_lab_identity(
            if is_default {
                self.cfg.lab_name.clone()
            } else {
                None
            },
            self.cfg.local_addr,
        );
        let (dev_exit_tx, mut dev_exit_rx) = tokio::sync::watch::channel(false);
        w.arm_exit(dev_exit_tx);
        let fwd = Arc::clone(&self.exit_tx);
        tokio::spawn(async move {
            if dev_exit_rx.changed().await.is_ok() {
                let _ = fwd.send(true);
            }
        });

        let friendly_name = dc.friendly_name.clone().unwrap_or_else(|| {
            if is_default {
                "fakescan".to_string()
            } else {
                format!("fakescan {}", dc.id)
            }
        });
        let mut info = DeviceInfo {
            id: dc.id.clone(),
            uuid: dc.device_uuid,
            profile: dc.profile.clone(),
            friendly_name: friendly_name.clone(),
            scan_endpoint: scan_ep,
            device_endpoint: device_ep.clone(),
            install: None,
            config: None,
            needs_readd: false,
            needs_readd_reason: None,
        };

        let announcer = match device_ep.filter(|_| self.cfg.discovery && self.cfg.wsd) {
            Some(ep) => {
                let multicast_iface = match self.cfg.advertise {
                    IpAddr::V4(v4) if !v4.is_unspecified() => v4,
                    _ => Ipv4Addr::UNSPECIFIED,
                };
                // Resolve before spawn so GET /v1/status sees nonzero ids immediately.
                let instance_id = discovery::boot_sequence_id();
                stats.set_sequence(instance_id, instance_id);
                let dcfg = DiscoveryConfig {
                    endpoint: ep.clone(),
                    device_uuid: dc.device_uuid,
                    friendly_name,
                    multicast_iface,
                    listen_probes: self.cfg.probe_listen,
                    stats,
                    instance_id,
                    metadata_version: instance_id,
                    on_peer: Some({
                        let ctx = DeviceCtx {
                            manager: self.me.clone(),
                            id: dc.id.clone(),
                        };
                        Arc::new(move |ip| ctx.probe(ip))
                    }),
                };
                info!(device = %dc.id, %ep, %multicast_iface, "WSD Scan Service + WS-Discovery enabled");
                let (stop, stop_rx) = oneshot::channel();
                let task = tokio::spawn(async move {
                    if let Err(e) = discovery::run(dcfg, stop_rx).await {
                        tracing::error!(error = %e, "WS-Discovery stopped");
                    }
                });
                Some(Announcer { stop, task })
            }
            None => None,
        };

        let row = self.row_name_for(&dc.id, is_default);
        let ci = config_info(&profile);
        let advertise = if self.cfg.advertise.is_unspecified() {
            String::new()
        } else {
            self.cfg.advertise.to_string()
        };
        if let Err(e) = self.persist_device(&row, &dc, &info.friendly_name, &advertise, &ci) {
            tracing::warn!(device = %dc.id, error = %e, "registry write failed");
        }
        if let Ok(reg) = self.store.snapshot() {
            info = decorate(info, reg.find(&row));
        }

        let world = Arc::new(RwLock::new(w));
        let ctx = DeviceCtx {
            manager: self.me.clone(),
            id: dc.id.clone(),
        };
        let device = Device {
            router: device_router_ctx(Arc::clone(&world), self.cfg.wsd, Some(ctx)),
            world,
            cfg: dc.clone(),
            info: info.clone(),
            announcer,
            row,
        };
        if is_default {
            g.default_id = Some(dc.id.clone());
        }
        g.order.push(dc.id.clone());
        g.devices.insert(dc.id, device);
        Ok(info)
    }

    /// Create or refresh this device's registry row. An existing row with the same UUID keeps its
    /// install state (a restart is invisible to Windows); a different UUID is a new identity.
    fn persist_device(
        &self,
        row_name: &str,
        dc: &DeviceConfig,
        friendly: &str,
        advertise: &str,
        ci: &ConfigInfo,
    ) -> Result<()> {
        let uuid = dc.device_uuid.to_string();
        let listen = self.cfg.listen_key.clone();
        self.store.update(|reg| {
            match reg.find(row_name).cloned() {
                Some(old) if old.device_uuid.eq_ignore_ascii_case(&uuid) => {
                    let r = reg.find_mut(row_name).expect("row");
                    r.listen = listen;
                    r.profile = dc.profile.clone();
                    r.friendly_name = Some(friendly.to_string());
                    // The profile file may have changed while we were down.
                    apply_published_fingerprint(r, ci);
                    r.config = Some(ci.clone());
                }
                old => {
                    let mut r = DeviceRow::new(row_name, listen, uuid, dc.profile.clone());
                    r.advertise = advertise.to_string();
                    if let Some(old) = old {
                        r.advertise = old.advertise;
                        r.exe = old.exe;
                        r.autostart = old.autostart;
                        r.harness = old.harness;
                    }
                    r.friendly_name = Some(friendly.to_string());
                    r.config = Some(ci.clone());
                    reg.upsert(r);
                }
            }
        })
    }

    /// Stop a device (WSD Bye when discovery is on) and drop it. Its `/d/{id}` routes 404 at once.
    /// The registry row (and its install state) is kept; see [`DeviceManager::remove_row`].
    pub async fn remove(&self, id: &str) -> Result<()> {
        self.remove_row(id, false).await
    }

    /// Like [`DeviceManager::remove`]; with `drop_row` the registry row is deleted too.
    pub async fn remove_row(&self, id: &str, drop_row: bool) -> Result<()> {
        let device = {
            let mut g = self.inner.write().await;
            let Some(device) = g.devices.remove(id) else {
                bail!("unknown device");
            };
            g.order.retain(|x| x != id);
            if g.default_id.as_deref() == Some(id) {
                g.default_id = g.order.first().cloned();
            }
            device
        };
        if drop_row {
            let row = device.row.clone();
            if let Err(e) = self.store.update(|r| r.remove_by_name(&row)) {
                tracing::warn!(device = %id, error = %e, "registry write failed");
            }
        }
        self.probe_last
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(id);
        stop_announcer(device.announcer).await;
        Ok(())
    }

    /// Swap a device's profile. An installed device goes `stale` only if what Windows caches
    /// (the published capability set) changed; capacity-only changes never do.
    pub async fn reconfigure(&self, id: &str, profile_id: &str) -> Result<DeviceInfo> {
        let profile = Profile::load(&self.cfg.profiles, profile_id)?;
        let ci = config_info(&profile);
        let mut g = self.inner.write().await;
        let Some(dev) = g.devices.get_mut(id) else {
            bail!("unknown device");
        };
        {
            let mut w = dev.world.write().await;
            w.load_profile(profile_id)?;
            crate::wsd::job_events::maybe_emit_elements_change(
                &w,
                crate::wsd::soap::SCAN_NS_AIRSCAN,
                w.profile(),
            );
        }
        dev.cfg.profile = profile_id.to_string();
        dev.info.profile = profile_id.to_string();
        let row = dev.row.clone();
        let pid = profile_id.to_string();
        self.store.update(|reg| {
            if let Some(r) = reg.find_mut(&row) {
                r.profile = pid;
                apply_published_fingerprint(r, &ci);
                r.config = Some(ci.clone());
            }
        })?;
        let reg = self.store.snapshot().unwrap_or_default();
        dev.info = decorate(dev.info.clone(), reg.find(&row));
        Ok(dev.info.clone())
    }

    /// Record peer contact for `id` (throttled: the file is rewritten at most once per interval
    /// per device; the first contact always writes). Returns whether a write happened.
    pub async fn mark_probed(&self, id: &str, peer: &str) -> Result<bool> {
        if !self.probe_due(id, true) {
            return Ok(false);
        }
        let row = self.row_key(id).await;
        let peer = peer.to_string();
        let store = Arc::clone(&self.store);
        tokio::task::spawn_blocking(move || {
            store.update(|reg| match reg.find_mut(&row) {
                Some(r) => {
                    r.touch_probed(&peer);
                    true
                }
                None => false,
            })
        })
        .await?
    }

    /// A WIA device bound to this device's UUID was observed. The port must carry the UUID:
    /// nothing is ever marked installed on a name guess.
    pub async fn mark_installed(&self, id: &str, wia: WiaInfo) -> Result<()> {
        let row = self.row_key(id).await;
        self.store.update(|reg| -> Result<()> {
            let Some(r) = reg.find_mut(&row) else {
                bail!("unknown device")
            };
            ensure!(
                port_has_uuid(&wia.port, &r.device_uuid),
                "wia_port does not contain this device's UUID {}; refusing to mark installed",
                r.device_uuid
            );
            r.set_installed(Some(wia), None);
            Ok(())
        })?
    }

    /// Legacy `mark-added`: installed, but claiming no WIA details.
    pub async fn mark_installed_legacy(&self, id: &str) -> Result<()> {
        let row = self.row_key(id).await;
        self.store.update(|reg| -> Result<()> {
            let Some(r) = reg.find_mut(&row) else {
                bail!("unknown device")
            };
            r.set_installed(None, Some("legacy mark-added".into()));
            Ok(())
        })?
    }

    pub async fn mark_stale(&self, id: &str, reason: &str) -> Result<()> {
        let row = self.row_key(id).await;
        self.store.update(|reg| -> Result<()> {
            let Some(r) = reg.find_mut(&row) else {
                bail!("unknown device")
            };
            r.set_stale(reason);
            Ok(())
        })?
    }

    /// Drop install information (back to `new`), keep the row.
    pub async fn forget(&self, id: &str) -> Result<()> {
        let row = self.row_key(id).await;
        self.probe_last
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(id);
        self.store.update(|reg| -> Result<()> {
            let Some(r) = reg.find_mut(&row) else {
                bail!("unknown device")
            };
            r.reset_install();
            Ok(())
        })?
    }

    /// Reconcile the registry against the Windows scanner list. A WIA entry matches a row ONLY
    /// when the row's device UUID appears in the entry's port string. Matched = installed
    /// (stale if the published capabilities changed since install); a previously installed row
    /// with no match = stale; entries matching no row are returned as `foreign` and never touched.
    pub fn reconcile_with_windows(&self, listing: &[WiaInfo]) -> Result<ReconcileResult> {
        self.store.update(|reg| {
            let mut out = ReconcileResult::default();
            let mut matched: HashSet<usize> = HashSet::new();
            for entry in listing {
                let hits: Vec<usize> = reg
                    .devices
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| port_has_uuid(&entry.port, &r.device_uuid))
                    .map(|(i, _)| i)
                    .collect();
                if hits.is_empty() {
                    out.foreign.push(entry.clone());
                    continue;
                }
                // Ambiguous (several UUIDs) or a duplicate listing of one device: skip.
                if hits.len() != 1 || matched.contains(&hits[0]) {
                    continue;
                }
                let i = hits[0];
                matched.insert(i);
                let r = &mut reg.devices[i];
                let prev = r.install.published_fingerprint_at_install.clone();
                let cur = r.config.as_ref().map(|c| c.published_fingerprint.clone());
                let was_installed = matches!(
                    r.install.state,
                    InstallState::Installed | InstallState::Stale
                );
                // A different WIA id means Windows removed and re-Added it: capabilities re-read.
                let re_added = r
                    .install
                    .wia
                    .as_ref()
                    .is_some_and(|w| !w.id.is_empty() && w.id != entry.id);
                if was_installed && !re_added && prev.is_some() && cur.is_some() && prev != cur {
                    r.install.wia = Some(entry.clone());
                    r.install.verified_at = Some(crate::timeutil::utc_now_iso());
                    r.set_stale(REASON_PROFILE_CHANGED);
                    out.stale.push(r.name.clone());
                } else {
                    r.set_installed(Some(entry.clone()), None);
                    out.installed.push(r.name.clone());
                }
            }
            for (i, r) in reg.devices.iter_mut().enumerate() {
                if !matched.contains(&i) && r.install.state == InstallState::Installed {
                    r.set_stale(REASON_NOT_LISTED);
                    out.stale.push(r.name.clone());
                }
            }
            out
        })
    }

    /// Stop every announcer (used at server shutdown) and wait briefly for their Byes.
    pub async fn shutdown(&self) {
        let announcers: Vec<Announcer> = {
            let mut g = self.inner.write().await;
            g.devices
                .values_mut()
                .filter_map(|d| d.announcer.take())
                .collect()
        };
        let mut tasks = Vec::new();
        for a in announcers {
            let _ = a.stop.send(());
            tasks.push(a.task);
        }
        for t in tasks {
            if tokio::time::timeout(Duration::from_millis(1500), t)
                .await
                .is_err()
            {
                tracing::warn!("discovery Bye timed out");
            }
        }
    }
}

async fn stop_announcer(a: Option<Announcer>) {
    if let Some(a) = a {
        let _ = a.stop.send(());
        if tokio::time::timeout(Duration::from_millis(1500), a.task)
            .await
            .is_err()
        {
            tracing::warn!("discovery Bye timed out");
        }
    }
}

impl Device {
    fn handle(&self) -> DeviceHandle {
        DeviceHandle {
            info: self.info.clone(),
            world: Arc::clone(&self.world),
            router: self.router.clone(),
        }
    }
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Add the install/config view of a registry row to a device listing entry.
fn decorate(mut info: DeviceInfo, row: Option<&DeviceRow>) -> DeviceInfo {
    if let Some(r) = row {
        info.install = Some(r.install.clone());
        info.config = r.config.clone();
        let (n, why) = r.needs_readd();
        info.needs_readd = n;
        info.needs_readd_reason = why;
    }
    info
}

/// Re-evaluate a row against the (re-)applied profile's published capability set.
///  - installed and the set differs from the one recorded at install: `stale` (reason
///    "profile changed", remembering it came from `installed`; unpublished changes never do);
///  - stale with a "profile changed" reason and the set equals the install-time one again:
///    healed back to the state it came from (a temporary profile switch is invisible);
///  - anything else (including stale "not listed by Windows") is left alone.
///
/// Called from every path that applies a profile: `reconfigure` (POST /v1/profile, ctl
/// `profile`, /v1/devices/{id}/reconfigure) and `persist_device` (daemon/`--manage` start-up).
/// `POST /v1/reset` does not re-apply a profile, so it never reaches this.
fn apply_published_fingerprint(r: &mut DeviceRow, new: &ConfigInfo) {
    match r.install.state {
        InstallState::Installed => {
            let base = r
                .install
                .published_fingerprint_at_install
                .clone()
                .or_else(|| r.config.as_ref().map(|c| c.published_fingerprint.clone()))
                .filter(|s| !s.is_empty());
            if let Some(base) = base {
                if base != new.published_fingerprint {
                    r.set_stale(REASON_PROFILE_CHANGED);
                }
            }
        }
        InstallState::Stale => {
            r.heal_if_profile_restored(&new.published_fingerprint);
        }
        _ => {}
    }
}

/// UUID-shaped (8-4-4-4-12 hex) substrings of `s`, lowercased.
fn uuids_in(s: &str) -> Vec<String> {
    let b = s.to_ascii_lowercase().into_bytes();
    let shape = [8usize, 4, 4, 4, 12];
    let total = 36;
    let mut out = Vec::new();
    let mut i = 0;
    while i + total <= b.len() {
        let mut ok = true;
        let mut p = i;
        for (k, n) in shape.iter().enumerate() {
            if !b[p..p + n].iter().all(|c| c.is_ascii_hexdigit()) {
                ok = false;
                break;
            }
            p += n;
            if k < 4 {
                if b[p] != b'-' {
                    ok = false;
                    break;
                }
                p += 1;
            }
        }
        if ok {
            out.push(String::from_utf8_lossy(&b[i..i + total]).into_owned());
            i += total;
        } else {
            i += 1;
        }
    }
    out
}

/// True when the WIA port string names exactly one UUID (it may repeat, as in
/// `urn:uuid:X/uri:X/ScannerService`) and that UUID is `uuid`. Case, braces and the `urn:uuid:`
/// prefix are ignored. A port naming any other UUID as well is foreign.
pub fn port_has_uuid(port: &str, uuid: &str) -> bool {
    let want = uuids_in(uuid);
    if want.len() != 1 {
        return false;
    }
    let mut found = uuids_in(port);
    found.sort();
    found.dedup();
    found.len() == 1 && found[0] == want[0]
}
