//! Pure model behind the tray icon: summary level, tooltip, menu lines and the notification
//! transition detector. No platform code, so it is unit-tested on every platform.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use serde_json::Value;

use crate::server::DeviceInfo;

/// Minimum gap between two notifications for the same device.
pub const NOTIFY_INTERVAL: Duration = Duration::from_secs(600);

/// A `profile changed` stale must persist this long before the tray treats it as "needs re-Add".
/// A harness that flips the profile and back (daemon heals the row when the fingerprint matches
/// again) never raises a popup. The registry/API stay truthful: the row is stale immediately.
pub const PROFILE_STALE_GRACE: Duration = Duration::from_secs(20);

/// Windows truncates tooltips at 127 UTF-16 units; stay below.
const TOOLTIP_MAX: usize = 120;

/// Icon colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// Every device installed.
    Green,
    /// At least one device needs an Add or Re-Add.
    Amber,
    /// The manager could not be read.
    Red,
}

/// Install lifecycle, mirroring `registry::InstallState` without depending on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    New,
    Probed,
    Installed,
    Stale,
}

impl State {
    pub fn parse(s: &str) -> State {
        match s {
            "installed" => State::Installed,
            "probed" => State::Probed,
            "stale" => State::Stale,
            _ => State::New,
        }
    }
}

/// The few fields of a device the tray cares about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInput {
    pub id: String,
    pub state: State,
    pub needs_readd: bool,
    pub reason: Option<String>,
    /// How long the device has been stale (from `install.stale_at`); `None` = unknown/not stale.
    pub stale_age: Option<Duration>,
}

/// Seconds since the Unix epoch for `YYYY-MM-DDTHH:MM:SSZ`.
fn parse_iso_unix(s: &str) -> Option<u64> {
    let b = s.as_bytes();
    if b.len() != 20 || b[10] != b'T' || b[19] != b'Z' {
        return None;
    }
    let n = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, m, d) = (n(0..4)?, n(5..7)?, n(8..10)?);
    let (hh, mm, ss) = (n(11..13)?, n(14..16)?, n(17..19)?);
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u64::try_from(days * 86_400 + hh * 3600 + mm * 60 + ss).ok()
}

/// Age of an ISO timestamp relative to `now_unix` seconds (saturating at zero).
pub fn age_of_iso(iso: &str, now_unix: u64) -> Option<Duration> {
    parse_iso_unix(iso).map(|t| Duration::from_secs(now_unix.saturating_sub(t)))
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl DeviceInput {
    /// From one element of `GET /v1/devices`.
    pub fn from_json(v: &Value) -> Option<DeviceInput> {
        let id = v.get("id")?.as_str()?.to_string();
        let state = v
            .pointer("/install/state")
            .and_then(Value::as_str)
            .map(State::parse)
            .unwrap_or(State::New);
        let needs_readd = v
            .get("needs_readd")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let reason = v
            .get("needs_readd_reason")
            .and_then(Value::as_str)
            .or_else(|| v.pointer("/install/reason").and_then(Value::as_str))
            .map(str::to_string);
        let stale_age = v
            .pointer("/install/stale_at")
            .and_then(Value::as_str)
            .and_then(|t| age_of_iso(t, now_unix()));
        Some(DeviceInput {
            id,
            state,
            needs_readd,
            reason,
            stale_age,
        })
    }
}

impl From<&DeviceInfo> for DeviceInput {
    fn from(d: &DeviceInfo) -> Self {
        use crate::registry::InstallState as S;
        let stale_age = d
            .install
            .as_ref()
            .and_then(|i| i.stale_at.as_deref())
            .and_then(|t| age_of_iso(t, now_unix()));
        let (state, ireason) = match &d.install {
            Some(i) => (
                match i.state {
                    S::New => State::New,
                    S::Probed => State::Probed,
                    S::Installed => State::Installed,
                    S::Stale => State::Stale,
                },
                i.reason.clone(),
            ),
            None => (State::New, None),
        };
        DeviceInput {
            id: d.id.clone(),
            state,
            needs_readd: d.needs_readd,
            reason: d.needs_readd_reason.clone().or(ireason),
            stale_age,
        }
    }
}

/// One status line of the menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceLine {
    pub id: String,
    pub text: String,
    pub needs_attention: bool,
}

/// Counts shown in the tooltip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Counts {
    pub total: usize,
    pub installed: usize,
    pub need_add: usize,
    pub need_readd: usize,
    /// Registered and probably installed, but Windows is not listing it right now. Not a request
    /// for the human to Add anything: it never raises a notification.
    pub offline: usize,
    /// Profile-change stale younger than [`PROFILE_STALE_GRACE`]: may still flap back. Not yet a
    /// request for the human to re-Add; never raises a notification.
    pub settling: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    pub level: Level,
    pub counts: Counts,
    pub devices: Vec<DeviceLine>,
    /// Set when `level` is Red.
    pub error: Option<String>,
}

/// Reason the daemon records when a registered device is absent from Windows' live scanner list.
/// That list leaves out devices Windows holds but cannot reach at the moment, so this is evidence
/// of "offline", not of "never installed". Telling the human to Add again would be wrong.
const OFFLINE_REASON: &str = "not listed by Windows";

fn is_offline(d: &DeviceInput) -> bool {
    d.state == State::Stale
        && d.reason
            .as_deref()
            .is_some_and(|r| r.trim_start().starts_with(OFFLINE_REASON))
}

/// Stale because the profile changed, for less than the grace period (age unknown = old).
fn is_settling(d: &DeviceInput) -> bool {
    d.state == State::Stale
        && d.reason
            .as_deref()
            .is_some_and(|r| r.trim_start().starts_with("profile changed"))
        && d.stale_age.is_some_and(|a| a < PROFILE_STALE_GRACE)
}

fn is_readd(d: &DeviceInput) -> bool {
    !is_offline(d) && !is_settling(d) && (d.needs_readd || d.state == State::Stale)
}

fn needs_attention(d: &DeviceInput) -> bool {
    d.state != State::Installed || d.needs_readd
}

fn line_text(d: &DeviceInput) -> String {
    if is_settling(d) {
        format!("{}: profile changed, confirming", d.id)
    } else if is_offline(d) {
        format!("{}: offline (not listed by Windows)", d.id)
    } else if is_readd(d) {
        match d.reason.as_deref().filter(|r| !r.is_empty()) {
            Some(r) => format!("{}: needs re-Add ({})", d.id, r),
            None => format!("{}: needs re-Add", d.id),
        }
    } else if d.state == State::Installed {
        format!("{}: installed", d.id)
    } else {
        format!("{}: needs Add", d.id)
    }
}

impl Summary {
    pub fn from_devices(devices: &[DeviceInput]) -> Summary {
        let mut counts = Counts {
            total: devices.len(),
            ..Counts::default()
        };
        let mut lines = Vec::with_capacity(devices.len());
        for d in devices {
            let attn = needs_attention(d);
            let offline = is_offline(d);
            let settling = is_settling(d);
            if !attn {
                counts.installed += 1;
            } else if settling {
                counts.settling += 1;
            } else if offline {
                counts.offline += 1;
            } else if is_readd(d) {
                counts.need_readd += 1;
            } else {
                counts.need_add += 1;
            }
            // `needs_attention` means "a human must Add or re-Add this" (what notifications key on).
            lines.push(DeviceLine {
                id: d.id.clone(),
                text: line_text(d),
                needs_attention: attn && !offline && !settling,
            });
        }
        let level = if counts.need_add + counts.need_readd + counts.offline + counts.settling > 0 {
            Level::Amber
        } else {
            Level::Green
        };
        Summary {
            level,
            counts,
            devices: lines,
            error: None,
        }
    }

    /// The manager could not be read.
    pub fn error(msg: impl Into<String>) -> Summary {
        Summary {
            level: Level::Red,
            counts: Counts::default(),
            devices: Vec::new(),
            error: Some(msg.into()),
        }
    }

    pub fn tooltip(&self) -> String {
        let c = &self.counts;
        let s = match self.level {
            Level::Red => format!(
                "fakescan: error ({})",
                self.error.as_deref().unwrap_or("unknown")
            ),
            _ if c.total == 0 => "fakescan: no devices".to_string(),
            Level::Green => format!(
                "fakescan: {} device{}, all installed",
                c.total,
                plural(c.total)
            ),
            Level::Amber => {
                let mut parts = Vec::new();
                if c.need_add > 0 {
                    parts.push(format!("{} need Add", c.need_add));
                }
                if c.need_readd > 0 {
                    parts.push(format!("{} need re-Add", c.need_readd));
                }
                if c.offline > 0 {
                    parts.push(format!("{} offline", c.offline));
                }
                if c.settling > 0 {
                    parts.push(format!("{} settling", c.settling));
                }
                format!(
                    "fakescan: {} device{}, {} installed, {}",
                    c.total,
                    plural(c.total),
                    c.installed,
                    parts.join(", ")
                )
            }
        };
        truncate(&s, TOOLTIP_MAX)
    }

    /// Ids of devices needing an Add or Re-Add.
    fn attention_ids(&self) -> Vec<&str> {
        self.devices
            .iter()
            .filter(|d| d.needs_attention)
            .map(|d| d.id.as_str())
            .collect()
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max - 1).collect();
    out.push('\u{2026}');
    out
}

/// Parse a raw HTTP/1.x response to `GET /v1/devices` into a summary (Red on any problem).
pub fn summary_from_http_response(raw: &[u8]) -> Summary {
    let Some(split) = raw.windows(4).position(|w| w == b"\r\n\r\n") else {
        return Summary::error("no HTTP response");
    };
    let head = String::from_utf8_lossy(&raw[..split]);
    let status = head.lines().next().unwrap_or("");
    if !status.split_whitespace().nth(1).is_some_and(|c| c == "200") {
        return Summary::error(format!("HTTP {status}"));
    }
    match serde_json::from_slice::<Value>(&raw[split + 4..]) {
        Ok(Value::Array(a)) => {
            let devs: Vec<DeviceInput> = a.iter().filter_map(DeviceInput::from_json).collect();
            Summary::from_devices(&devs)
        }
        Ok(_) => Summary::error("unexpected /v1/devices body"),
        Err(e) => Summary::error(format!("bad JSON: {e}")),
    }
}

/// A notification to raise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    /// Devices that triggered it.
    pub devices: Vec<String>,
    /// One-line detail text (for the balloon body).
    pub detail: String,
    /// True for the one-off start-up summary.
    pub startup: bool,
}

/// Detects devices newly entering needs-Add/Re-Add between summaries. Time is injected
/// (`now` = any monotonic offset) so tests need no clock.
#[derive(Debug, Default)]
pub struct TransitionDetector {
    started: bool,
    attention: HashSet<String>,
    last_notified: HashMap<String, Duration>,
    interval: Option<Duration>,
}

impl TransitionDetector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_interval(interval: Duration) -> Self {
        Self {
            interval: Some(interval),
            ..Self::default()
        }
    }

    fn interval(&self) -> Duration {
        self.interval.unwrap_or(NOTIFY_INTERVAL)
    }

    /// Feed the latest summary; returns a notification when one is due. An error summary changes
    /// nothing (the previous attention set stays, so a flapping error never re-notifies).
    pub fn observe(&mut self, s: &Summary, now: Duration) -> Option<Notification> {
        if s.level == Level::Red {
            return None;
        }
        let cur: Vec<&str> = s.attention_ids();
        let first = !self.started;
        self.started = true;

        let mut fire: Vec<String> = Vec::new();
        for id in &cur {
            if self.attention.contains(*id) {
                continue;
            }
            let limited = self
                .last_notified
                .get(*id)
                .is_some_and(|t| now.saturating_sub(*t) < self.interval());
            if !limited {
                fire.push((*id).to_string());
            }
        }
        self.attention = cur.iter().map(|s| s.to_string()).collect();
        for id in &fire {
            self.last_notified.insert(id.clone(), now);
        }
        if fire.is_empty() {
            return None;
        }
        let detail = if first {
            format!(
                "{} device{} need{} an Add in Windows: {}.",
                fire.len(),
                plural(fire.len()),
                if fire.len() == 1 { "s" } else { "" },
                fire.join(", ")
            )
        } else {
            format!("Add or re-Add needed: {}.", fire.join(", "))
        };
        Some(Notification {
            devices: fire,
            detail,
            startup: first,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(id: &str, state: State) -> DeviceInput {
        DeviceInput {
            id: id.into(),
            state,
            needs_readd: false,
            reason: None,
            stale_age: None,
        }
    }
    fn readd(id: &str, reason: &str) -> DeviceInput {
        DeviceInput {
            id: id.into(),
            state: State::Installed,
            needs_readd: true,
            reason: Some(reason.into()),
            stale_age: None,
        }
    }
    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    #[test]
    fn a_device_missing_from_the_windows_list_is_offline_not_a_request_to_add() {
        let mut d = dev("default", State::Stale);
        d.needs_readd = true;
        d.reason = Some("not listed by Windows".into());
        let s = Summary::from_devices(&[d]);
        assert_eq!(
            s.devices[0].text,
            "default: offline (not listed by Windows)"
        );
        assert!(
            !s.devices[0].needs_attention,
            "offline must not read as 'Add this'"
        );
        assert_eq!(
            (s.counts.offline, s.counts.need_readd, s.counts.need_add),
            (1, 0, 0)
        );
        assert_eq!(s.level, Level::Amber);
        assert!(s.tooltip().contains("1 offline"), "{}", s.tooltip());
        // No notification, not even the start-up summary.
        let mut det = TransitionDetector::new();
        assert_eq!(det.observe(&s, Duration::from_secs(0)), None);
        // A stale device for any OTHER reason still needs a real re-Add.
        let mut other = dev("glass", State::Stale);
        other.needs_readd = true;
        other.reason = Some("profile changed; remove and re-Add in Imaging".into());
        let s2 = Summary::from_devices(&[other]);
        assert!(s2.devices[0].needs_attention);
        assert_eq!(s2.counts.need_readd, 1);
    }

    #[test]
    fn levels() {
        assert_eq!(Summary::from_devices(&[]).level, Level::Green);
        let g = Summary::from_devices(&[dev("a", State::Installed)]);
        assert_eq!(g.level, Level::Green);
        for st in [State::New, State::Probed, State::Stale] {
            assert_eq!(
                Summary::from_devices(&[dev("a", State::Installed), dev("b", st)]).level,
                Level::Amber
            );
        }
        assert_eq!(
            Summary::from_devices(&[readd("a", "profile changed")]).level,
            Level::Amber
        );
        assert_eq!(Summary::error("boom").level, Level::Red);
    }

    #[test]
    fn menu_lines() {
        let s = Summary::from_devices(&[
            dev("default", State::Installed),
            dev("tray5", State::New),
            dev("seen", State::Probed),
            readd("glass", "profile changed"),
            dev("old", State::Stale),
        ]);
        let t: Vec<&str> = s.devices.iter().map(|d| d.text.as_str()).collect();
        assert_eq!(
            t,
            [
                "default: installed",
                "tray5: needs Add",
                "seen: needs Add",
                "glass: needs re-Add (profile changed)",
                "old: needs re-Add"
            ]
        );
        assert_eq!(
            s.counts,
            Counts {
                total: 5,
                installed: 1,
                need_add: 2,
                need_readd: 2,
                offline: 0,
                settling: 0
            }
        );
    }

    #[test]
    fn tooltips() {
        assert_eq!(Summary::from_devices(&[]).tooltip(), "fakescan: no devices");
        assert_eq!(
            Summary::from_devices(&[dev("a", State::Installed)]).tooltip(),
            "fakescan: 1 device, all installed"
        );
        let s = Summary::from_devices(&[
            dev("a", State::Installed),
            dev("b", State::New),
            readd("c", "x"),
        ]);
        assert_eq!(
            s.tooltip(),
            "fakescan: 3 devices, 1 installed, 1 need Add, 1 need re-Add"
        );
        assert_eq!(
            Summary::error("manager down").tooltip(),
            "fakescan: error (manager down)"
        );
        let long = Summary::error("x".repeat(500)).tooltip();
        assert!(long.chars().count() <= 120);
    }

    #[test]
    fn from_json() {
        let v: Value = serde_json::json!({
            "id": "tray5", "needs_readd": true, "needs_readd_reason": "profile changed",
            "install": {"state": "installed"}
        });
        let d = DeviceInput::from_json(&v).unwrap();
        assert_eq!(d, readd("tray5", "profile changed"));
        let v: Value = serde_json::json!({"id": "x"});
        assert_eq!(DeviceInput::from_json(&v).unwrap().state, State::New);
        assert!(DeviceInput::from_json(&serde_json::json!({})).is_none());
    }

    #[test]
    fn http_response_parsing() {
        let ok = b"HTTP/1.0 200 OK\r\ncontent-type: application/json\r\n\r\n[{\"id\":\"a\",\"install\":{\"state\":\"installed\"}},{\"id\":\"b\"}]";
        let s = summary_from_http_response(ok);
        assert_eq!(s.level, Level::Amber);
        assert_eq!(s.devices.len(), 2);
        assert_eq!(
            summary_from_http_response(b"HTTP/1.0 500 x\r\n\r\n[]").level,
            Level::Red
        );
        assert_eq!(summary_from_http_response(b"garbage").level, Level::Red);
        assert_eq!(
            summary_from_http_response(b"HTTP/1.0 200 OK\r\n\r\n{}").level,
            Level::Red
        );
    }

    #[test]
    fn startup_summary_only_if_needed() {
        let mut d = TransitionDetector::new();
        assert!(d
            .observe(
                &Summary::from_devices(&[dev("a", State::Installed)]),
                secs(0)
            )
            .is_none());

        let mut d = TransitionDetector::new();
        let n = d
            .observe(
                &Summary::from_devices(&[dev("a", State::New), dev("b", State::Stale)]),
                secs(0),
            )
            .unwrap();
        assert!(n.startup);
        assert_eq!(n.devices, ["a", "b"]);
        assert!(n.detail.contains("2 devices") && n.detail.contains("a, b"));
    }

    #[test]
    fn no_repeat_and_renotify_after_leaving() {
        let mut d = TransitionDetector::with_interval(secs(600));
        let ok = Summary::from_devices(&[dev("a", State::Installed)]);
        let bad = Summary::from_devices(&[readd("a", "profile changed")]);
        assert!(d.observe(&ok, secs(0)).is_none());
        let n = d.observe(&bad, secs(10)).unwrap();
        assert!(!n.startup);
        assert_eq!(n.devices, ["a"]);
        assert!(d.observe(&bad, secs(20)).is_none());
        assert!(d.observe(&bad, secs(5000)).is_none());
        assert!(d.observe(&ok, secs(5001)).is_none());
        assert!(d.observe(&bad, secs(5002)).is_some());
    }

    #[test]
    fn rate_limit_per_device() {
        let mut d = TransitionDetector::with_interval(secs(600));
        let ok = Summary::from_devices(&[dev("a", State::Installed), dev("b", State::Installed)]);
        let a_bad = Summary::from_devices(&[dev("a", State::Stale), dev("b", State::Installed)]);
        let both = Summary::from_devices(&[dev("a", State::Stale), dev("b", State::Stale)]);
        d.observe(&ok, secs(0));
        assert!(d.observe(&a_bad, secs(100)).is_some());
        // a flaps back and re-enters within 10 min: suppressed.
        d.observe(&ok, secs(110));
        assert!(d.observe(&a_bad, secs(120)).is_none());
        // b enters (not rate limited) while a is already in attention: only b reported.
        let n = d.observe(&both, secs(130)).unwrap();
        assert_eq!(n.devices, ["b"]);
        // a flaps again after its window has passed.
        d.observe(&ok, secs(140));
        let n = d.observe(&a_bad, secs(100 + 601)).unwrap();
        assert_eq!(n.devices, ["a"]);
    }

    fn profile_stale(id: &str, age_s: u64) -> DeviceInput {
        DeviceInput {
            id: id.into(),
            state: State::Stale,
            needs_readd: true,
            reason: Some("profile changed; remove and re-Add in Imaging".into()),
            stale_age: Some(secs(age_s)),
        }
    }

    #[test]
    fn profile_flap_shorter_than_grace_never_notifies() {
        let mut d = TransitionDetector::new();
        let ok = Summary::from_devices(&[dev("a", State::Installed)]);
        assert!(d.observe(&ok, secs(0)).is_none());
        for (t, age) in [(3, 3), (15, 15)] {
            let s = Summary::from_devices(&[profile_stale("a", age)]);
            assert_eq!(s.counts.settling, 1);
            assert!(!s.devices[0].needs_attention);
            assert!(
                s.devices[0].text.contains("confirming"),
                "{}",
                s.devices[0].text
            );
            assert!(d.observe(&s, secs(t)).is_none());
        }
        // Healed before the grace period ends: still nothing.
        assert!(d.observe(&ok, secs(18)).is_none());
    }

    #[test]
    fn persistent_profile_stale_notifies_once_after_grace() {
        let mut d = TransitionDetector::new();
        assert!(d
            .observe(
                &Summary::from_devices(&[dev("a", State::Installed)]),
                secs(0)
            )
            .is_none());
        assert!(d
            .observe(&Summary::from_devices(&[profile_stale("a", 5)]), secs(5))
            .is_none());
        let n = d
            .observe(&Summary::from_devices(&[profile_stale("a", 20)]), secs(20))
            .unwrap();
        assert_eq!(n.devices, ["a"]);
        assert!(!n.startup);
        assert!(d
            .observe(&Summary::from_devices(&[profile_stale("a", 30)]), secs(30))
            .is_none());
    }

    #[test]
    fn grace_applies_only_to_profile_changed_and_unknown_age_is_old() {
        let mut other = profile_stale("a", 1);
        other.reason = Some("something else".into());
        assert!(Summary::from_devices(&[other]).devices[0].needs_attention);
        let mut old = profile_stale("a", 0);
        old.stale_age = None;
        assert!(Summary::from_devices(&[old]).devices[0].needs_attention);
    }

    #[test]
    fn iso_age_parsing() {
        assert_eq!(age_of_iso("1970-01-01T00:00:10Z", 25), Some(secs(15)));
        assert_eq!(
            age_of_iso("2026-01-01T00:00:00Z", 1_767_225_600 + 7),
            Some(secs(7))
        );
        assert_eq!(
            age_of_iso("2020-02-29T00:00:00Z", 1_582_934_400 + 60),
            Some(secs(60))
        );
        assert_eq!(age_of_iso("garbage", 5), None);
        assert_eq!(age_of_iso("2026-01-01T00:00:00Z", 0), Some(secs(0)));
    }

    #[test]
    fn error_summary_changes_nothing() {
        let mut d = TransitionDetector::new();
        let bad = Summary::from_devices(&[dev("a", State::New)]);
        assert!(d.observe(&bad, secs(0)).is_some());
        assert!(d.observe(&Summary::error("x"), secs(1)).is_none());
        assert!(d.observe(&bad, secs(2)).is_none());
    }
}
