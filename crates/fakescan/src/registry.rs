//! Persistent device registry: `%LOCALAPPDATA%\fakescan\devices.json` (override:
//! `FAKESCAN_REGISTRY`). One row per emulated device; see `docs/multi-device.md` sections 8-10.
//!
//! Read by `ensure` / `--lab-name` / `--manage`; written by the device manager (install
//! tracking), `POST /v1/ctl save`, and `scripts/fakescan.ps1`.
//!
//! Writes go through [`RegistryStore::update`]: an in-process mutex plus an OS file lock on
//! `<registry>.lock`, a fresh read of the file, the mutation, then an atomic replace (temp file
//! in the same directory, fsync, rename). Concurrent writers (this process, Python) therefore
//! never lose each other's rows. Unknown JSON fields are preserved across a rewrite.
//!
//! Port policy: **fakescan uses even ports 3920, 3922, ...**  Odd ports (3921, 3923, ...) are
//! reserved for fakeprint.  `validate_port` enforces this.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::profile::Profile;
use crate::timeutil::utc_now_iso;

/// Environment override for the registry file (tests, sandboxes). Always wins.
pub const REGISTRY_ENV: &str = "FAKESCAN_REGISTRY";

/// `$FAKESCAN_REGISTRY` if set, else `%LOCALAPPDATA%\fakescan\devices.json` on Windows;
/// `~/.local/share/fakescan/devices.json` elsewhere.
///
/// In this crate's own unit tests the real location is never returned: a test that reaches
/// this function without `FAKESCAN_REGISTRY` set panics instead of touching the user's file.
pub fn registry_path() -> PathBuf {
    if let Some(p) = std::env::var_os(REGISTRY_ENV).filter(|p| !p.is_empty()) {
        return PathBuf::from(p);
    }
    #[cfg(test)]
    panic!("tests must set FAKESCAN_REGISTRY (see registry::test_support::init_env)");
    #[cfg(all(windows, not(test)))]
    {
        let base = std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("C:\\Users\\Default\\AppData\\Local"));
        base.join("fakescan").join("devices.json")
    }
    #[cfg(all(not(windows), not(test)))]
    {
        let home = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/tmp"));
        home.join(".local")
            .join("share")
            .join("fakescan")
            .join("devices.json")
    }
}

/// Top-level registry document.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Registry {
    /// Default LAN IP for WS-Discovery `--advertise` when a row has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub advertise_default: Option<String>,
    /// Port-allocation policy used by `add -Name`.
    #[serde(default)]
    pub port_policy: PortPolicy,
    #[serde(default)]
    pub devices: Vec<DeviceRow>,
    /// Unknown top-level fields, preserved on rewrite.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// fakescan allocates even ports; fakeprint allocates odd ports.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortPolicy {
    pub base: u16,
    pub step: u16,
}

impl Default for PortPolicy {
    fn default() -> Self {
        Self {
            base: 3920,
            step: 2,
        }
    }
}

/// One emulated device (one row per device, not per process).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceRow {
    /// Device id (slug), e.g. `default`, `bpl0`, `tray5`.
    pub name: String,
    /// `host:port` to bind, e.g. `0.0.0.0:3920`. Rows sharing a `listen` belong to one process.
    pub listen: String,
    /// LAN IP to advertise in WS-Discovery XAddrs.  Empty = auto.
    pub advertise: String,
    /// WS-Discovery / PnP device UUID.
    pub device_uuid: String,
    /// Capability profile id, e.g. `brother-wsd`.
    #[serde(default = "default_profile_id")]
    pub profile: String,
    /// Shown in Windows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub friendly_name: Option<String>,
    /// What this device was when last started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<ConfigInfo>,
    /// Install lifecycle (new | probed | installed | stale).
    #[serde(default)]
    pub install: InstallInfo,
    /// Path to the fakescan executable (`ensure` uses this to start the process).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exe: Option<String>,
    /// When true, `ensure --autostart` installs a logon task for this row.
    #[serde(default)]
    pub autostart: bool,
    /// Harness config applied after the process is up (`/v1/pattern`, `/v1/pages`, ...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness: Option<Value>,
    /// Legacy boolean mirror of `install.state == installed` (kept in sync).
    #[serde(default)]
    pub added: AddedInfo,
    /// Unknown fields, preserved on rewrite.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

fn default_profile_id() -> String {
    "brother-wsd".into()
}

/// Which Windows install surfaces have this instance (legacy; see [`InstallInfo`]).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AddedInfo {
    /// True after a person completed "Add device" and WIA confirmed it.
    #[serde(default)]
    pub pnp: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct FeederInfo {
    pub capacity: Option<u32>,
    #[serde(default)]
    pub media_sizes: Vec<String>,
    #[serde(default)]
    pub duplex: bool,
}

/// Effective profile summary and its two hashes.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ConfigInfo {
    /// `sha256:<hex>` over everything in the effective profile that matters (feeder capacity,
    /// media sizes, duplex, formats, dpi, colors, pages min/max).
    pub fingerprint: String,
    /// `sha256:<hex>` over the capability XML Windows caches at Add time (what
    /// GetScannerElements publishes). Capacity and runtime state are not in it.
    #[serde(default)]
    pub published_fingerprint: String,
    #[serde(default)]
    pub feeder: FeederInfo,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum InstallState {
    #[default]
    New,
    Probed,
    Installed,
    Stale,
}

/// The Windows device bound to a fakescan UUID, as observed on the Windows side.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct WiaInfo {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    /// WIA port string; carries the fakescan UUID (`urn:uuid:<uuid>/uri:<uuid>/ScannerService`).
    #[serde(default)]
    pub port: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InstallInfo {
    #[serde(default)]
    pub state: InstallState,
    /// Why the state is `stale`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_probed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_seen_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_peer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wia: Option<WiaInfo>,
    /// `config.published_fingerprint` when the device was installed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published_fingerprint_at_install: Option<String>,
    /// State the row was in when it went `stale` (normally `installed`). Lets a profile change
    /// that is later undone restore the row. Cleared whenever the row leaves `stale`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale_from: Option<InstallState>,
    /// When the row went `stale` (or last changed stale reason), UTC ISO. The tray holds off
    /// notifying about a profile-change stale until it has persisted for a grace period.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale_at: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Stale reason prefix for "the published profile differs from what Windows cached". Only this
/// kind of stale can heal by itself (when the profile is switched back).
pub const PROFILE_CHANGED_PREFIX: &str = "profile changed";

/// Canonical text for the full fingerprint: stable key order, sorted set-like lists.
fn canonical_profile(p: &Profile) -> String {
    fn sorted<T: Ord + Clone>(v: &[T]) -> Vec<T> {
        let mut v = v.to_vec();
        v.sort();
        v
    }
    let media: Vec<String> = sorted(
        &p.feeder
            .media_sizes
            .clone()
            .unwrap_or_default()
            .iter()
            .map(|s| s.trim().to_ascii_lowercase())
            .collect::<Vec<_>>(),
    );
    let v = serde_json::json!({
        "colors": sorted(&p.colors),
        "dpi": sorted(&p.dpi),
        "feeder": {
            "capacity": p.feeder.capacity,
            "duplex": p.feeder.duplex,
            "media_sizes": media,
        },
        "formats": sorted(&p.formats),
        "pages_max": p.feeder.pages_max,
        "pages_min": p.feeder.pages_min,
    });
    // serde_json maps are BTreeMaps (no preserve_order), so this is key-sorted.
    v.to_string()
}

fn sha256_hex(text: &str) -> String {
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    let out = h.finalize();
    let mut s = String::from("sha256:");
    for b in out {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Hash of the effective profile (everything that matters, including unpublished tray capacity).
pub fn config_fingerprint(p: &Profile) -> String {
    sha256_hex(&canonical_profile(p))
}

/// Hash of the capability XML the profile publishes (what Windows caches at Add time).
pub fn published_fingerprint(p: &Profile) -> String {
    sha256_hex(&crate::wsd::published_capability_xml(p))
}

/// Registry `config` block for a profile.
pub fn config_info(p: &Profile) -> ConfigInfo {
    ConfigInfo {
        fingerprint: config_fingerprint(p),
        published_fingerprint: published_fingerprint(p),
        feeder: FeederInfo {
            capacity: p.feeder.capacity,
            media_sizes: p.feeder.media_sizes.clone().unwrap_or_default(),
            duplex: p.feeder.duplex,
        },
    }
}

impl DeviceRow {
    pub fn new(
        name: impl Into<String>,
        listen: impl Into<String>,
        device_uuid: impl Into<String>,
        profile: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            listen: listen.into(),
            advertise: String::new(),
            device_uuid: device_uuid.into(),
            profile: profile.into(),
            friendly_name: None,
            config: None,
            install: InstallInfo::default(),
            exe: None,
            autostart: false,
            harness: None,
            added: AddedInfo::default(),
            extra: BTreeMap::new(),
        }
    }

    fn sync_pnp(&mut self) {
        self.added.pnp = self.install.state == InstallState::Installed;
    }

    /// Record peer contact.
    pub fn touch_probed(&mut self, peer: &str) {
        let now = utc_now_iso();
        let i = &mut self.install;
        i.last_seen_at = Some(now.clone());
        i.last_peer = Some(peer.to_string());
        if i.first_probed_at.is_none() {
            i.first_probed_at = Some(now);
        }
        if i.state == InstallState::New {
            i.state = InstallState::Probed;
        }
    }

    /// Mark installed (a Windows device bound to this UUID was observed).
    pub fn set_installed(&mut self, wia: Option<WiaInfo>, note: Option<String>) {
        let now = utc_now_iso();
        let published = self
            .config
            .as_ref()
            .map(|c| c.published_fingerprint.clone());
        let i = &mut self.install;
        if i.state != InstallState::Installed || i.installed_at.is_none() {
            i.installed_at = Some(now.clone());
        }
        i.state = InstallState::Installed;
        i.reason = None;
        i.stale_from = None;
        i.stale_at = None;
        i.note = note;
        i.verified_at = Some(now);
        let fresh = wia.is_some();
        if fresh {
            i.wia = wia;
        }
        if published.is_some() && (i.published_fingerprint_at_install.is_none() || fresh) {
            i.published_fingerprint_at_install = published;
        }
        self.sync_pnp();
    }

    pub fn set_stale(&mut self, reason: &str) {
        let i = &mut self.install;
        if i.state != InstallState::Stale {
            i.stale_from = Some(i.state);
            i.stale_at = Some(utc_now_iso());
        } else if i.reason.as_deref() != Some(reason) || i.stale_at.is_none() {
            i.stale_at = Some(utc_now_iso());
        }
        i.state = InstallState::Stale;
        i.reason = Some(reason.to_string());
        self.sync_pnp();
    }

    /// Undo a profile-change stale: the row is `stale` with a reason starting `profile changed`
    /// and `published` equals the fingerprint recorded at install, so Windows' cached
    /// capabilities are valid again. Restores the pre-stale state (installed; wia info,
    /// installed_at and the fingerprint are untouched). Any other stale reason (for example
    /// `not listed by Windows`) is never healed here: it needs a Windows reconcile.
    /// Rows written before `stale_from` existed are treated as coming from `installed`.
    pub fn heal_if_profile_restored(&mut self, published: &str) -> bool {
        let i = &mut self.install;
        let profile_stale = i.state == InstallState::Stale
            && i.reason
                .as_deref()
                .is_some_and(|r| r.trim_start().starts_with(PROFILE_CHANGED_PREFIX));
        if !profile_stale
            || published.is_empty()
            || i.published_fingerprint_at_install.as_deref() != Some(published)
        {
            return false;
        }
        i.state = i
            .stale_from
            .filter(|s| *s != InstallState::Stale)
            .unwrap_or(InstallState::Installed);
        i.reason = None;
        i.stale_from = None;
        i.stale_at = None;
        self.sync_pnp();
        true
    }

    /// Drop all install information, keep the row (identity + config).
    pub fn reset_install(&mut self) {
        self.install = InstallInfo::default();
        self.sync_pnp();
    }

    /// `(needs_readd, reason)`: true only when a previously installed device went stale.
    pub fn needs_readd(&self) -> (bool, Option<String>) {
        if self.install.state == InstallState::Stale {
            (true, self.install.reason.clone())
        } else {
            (false, None)
        }
    }
}

impl Registry {
    /// Load from [`registry_path`]; returns an empty registry if the file is absent.
    pub fn load() -> anyhow::Result<Self> {
        Self::load_from(&registry_path())
    }

    pub fn load_from(path: &Path) -> anyhow::Result<Self> {
        read_doc(path)
    }

    /// Replace the whole document at [`registry_path`] (locked, atomic).
    pub fn save_to_disk(&self) -> anyhow::Result<()> {
        let doc = self.clone();
        RegistryStore::file(registry_path()).update(move |r| *r = doc)?;
        Ok(())
    }

    /// Fix up rows written by older tools: `added.pnp = true` with no install state means
    /// installed (legacy).
    pub fn normalize(&mut self) {
        for d in &mut self.devices {
            if d.added.pnp && matches!(d.install.state, InstallState::New | InstallState::Probed) {
                d.install.state = InstallState::Installed;
                if d.install.note.is_none() {
                    d.install.note = Some("legacy added.pnp".into());
                }
            }
        }
    }

    pub fn find(&self, name: &str) -> Option<&DeviceRow> {
        self.devices.iter().find(|d| d.name == name)
    }

    pub fn find_mut(&mut self, name: &str) -> Option<&mut DeviceRow> {
        self.devices.iter_mut().find(|d| d.name == name)
    }

    /// Rows whose `listen` is the same socket address as `listen`.
    pub fn rows_for_listen(&self, listen: &str) -> Vec<&DeviceRow> {
        self.devices
            .iter()
            .filter(|d| same_listen(&d.listen, listen))
            .collect()
    }

    /// Insert or replace a row by name.
    pub fn upsert(&mut self, row: DeviceRow) {
        if let Some(r) = self.find_mut(&row.name.clone()) {
            *r = row;
        } else {
            self.devices.push(row);
        }
    }

    /// Remove a row by name.  Returns `true` if a row was removed.
    pub fn remove_by_name(&mut self, name: &str) -> bool {
        let before = self.devices.len();
        self.devices.retain(|d| d.name != name);
        self.devices.len() < before
    }

    /// Validate that a port is in the fakescan even-port band (3920, 3922, ...).
    pub fn validate_port(port: u16) -> anyhow::Result<()> {
        if !port.is_multiple_of(2) {
            bail!(
                "fakescan uses even ports (3920, 3922, ...); {} is odd - \
                 odd ports (3921, 3923, ...) are reserved for fakeprint",
                port
            );
        }
        Ok(())
    }

    /// Next free port in the even-port band (skips ports already in use in the registry).
    pub fn next_port(&self) -> u16 {
        let base = self.port_policy.base;
        let step = self.port_policy.step.max(2);
        let used: std::collections::HashSet<u16> = self
            .devices
            .iter()
            .filter_map(|d| d.listen.split(':').next_back()?.parse::<u16>().ok())
            .collect();
        let mut candidate = base;
        loop {
            if !used.contains(&candidate) {
                return candidate;
            }
            candidate = candidate.saturating_add(step);
        }
    }
}

/// True when two `host:port` strings are the same socket address (string-equal fallback).
pub fn same_listen(a: &str, b: &str) -> bool {
    match (
        a.parse::<std::net::SocketAddr>(),
        b.parse::<std::net::SocketAddr>(),
    ) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

/// Read and parse the registry file; absent = empty. A corrupt file is an error (never clobbered).
pub fn read_doc(path: &Path) -> anyhow::Result<Registry> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Registry::default()),
        Err(e) => return Err(e).with_context(|| format!("read {}", path.display())),
    };
    let text = text.trim_start_matches('\u{feff}'); // PowerShell writes a BOM
    if text.trim().is_empty() {
        return Ok(Registry::default());
    }
    let mut reg: Registry =
        serde_json::from_str(text).with_context(|| format!("parse {}", path.display()))?;
    reg.normalize();
    Ok(reg)
}

static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// Write `text` to `path` atomically: temp file in the same directory, fsync, rename.
pub fn atomic_write(path: &Path, text: &str) -> anyhow::Result<()> {
    let dir = path
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(
        ".{}.tmp.{}.{}",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("registry"),
        std::process::id(),
        TMP_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
    }
    // On Windows a reader that holds the target open can make the rename fail briefly.
    let mut last = None;
    for _ in 0..40 {
        match std::fs::rename(&tmp, path) {
            Ok(()) => return Ok(()),
            Err(e) => {
                last = Some(e);
                std::thread::sleep(std::time::Duration::from_millis(15));
            }
        }
    }
    let _ = std::fs::remove_file(&tmp);
    Err(last.unwrap()).with_context(|| format!("replace {}", path.display()))
}

/// Shared handle to the registry: file-backed (daemon mode) or in-memory (embedded servers).
pub struct RegistryStore {
    path: Option<PathBuf>,
    mem: Mutex<Registry>,
    gate: Mutex<()>,
    writes: AtomicU64,
}

impl std::fmt::Debug for RegistryStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RegistryStore({:?})", self.path)
    }
}

impl RegistryStore {
    /// File-backed store at `path`.
    pub fn file(path: impl Into<PathBuf>) -> Arc<Self> {
        Arc::new(Self {
            path: Some(path.into()),
            mem: Mutex::new(Registry::default()),
            gate: Mutex::new(()),
            writes: AtomicU64::new(0),
        })
    }

    /// In-memory only; nothing is ever written to disk.
    pub fn memory() -> Arc<Self> {
        Arc::new(Self {
            path: None,
            mem: Mutex::new(Registry::default()),
            gate: Mutex::new(()),
            writes: AtomicU64::new(0),
        })
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Number of times the file was actually rewritten (a no-op update does not count).
    pub fn writes(&self) -> u64 {
        self.writes.load(Ordering::Relaxed)
    }

    pub fn snapshot(&self) -> anyhow::Result<Registry> {
        match &self.path {
            Some(p) => read_doc(p),
            None => Ok(self.mem.lock().unwrap_or_else(|e| e.into_inner()).clone()),
        }
    }

    /// Read-modify-write under the in-process mutex and the OS file lock.
    pub fn update<R>(&self, f: impl FnOnce(&mut Registry) -> R) -> anyhow::Result<R> {
        let _g = self.gate.lock().unwrap_or_else(|e| e.into_inner());
        let Some(path) = &self.path else {
            let mut m = self.mem.lock().unwrap_or_else(|e| e.into_inner());
            return Ok(f(&mut m));
        };
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        let mut lock_name = path.as_os_str().to_owned();
        lock_name.push(".lock");
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(PathBuf::from(lock_name))?;
        lock.lock()?; // released when `lock` is dropped
        let mut reg = read_doc(path)?;
        let before = serde_json::to_string_pretty(&reg)?;
        let r = f(&mut reg);
        let after = serde_json::to_string_pretty(&reg)?;
        if after != before || !path.exists() {
            atomic_write(path, &after)?;
            self.writes.fetch_add(1, Ordering::Relaxed);
        }
        drop(lock);
        Ok(r)
    }
}

/// Helpers for tests that must never touch the real registry.
#[cfg(test)]
pub mod test_support {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static N: AtomicU64 = AtomicU64::new(0);

    /// A unique path under the temp dir (not created).
    pub fn temp_registry(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "fakescan-reg-{}-{}-{}",
            std::process::id(),
            tag,
            N.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("devices.json")
    }

    /// Point `FAKESCAN_REGISTRY` at a temp file for this whole test process (idempotent).
    pub fn init_env() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            std::env::set_var(super::REGISTRY_ENV, temp_registry("env"));
        });
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;

    fn row(name: &str, listen: &str) -> DeviceRow {
        DeviceRow::new(
            name,
            listen,
            "00000000-0000-4000-8000-0000000000f2",
            "brother-wsd",
        )
    }

    #[test]
    fn registry_path_honors_env_and_never_real_in_tests() {
        init_env();
        let p = registry_path();
        assert!(p.to_string_lossy().contains("fakescan-reg-"), "{p:?}");
    }

    #[test]
    fn validate_port_rejects_odd() {
        assert!(Registry::validate_port(3921).is_err(), "3921 is fakeprint");
        assert!(Registry::validate_port(3920).is_ok());
        assert!(Registry::validate_port(3922).is_ok());
    }

    #[test]
    fn next_port_skips_used() {
        let mut reg = Registry::default();
        reg.devices.push(row("default", "0.0.0.0:3920"));
        assert_eq!(reg.next_port(), 3922);
    }

    #[test]
    fn upsert_replaces_and_adds() {
        let mut reg = Registry::default();
        let r = row("a", "0.0.0.0:3920");
        reg.upsert(r.clone());
        assert_eq!(reg.devices.len(), 1);
        let mut r2 = r;
        r2.listen = "0.0.0.0:3922".into();
        reg.upsert(r2);
        assert_eq!(reg.devices.len(), 1);
        assert_eq!(reg.find("a").unwrap().listen, "0.0.0.0:3922");
    }

    #[test]
    fn remove_by_name_works() {
        let mut reg = Registry::default();
        reg.devices.push(row("x", "0.0.0.0:3920"));
        assert!(reg.remove_by_name("x"));
        assert!(!reg.remove_by_name("x")); // idempotent
        assert!(reg.devices.is_empty());
    }

    const OLD: &str = r#"{
      "port_policy": {"base": 3920, "step": 2},
      "future_top": {"k": 1},
      "devices": [
        {"name": "default", "listen": "0.0.0.0:3920", "advertise": "192.168.1.70",
         "device_uuid": "00000000-0000-4000-8000-0000000000f2", "profile": "brother-wsd",
         "exe": "C:\\x\\fakescan.exe", "autostart": true, "added": {"pnp": true}, "mystery": [1,2]},
        {"name": "bpl0", "listen": "0.0.0.0:3922", "advertise": "",
         "device_uuid": "00000000-0000-4000-8000-0000000000f3"}
      ]}"#;

    #[test]
    fn old_format_loads_and_round_trips() {
        let path = temp_registry("old");
        std::fs::write(&path, OLD).unwrap();
        let store = RegistryStore::file(&path);
        let reg = store.snapshot().unwrap();
        assert_eq!(reg.devices.len(), 2);
        let d = reg.find("default").unwrap();
        assert!(d.added.pnp);
        assert_eq!(
            d.install.state,
            InstallState::Installed,
            "legacy pnp -> installed"
        );
        assert_eq!(reg.find("bpl0").unwrap().profile, "brother-wsd");
        assert_eq!(reg.find("bpl0").unwrap().install.state, InstallState::New);
        // Rewrite (touch a row) keeps unknown fields.
        store
            .update(|r| r.find_mut("bpl0").unwrap().touch_probed("10.0.0.1"))
            .unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["future_top"]["k"], 1);
        assert_eq!(v["devices"][0]["mystery"][1], 2);
        assert_eq!(v["devices"][0]["added"]["pnp"], true);
        assert_eq!(v["devices"][1]["install"]["state"], "probed");
        let again = store.snapshot().unwrap();
        assert_eq!(again.devices.len(), 2);
        assert!(crate::timeutil::is_utc_iso(
            again.devices[1].install.last_seen_at.as_ref().unwrap()
        ));
    }

    #[test]
    fn pnp_stays_in_sync() {
        let mut r = row("a", "0.0.0.0:3920");
        r.set_installed(None, Some("legacy mark-added".into()));
        assert!(r.added.pnp);
        r.set_stale("x");
        assert!(!r.added.pnp);
        r.reset_install();
        assert_eq!(r.install.state, InstallState::New);
    }

    fn profiles() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../profiles")
    }

    #[test]
    fn fingerprint_changes_with_capacity_but_published_does_not() {
        let t5 = Profile::load(&profiles(), "feeder-tray5").unwrap();
        let mut t5b = t5.clone();
        t5b.feeder.capacity = Some(5);
        assert_eq!(config_fingerprint(&t5), config_fingerprint(&t5b));
        t5b.feeder.capacity = Some(9);
        assert_ne!(config_fingerprint(&t5), config_fingerprint(&t5b));
        assert_eq!(
            published_fingerprint(&t5),
            published_fingerprint(&t5b),
            "capacity is unpublished"
        );
        assert!(config_fingerprint(&t5).starts_with("sha256:"));
        let t50 = Profile::load(&profiles(), "feeder-tray50").unwrap();
        assert_ne!(config_fingerprint(&t5), config_fingerprint(&t50));
        let glass = Profile::load(&profiles(), "glass-wsd").unwrap();
        assert_ne!(published_fingerprint(&t5), published_fingerprint(&glass));
    }

    #[test]
    fn concurrent_writers_lose_no_rows() {
        let path = temp_registry("conc");
        let a = RegistryStore::file(&path);
        let b = RegistryStore::file(&path); // separate handle: only the file lock excludes it
        a.update(|r| {
            r.upsert(row("a", "0.0.0.0:3920"));
            r.upsert(row("b", "0.0.0.0:3920"));
        })
        .unwrap();
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let reader = {
            let (p, stop) = (path.clone(), stop.clone());
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    if let Ok(t) = std::fs::read_to_string(&p) {
                        serde_json::from_str::<Value>(&t).expect("file always valid JSON");
                    }
                }
            })
        };
        let ta = std::thread::spawn(move || {
            for i in 0..60 {
                a.update(|r| {
                    r.find_mut("a")
                        .unwrap()
                        .touch_probed(&format!("10.0.0.{i}"))
                })
                .unwrap();
            }
            a.update(|r| r.find_mut("a").unwrap().set_installed(None, None))
                .unwrap();
        });
        let tb = std::thread::spawn(move || {
            for i in 0..60 {
                b.update(|r| {
                    let d = r.find_mut("b").unwrap();
                    d.set_installed(
                        Some(WiaInfo {
                            id: i.to_string(),
                            ..Default::default()
                        }),
                        None,
                    )
                })
                .unwrap();
            }
        });
        ta.join().unwrap();
        tb.join().unwrap();
        stop.store(true, Ordering::Relaxed);
        reader.join().unwrap();
        let reg = read_doc(&path).unwrap();
        assert_eq!(reg.devices.len(), 2);
        assert_eq!(
            reg.find("a").unwrap().install.state,
            InstallState::Installed
        );
        assert_eq!(
            reg.find("a").unwrap().install.last_peer.as_deref(),
            Some("10.0.0.59")
        );
        assert_eq!(
            reg.find("b").unwrap().install.wia.as_ref().unwrap().id,
            "59"
        );
    }
}
