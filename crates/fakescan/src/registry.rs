//! Per-repo device registry: `%LOCALAPPDATA%\fakescan\devices.json`
//!
//! Read by `ensure` / `--lab-name` to resolve listen/advertise/uuid from a
//! named row without requiring explicit CLI flags on every restart.
//! Written by `POST /v1/ctl save`, `scripts/fakescan.ps1 save`, and `mark-added`.
//!
//! Port policy: **fakescan uses even ports 3920, 3922, …**  Odd ports (3921,
//! 3923, …) are reserved for fakeprint.  `validate_port` enforces this.

use anyhow::bail;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// `%LOCALAPPDATA%\fakescan\devices.json` on Windows;
/// `~/.local/share/fakescan/devices.json` elsewhere.
pub fn registry_path() -> PathBuf {
    #[cfg(windows)]
    {
        let base = std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("C:\\Users\\Default\\AppData\\Local"));
        base.join("fakescan").join("devices.json")
    }
    #[cfg(not(windows))]
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

/// One named lab instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceRow {
    /// Short name, e.g. `default` or `bpl0`.
    pub name: String,
    /// `host:port` to bind, e.g. `0.0.0.0:3920`.
    pub listen: String,
    /// LAN IP to advertise in WS-Discovery XAddrs.  Empty → auto.
    pub advertise: String,
    /// WS-Discovery / PnP device UUID.
    pub device_uuid: String,
    /// Capability profile id, e.g. `brother-wsd`.
    #[serde(default = "default_profile_id")]
    pub profile: String,
    /// Path to the fakescan executable (`ensure` uses this to start the process).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exe: Option<String>,
    /// When true, `ensure --autostart` installs a logon task for this row.
    #[serde(default)]
    pub autostart: bool,
    /// Harness config applied after the process is up (`/v1/pattern`, `/v1/pages`, …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness: Option<serde_json::Value>,
    /// Windows surfaces where this instance has been added (PnP bookkeeping).
    #[serde(default)]
    pub added: AddedInfo,
}

fn default_profile_id() -> String {
    "brother-wsd".into()
}

/// Which Windows install surfaces have this instance.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AddedInfo {
    /// True after a person completed "Add device" and WIA confirmed it.
    #[serde(default)]
    pub pnp: bool,
}

impl Registry {
    /// Load from [`registry_path`]; returns an empty registry if the file is absent.
    pub fn load() -> anyhow::Result<Self> {
        let path = registry_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let s = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&s)?)
    }

    /// Save to [`registry_path`], creating the directory tree as needed.
    pub fn save_to_disk(&self) -> anyhow::Result<()> {
        let path = registry_path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, text)?;
        Ok(())
    }

    pub fn find(&self, name: &str) -> Option<&DeviceRow> {
        self.devices.iter().find(|d| d.name == name)
    }

    pub fn find_mut(&mut self, name: &str) -> Option<&mut DeviceRow> {
        self.devices.iter_mut().find(|d| d.name == name)
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

    /// Validate that a port is in the fakescan even-port band (3920, 3922, …).
    pub fn validate_port(port: u16) -> anyhow::Result<()> {
        if !port.is_multiple_of(2) {
            bail!(
                "fakescan uses even ports (3920, 3922, …); {} is odd — \
                 odd ports (3921, 3923, …) are reserved for fakeprint",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_path_contains_fakescan() {
        let p = registry_path();
        assert!(
            p.to_string_lossy().contains("fakescan"),
            "path should contain fakescan: {p:?}"
        );
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
        reg.devices.push(DeviceRow {
            name: "default".into(),
            listen: "0.0.0.0:3920".into(),
            advertise: String::new(),
            device_uuid: "00000000-0000-4000-8000-0000000000f2".into(),
            profile: "brother-wsd".into(),
            exe: None,
            autostart: false,
            harness: None,
            added: AddedInfo::default(),
        });
        assert_eq!(reg.next_port(), 3922);
    }

    #[test]
    fn upsert_replaces_and_adds() {
        let mut reg = Registry::default();
        let row = DeviceRow {
            name: "a".into(),
            listen: "0.0.0.0:3920".into(),
            advertise: String::new(),
            device_uuid: "00000000-0000-4000-8000-0000000000f2".into(),
            profile: "brother-wsd".into(),
            exe: None,
            autostart: false,
            harness: None,
            added: AddedInfo::default(),
        };
        reg.upsert(row.clone());
        assert_eq!(reg.devices.len(), 1);
        let mut row2 = row;
        row2.listen = "0.0.0.0:3922".into();
        reg.upsert(row2);
        assert_eq!(reg.devices.len(), 1);
        assert_eq!(reg.find("a").unwrap().listen, "0.0.0.0:3922");
    }

    #[test]
    fn remove_by_name_works() {
        let mut reg = Registry::default();
        reg.devices.push(DeviceRow {
            name: "x".into(),
            listen: "0.0.0.0:3920".into(),
            advertise: String::new(),
            device_uuid: "00000000-0000-4000-8000-0000000000f2".into(),
            profile: "brother-wsd".into(),
            exe: None,
            autostart: false,
            harness: None,
            added: AddedInfo::default(),
        });
        assert!(reg.remove_by_name("x"));
        assert!(!reg.remove_by_name("x")); // idempotent
        assert!(reg.devices.is_empty());
    }
}
