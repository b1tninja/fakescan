//! General fault plan: schedule any fault kind between pages, mid page, timed or live.
//!
//! One mechanism behind `POST /v1/faults/plan`. The older one-off counters
//! (`jam_after_n`, `offline_after_n`, ...) keep working unchanged and are listed in
//! `Status.fault_plan` as read-only `legacy` rows; `cover_open_after_n` is real sugar
//! (it inserts a plan entry).
//!
//! WS-Scan subcodes (docs/reference/faults.md): `ClientErrorMediaJam`, `ClientErrorCoverOpen`,
//! `ClientErrorInterlockOpen`, `ClientErrorMultipleFeed` are the lab/driver extension family
//! the repo already uses for sticky conditions (the XSD `ScanServiceFaultValuesType` has no
//! cover/door value); `ClientErrorNoImagesAvailable` (XSD) for empty; `ServerErrorNotAcceptingJobs`
//! (XSD) for offline / lamp error / busy. Conditions use the same Ids as `elements.rs`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{ConditionChange, World};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FaultKind {
    Jam,
    #[serde(alias = "cover")]
    CoverOpen,
    #[serde(alias = "interlock")]
    InterlockOpen,
    #[serde(alias = "double_feed")]
    MultiFeed,
    #[serde(alias = "no_media", alias = "empty_feeder")]
    Empty,
    Offline,
    LampError,
    Busy,
    /// Transport only: the MTOM stream is cut, no scanner condition.
    #[serde(alias = "abort_retrieve", alias = "cut")]
    Abort,
}

impl FaultKind {
    pub fn name(self) -> &'static str {
        match self {
            FaultKind::Jam => "jam",
            FaultKind::CoverOpen => "cover_open",
            FaultKind::InterlockOpen => "interlock_open",
            FaultKind::MultiFeed => "multi_feed",
            FaultKind::Empty => "empty",
            FaultKind::Offline => "offline",
            FaultKind::LampError => "lamp_error",
            FaultKind::Busy => "busy",
            FaultKind::Abort => "abort",
        }
    }

    /// Sticky by default for real device conditions; transient for busy / empty / abort.
    pub fn default_sticky(self) -> bool {
        !matches!(self, FaultKind::Busy | FaultKind::Empty | FaultKind::Abort)
    }

    /// SOAP fault reason (substring-mapped to a `wscn:` subcode in `wsd/mod.rs`).
    pub fn reason(self) -> &'static str {
        match self {
            FaultKind::Jam => "ClientErrorMediaJam: MediaJam",
            FaultKind::CoverOpen => "ClientErrorCoverOpen: CoverOpen",
            FaultKind::InterlockOpen => "ClientErrorInterlockOpen: InterlockOpen",
            FaultKind::MultiFeed => "ClientErrorMultipleFeed: MultipleFeedError",
            FaultKind::Empty => "ClientErrorNoImagesAvailable: feeder empty",
            FaultKind::Offline => "ServerErrorNotAcceptingJobs: device offline",
            FaultKind::LampError => "ServerErrorNotAcceptingJobs: LampError",
            FaultKind::Busy => "ServerErrorNotAcceptingJobs: device busy",
            FaultKind::Abort => "connection reset",
        }
    }

    /// DeviceCondition (Id, Name, Component, Severity); Id 0 = AttentionRequired only.
    fn condition(self) -> Option<(u32, &'static str, &'static str, &'static str)> {
        match self {
            FaultKind::Jam => Some((1, "MediaJam", "ADF", "Critical")),
            FaultKind::CoverOpen => Some((2, "CoverOpen", "Platen", "Critical")),
            FaultKind::MultiFeed => Some((3, "MultipleFeedError", "ADF", "Critical")),
            FaultKind::LampError => Some((4, "LampError", "Platen", "Critical")),
            FaultKind::InterlockOpen => Some((6, "InterlockOpen", "MediaPath", "Critical")),
            FaultKind::Offline => Some((0, "AttentionRequired", "", "")),
            _ => None,
        }
    }
}

/// Where a mid-page cut lands in the MTOM body.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Cut {
    Bytes(u64),
    Fraction(f64),
}

impl Cut {
    /// Bytes sent before the stream errors: at least 1, always less than the full body.
    pub fn len(self, body_len: usize) -> usize {
        if body_len <= 1 {
            return body_len.saturating_sub(1);
        }
        let n = match self {
            Cut::Bytes(b) => b as usize,
            Cut::Fraction(f) => ((body_len as f64) * f.clamp(0.0, 1.0)) as usize,
        };
        n.clamp(1, body_len - 1)
    }

    fn describe(self) -> String {
        match self {
            Cut::Bytes(b) => format!("@ {b} bytes"),
            Cut::Fraction(f) => format!("@ {}%", (f * 100.0).round() as u32),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    Create,
    FirstRetrieve,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Trigger {
    Now,
    AfterPages(u32),
    /// `page` is 1-based (the page being streamed).
    MidPage {
        page: u32,
        cut: Cut,
    },
    AfterMs {
        ms: u64,
        from: Anchor,
    },
}

impl Trigger {
    pub fn to_json(self) -> Value {
        match self {
            Trigger::Now => json!("now"),
            Trigger::AfterPages(n) => json!({ "after_pages": n }),
            Trigger::MidPage {
                page,
                cut: Cut::Bytes(b),
            } => json!({ "page": page, "bytes": b }),
            Trigger::MidPage {
                page,
                cut: Cut::Fraction(f),
            } => {
                json!({ "page": page, "fraction": f })
            }
            Trigger::AfterMs { ms, from } => json!({
                "after_ms": ms,
                "from": if from == Anchor::Create { "create" } else { "first_retrieve" },
            }),
        }
    }

    pub fn describe(self) -> String {
        match self {
            Trigger::Now => "now".into(),
            Trigger::AfterPages(n) => format!("after {n} pages"),
            Trigger::MidPage { page, cut } => format!("mid page {page} {}", cut.describe()),
            Trigger::AfterMs { ms, from } => format!(
                "after {ms} ms from {}",
                if from == Anchor::Create {
                    "create"
                } else {
                    "first retrieve"
                }
            ),
        }
    }
}

// ---- input types -------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum WhenIn {
    Word(String),
    Obj(WhenObj),
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct WhenObj {
    pub after_pages: Option<u32>,
    pub page: Option<u32>,
    pub bytes: Option<u64>,
    pub fraction: Option<f64>,
    pub after_ms: Option<u64>,
    /// `create` (default) or `first_retrieve`, for `after_ms`.
    pub from: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlanIn {
    pub kind: FaultKind,
    #[serde(default)]
    pub when: Option<WhenIn>,
    pub sticky: Option<bool>,
    pub one_shot: Option<bool>,
    /// Only this JobId (default: any job).
    pub job: Option<i32>,
    /// Only while retrieving pages in this inclusive 1-based range.
    pub pages: Option<[u32; 2]>,
}

/// `POST /v1/faults/plan` body: `{faults:[...]}` or one bare entry.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum PlanBody {
    Many { faults: Vec<PlanIn> },
    One(PlanIn),
}

impl PlanBody {
    pub fn into_entries(self) -> Vec<PlanIn> {
        match self {
            PlanBody::Many { faults } => faults,
            PlanBody::One(p) => vec![p],
        }
    }
}

fn parse_when(w: Option<&WhenIn>) -> Result<Trigger> {
    let obj = match w {
        None => return Ok(Trigger::Now),
        Some(WhenIn::Word(s)) => {
            if s.trim().eq_ignore_ascii_case("now") {
                return Ok(Trigger::Now);
            }
            bail!("unknown when {s:?}; expected \"now\" or an object");
        }
        Some(WhenIn::Obj(o)) => o,
    };
    if let Some(n) = obj.after_pages {
        return Ok(Trigger::AfterPages(n));
    }
    if let Some(page) = obj.page {
        if page == 0 {
            bail!("when.page is 1-based");
        }
        if let Some(b) = obj.bytes {
            return Ok(Trigger::MidPage {
                page,
                cut: Cut::Bytes(b),
            });
        }
        if let Some(f) = obj.fraction {
            if !(f > 0.0 && f < 1.0) {
                bail!("when.fraction must be in (0,1)");
            }
            return Ok(Trigger::MidPage {
                page,
                cut: Cut::Fraction(f),
            });
        }
        bail!("when.page needs bytes or fraction");
    }
    if let Some(ms) = obj.after_ms {
        let from = match obj.from.as_deref() {
            None | Some("create") => Anchor::Create,
            Some("first_retrieve") | Some("retrieve") => Anchor::FirstRetrieve,
            Some(o) => bail!("when.from {o:?}: expected create or first_retrieve"),
        };
        return Ok(Trigger::AfterMs { ms, from });
    }
    bail!("when: need after_pages, page+bytes|fraction, after_ms, or \"now\"")
}

// ---- stored entries ----------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PlanEntry {
    pub id: u32,
    pub kind: FaultKind,
    pub trigger: Trigger,
    pub sticky: bool,
    pub one_shot: bool,
    pub job: Option<i32>,
    pub pages: Option<(u32, u32)>,
    pub legacy_cover: bool,
    pub fired_count: u32,
    pub last_fired: Option<String>,
}

impl PlanEntry {
    fn pending(&self) -> bool {
        !(self.one_shot && self.fired_count > 0)
    }

    fn matches_job(&self, job_id: i32) -> bool {
        self.job.is_none_or(|j| j == job_id)
    }

    fn matches_page(&self, page_no: u32) -> bool {
        self.pages.is_none_or(|(a, b)| page_no >= a && page_no <= b)
    }
}

/// One row of `Status.fault_plan`.
#[derive(Debug, Clone, Serialize)]
pub struct PlanEntryView {
    pub id: u32,
    pub kind: &'static str,
    pub when: Value,
    pub trigger: String,
    pub sticky: bool,
    pub one_shot: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pages: Option<[u32; 2]>,
    pub fired: bool,
    pub fired_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_fired: Option<String>,
    /// Read-only row synthesised from a `*_after_n` counter.
    pub legacy: bool,
}

/// Per-stream handle: cut when the epoch moves (live fault) or a deadline passes.
#[derive(Debug, Clone)]
pub struct LiveCut {
    pub counter: Arc<AtomicU64>,
    pub epoch: u64,
    pub deadline: Option<Instant>,
}

impl LiveCut {
    pub fn tripped(&self) -> bool {
        self.counter.load(Ordering::SeqCst) != self.epoch
            || self.deadline.is_some_and(|d| Instant::now() >= d)
    }
}

/// Result of the RetrieveImage pre-check.
#[derive(Debug, Default)]
pub(super) struct Gate {
    pub fault: Option<String>,
    /// (plan entry id or 0, cut) for the page about to stream.
    pub mid: Option<(u32, Cut)>,
}

#[derive(Debug)]
pub struct PlanState {
    pub entries: Vec<PlanEntry>,
    next_id: u32,
    skew: Duration,
    armed: Vec<FaultKind>,
    pub live: Arc<AtomicU64>,
    pub pending_cut: Option<Cut>,
    first_retrieve: std::collections::HashMap<i32, Instant>,
}

impl Default for PlanState {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            next_id: 1,
            skew: Duration::ZERO,
            armed: Vec::new(),
            live: Arc::new(AtomicU64::new(0)),
            pending_cut: None,
            first_retrieve: Default::default(),
        }
    }
}

impl PlanState {
    pub fn clear(&mut self) {
        self.entries.clear();
        self.armed.clear();
        self.pending_cut = None;
        self.first_retrieve.clear();
        self.skew = Duration::ZERO;
    }
}

impl World {
    /// Plan clock: real time plus any test skew.
    pub fn plan_clock(&self) -> Instant {
        Instant::now() + self.plan.skew
    }

    /// Test hook: move the plan clock forward.
    pub fn advance_plan_clock(&mut self, d: Duration) {
        self.plan.skew += d;
    }

    pub fn live_cut_handle(&self, job_id: i32) -> LiveCut {
        LiveCut {
            counter: self.plan.live.clone(),
            epoch: self.plan.live.load(Ordering::SeqCst),
            deadline: self.next_timed_deadline(job_id),
        }
    }

    /// Whether MTOM streaming should use the chunked (cuttable) path.
    pub fn plan_has_pending(&self) -> bool {
        self.plan.entries.iter().any(|e| e.pending()) || !self.plan.armed.is_empty()
    }

    pub fn take_pending_cut(&mut self) -> Option<Cut> {
        self.plan.pending_cut.take()
    }

    /// Append entries. Validation happens before anything is stored. `now` entries apply at once.
    pub fn add_fault_plan(&mut self, body: PlanBody) -> Result<Vec<u32>> {
        let mut staged = Vec::new();
        for p in body.into_entries() {
            let trigger = parse_when(p.when.as_ref())?;
            if let Some([a, b]) = p.pages {
                if a == 0 || b < a {
                    bail!("pages must be [from,to] 1-based, from<=to");
                }
            }
            staged.push((p, trigger));
        }
        let mut ids = Vec::new();
        for (p, trigger) in staged {
            let id = self.plan.next_id;
            self.plan.next_id += 1;
            self.plan.entries.push(PlanEntry {
                id,
                kind: p.kind,
                trigger,
                sticky: p.sticky.unwrap_or_else(|| p.kind.default_sticky()),
                one_shot: p.one_shot.unwrap_or(true),
                job: p.job,
                pages: p.pages.map(|[a, b]| (a, b)),
                legacy_cover: false,
                fired_count: 0,
                last_fired: None,
            });
            ids.push(id);
        }
        self.tick_fault_plan();
        Ok(ids)
    }

    pub fn clear_fault_plan(&mut self) {
        self.plan.clear();
    }

    /// `cover_open_after_n` sugar: replace the legacy entry (0 removes it).
    pub(super) fn set_cover_open_after_n(&mut self, n: u32) {
        self.plan.entries.retain(|e| !e.legacy_cover);
        if n == 0 {
            return;
        }
        let id = self.plan.next_id;
        self.plan.next_id += 1;
        self.plan.entries.push(PlanEntry {
            id,
            kind: FaultKind::CoverOpen,
            trigger: Trigger::AfterPages(n),
            sticky: true,
            one_shot: true,
            job: None,
            pages: None,
            legacy_cover: true,
            fired_count: 0,
            last_fired: None,
        });
    }

    pub fn cover_open_after_n(&self) -> u32 {
        self.plan
            .entries
            .iter()
            .find(|e| e.legacy_cover && e.pending())
            .and_then(|e| match e.trigger {
                Trigger::AfterPages(n) => Some(n),
                _ => None,
            })
            .unwrap_or(0)
    }

    pub fn fault_plan_view(&self) -> Vec<PlanEntryView> {
        let mut out: Vec<PlanEntryView> = self
            .plan
            .entries
            .iter()
            .map(|e| PlanEntryView {
                id: e.id,
                kind: e.kind.name(),
                when: e.trigger.to_json(),
                trigger: e.trigger.describe(),
                sticky: e.sticky,
                one_shot: e.one_shot,
                job: e.job,
                pages: e.pages.map(|(a, b)| [a, b]),
                fired: e.fired_count > 0,
                fired_count: e.fired_count,
                last_fired: e.last_fired.clone(),
                legacy: false,
            })
            .collect();
        let mut legacy = |kind: FaultKind, n: u32, fired: bool, what: &str| {
            if n > 0 {
                out.push(PlanEntryView {
                    id: 0,
                    kind: kind.name(),
                    when: Trigger::AfterPages(n).to_json(),
                    trigger: format!("after {n} pages ({what})"),
                    sticky: kind.default_sticky(),
                    one_shot: true,
                    job: None,
                    pages: None,
                    fired,
                    fired_count: u32::from(fired),
                    last_fired: None,
                    legacy: true,
                });
            }
        };
        legacy(FaultKind::Jam, self.jam_after_n, self.jam, "jam_after_n");
        legacy(
            FaultKind::Offline,
            self.offline_after_n,
            self.offline,
            "offline_after_n",
        );
        legacy(
            FaultKind::MultiFeed,
            self.multi_feed_after_n,
            self.multi_feed,
            "multi_feed_after_n",
        );
        legacy(FaultKind::Empty, self.empty_mid_job, false, "empty_mid_job");
        legacy(
            FaultKind::Abort,
            self.abort_retrieve_after_n,
            self.abort_retrieve,
            "abort_retrieve_after_n",
        );
        out
    }

    fn flag(&self, k: FaultKind) -> bool {
        match k {
            FaultKind::Jam => self.jam,
            FaultKind::CoverOpen => self.cover_open,
            FaultKind::InterlockOpen => self.interlock_open,
            FaultKind::MultiFeed => self.multi_feed,
            FaultKind::Offline => self.offline,
            FaultKind::LampError => self.lamp_error,
            FaultKind::Busy => self.busy,
            _ => false,
        }
    }

    fn set_flag(&mut self, k: FaultKind, v: bool) {
        match k {
            FaultKind::Jam => self.jam = v,
            FaultKind::CoverOpen => self.cover_open = v,
            FaultKind::InterlockOpen => self.interlock_open = v,
            FaultKind::MultiFeed => self.multi_feed = v,
            FaultKind::Offline => self.offline = v,
            FaultKind::LampError => self.lamp_error = v,
            FaultKind::Busy => self.busy = v,
            _ => {}
        }
    }

    fn raise_kind(&mut self, k: FaultKind, mark_stopped: bool) {
        let Some((id, name, component, severity)) = k.condition() else {
            return;
        };
        if self.flag(k) {
            return;
        }
        if id > 0 {
            self.record_condition_transition(id, name, component, severity, true);
        }
        self.set_flag(k, true);
        if mark_stopped {
            self.mark_jobs_scanner_stopped();
        }
        self.enqueue_condition_change(ConditionChange {
            id,
            name,
            component,
            severity,
            raised: true,
        });
    }

    fn clear_kind(&mut self, k: FaultKind) {
        let Some((id, name, component, severity)) = k.condition() else {
            return;
        };
        if !self.flag(k) {
            return;
        }
        if id > 0 {
            self.record_condition_transition(id, name, component, severity, false);
        }
        self.set_flag(k, false);
        self.enqueue_condition_change(ConditionChange {
            id,
            name,
            component,
            severity,
            raised: false,
        });
    }

    /// Empty/no-media: the job ends early, completed with errors.
    pub(super) fn end_job_empty(&mut self, job_id: i32) {
        let snap = match self.jobs.get_mut(&job_id) {
            Some(job) if job.pages_left > 0 => {
                job.pages_left = 0;
                job.state_reason = "JobCompletedWithErrors".into();
                Some(job.clone())
            }
            _ => None,
        };
        if let Some(s) = snap {
            self.push_job_history(&s, "Completed", "JobCompletedWithErrors");
        }
    }

    fn any_active_job(&self) -> bool {
        self.jobs.values().any(|j| !j.aborted && j.pages_left > 0)
    }

    /// Apply a fired entry's side effects. `arm_next`: firing outside a RetrieveImage, so also
    /// make the next RetrieveImage fail (and cut any stream in flight).
    fn fire_entry(&mut self, idx: usize, job_id: Option<i32>, arm_next: bool, where_: &str) {
        let (kind, sticky, id) = {
            let e = &mut self.plan.entries[idx];
            e.fired_count += 1;
            e.last_fired = Some(where_.to_string());
            (e.kind, e.sticky, e.id)
        };
        let msg = format!("fault fired: {} {where_} (plan #{id})", kind.name());
        self.note("fault", "plan", "fired", &msg);
        match kind {
            FaultKind::Empty => {
                if let Some(j) = job_id {
                    self.end_job_empty(j);
                } else if arm_next {
                    self.plan.armed.push(kind);
                }
            }
            FaultKind::Abort | FaultKind::Busy => {
                if arm_next {
                    self.plan.armed.push(kind);
                }
            }
            k => {
                if sticky {
                    self.raise_kind(k, true);
                } else {
                    // Transient: the condition blips (history + NotifyTo) and clears.
                    self.raise_kind(k, false);
                    self.clear_kind(k);
                    if arm_next {
                        self.plan.armed.push(k);
                    }
                }
            }
        }
        if arm_next && self.any_active_job() {
            self.plan.live.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn timed_due(&self, e: &PlanEntry, now: Instant) -> Option<i32> {
        let Trigger::AfterMs { ms, from } = e.trigger else {
            return None;
        };
        let mut ids: Vec<i32> = self.jobs.keys().copied().collect();
        ids.sort_unstable();
        ids.into_iter().find(|id| {
            let Some(j) = self.jobs.get(id) else {
                return false;
            };
            if j.aborted || j.pages_left == 0 || !e.matches_job(*id) {
                return false;
            }
            let anchor = match from {
                Anchor::Create => Some(j.created_at),
                Anchor::FirstRetrieve => self.plan.first_retrieve.get(id).copied(),
            };
            anchor.is_some_and(|a| now >= a + Duration::from_millis(ms))
        })
    }

    /// Earliest real-time deadline of a pending timed entry for `job_id` (in-flight stream cut).
    pub(super) fn next_timed_deadline(&self, job_id: i32) -> Option<Instant> {
        let j = self.jobs.get(&job_id)?;
        let skew = self.plan.skew;
        self.plan
            .entries
            .iter()
            .filter(|e| e.pending() && e.matches_job(job_id))
            .filter_map(|e| match e.trigger {
                Trigger::AfterMs { ms, from } => {
                    let a = match from {
                        Anchor::Create => Some(j.created_at),
                        Anchor::FirstRetrieve => self.plan.first_retrieve.get(&job_id).copied(),
                    }?;
                    (a + Duration::from_millis(ms)).checked_sub(skew)
                }
                _ => None,
            })
            .min()
    }

    /// Evaluate `now` and timed entries (status polls, ctl, plan insert, RetrieveImage).
    pub fn tick_fault_plan(&mut self) {
        let now = self.plan_clock();
        for idx in 0..self.plan.entries.len() {
            if !self.plan.entries[idx].pending() {
                continue;
            }
            match self.plan.entries[idx].trigger {
                Trigger::Now => {
                    let jid = self.plan.entries[idx].job;
                    if let Some(j) = jid {
                        if !self
                            .jobs
                            .get(&j)
                            .is_some_and(|x| !x.aborted && x.pages_left > 0)
                        {
                            continue;
                        }
                    }
                    self.fire_entry(idx, jid, true, "now");
                }
                Trigger::AfterMs { ms, .. } => {
                    let e = self.plan.entries[idx].clone();
                    if let Some(j) = self.timed_due(&e, now) {
                        self.fire_entry(
                            idx,
                            Some(j),
                            true,
                            &format!("timed after {ms} ms (job {j})"),
                        );
                    }
                }
                _ => {}
            }
        }
    }

    /// RetrieveImage pre-check for `job_id` about to deliver page `page_index + 1`.
    pub(super) fn plan_gate_retrieve(&mut self, job_id: i32, token: &str) -> Gate {
        let mut gate = Gate::default();
        let Some(job) = self.jobs.get(&job_id) else {
            return gate;
        };
        if job.token != token || job.aborted {
            return gate;
        }
        let page_index = job.stack_position();
        let page_no = page_index + 1;
        let now = self.plan_clock();
        self.plan.first_retrieve.entry(job_id).or_insert(now);
        self.tick_fault_plan();
        if !self.plan.armed.is_empty() {
            let k = self.plan.armed.remove(0);
            match k {
                FaultKind::Abort => gate.mid = Some((0, Cut::Fraction(0.5))),
                FaultKind::Empty => {
                    self.end_job_empty(job_id);
                    gate.fault = Some(format!("{} (plan, job {job_id})", k.reason()));
                }
                k => gate.fault = Some(format!("{} (plan, job {job_id})", k.reason())),
            }
            if gate.fault.is_some() {
                return gate;
            }
        }
        for idx in 0..self.plan.entries.len() {
            let e = self.plan.entries[idx].clone();
            if !e.pending() || !e.matches_job(job_id) || !e.matches_page(page_no) {
                continue;
            }
            match e.trigger {
                Trigger::AfterPages(n) if page_index >= n => {
                    let how = format!("after {n} pages (job {job_id})");
                    self.fire_entry(idx, Some(job_id), false, &how);
                    if e.kind == FaultKind::Abort {
                        gate.mid = Some((0, Cut::Fraction(0.5)));
                    } else {
                        gate.fault = Some(format!("{} (plan #{} {how})", e.kind.reason(), e.id));
                        return gate;
                    }
                }
                Trigger::MidPage { page, cut } if page == page_no && gate.mid.is_none() => {
                    gate.mid = Some((e.id, cut));
                }
                _ => {}
            }
        }
        gate
    }

    /// A mid-page entry fires: the page is cut and not counted as delivered.
    pub(super) fn plan_fire_mid(&mut self, entry_id: u32, job_id: i32, page_no: u32, cut: Cut) {
        let where_ = format!("mid page {page_no} {} (job {job_id})", cut.describe());
        match self
            .plan
            .entries
            .iter()
            .position(|e| e.id == entry_id && entry_id != 0)
        {
            Some(idx) => self.fire_entry(idx, Some(job_id), false, &where_),
            None => self.note(
                "fault",
                "plan",
                "fired",
                &format!("fault fired: abort {where_}"),
            ),
        }
        self.plan.pending_cut = Some(cut);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::Profile;
    use crate::state::FaultBody;
    use std::path::PathBuf;

    const TICKET: &str = r#"
      <wscn:Format>jfif</wscn:Format>
      <wscn:ImagesToTransfer>0</wscn:ImagesToTransfer>
      <wscn:InputSource>Feeder</wscn:InputSource>
      <wscn:Resolution><wscn:Width>100</wscn:Width><wscn:Height>100</wscn:Height></wscn:Resolution>
    "#;

    fn world(pages: u32) -> World {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../profiles");
        let profile = Profile::load(&dir, "brother-wsd").expect("profile");
        let mut w = World::new(dir, profile);
        w.set_pages(pages);
        w
    }

    fn start(w: &mut World) -> (i32, String) {
        let job = w.create_job(TICKET).expect("create");
        (job.id, job.token.clone())
    }

    fn plan(w: &mut World, v: Value) {
        let b: PlanBody = serde_json::from_value(v).expect("plan json");
        w.add_fault_plan(b).expect("add plan");
    }

    #[test]
    fn cover_open_after_n_serves_two_then_faults_and_sticks() {
        let mut w = world(4);
        w.apply_faults(&FaultBody {
            cover_open_after_n: Some(2),
            ..Default::default()
        });
        assert_eq!(w.cover_open_after_n(), 2);
        assert_eq!(w.status().cover_open_after_n, 2);
        let (id, tok) = start(&mut w);
        w.retrieve_image(id, &tok).expect("page 1");
        w.retrieve_image(id, &tok).expect("page 2");
        assert!(!w.cover_open());
        let err = w.retrieve_image(id, &tok).unwrap_err().to_string();
        assert!(err.contains("ClientErrorCoverOpen"), "{err}");
        assert!(w.cover_open());
        assert_eq!(w.scanner_state_reasons(), ["CoverOpen"]);
        assert_eq!(w.get_job(id).unwrap().page_index, 2);
        assert_eq!(w.get_job(id).unwrap().state_reason, "ScannerStopped");
        let ch = w.drain_pending_condition_changes();
        assert_eq!(ch.len(), 1, "{ch:?}");
        assert_eq!((ch[0].id, ch[0].name, ch[0].raised), (2, "CoverOpen", true));
        // Still refused afterwards (sticky), now via the standing condition.
        assert!(w
            .retrieve_image(id, &tok)
            .unwrap_err()
            .to_string()
            .contains("CoverOpen"));
        assert_eq!(w.cover_open_after_n(), 0, "consumed");
        assert!(w
            .status()
            .fault_plan
            .iter()
            .any(|e| e.fired && e.kind == "cover_open"));
        w.reset();
        assert!(!w.cover_open());
        assert!(w.status().fault_plan.is_empty());
        assert_eq!(w.status().cover_open_after_n, 0);
    }

    #[test]
    fn between_pages_every_sticky_kind() {
        for (kind, needle) in [
            ("jam", "ClientErrorMediaJam"),
            ("cover_open", "ClientErrorCoverOpen"),
            ("interlock_open", "ClientErrorInterlockOpen"),
            ("multi_feed", "ClientErrorMultipleFeed"),
            ("offline", "ServerErrorNotAcceptingJobs"),
            ("lamp_error", "ServerErrorNotAcceptingJobs"),
        ] {
            let mut w = world(3);
            plan(
                &mut w,
                json!({"faults":[{"kind":kind,"when":{"after_pages":1}}]}),
            );
            let (id, tok) = start(&mut w);
            w.retrieve_image(id, &tok).expect("page 1");
            let err = w.retrieve_image(id, &tok).unwrap_err().to_string();
            assert!(err.contains(needle), "{kind}: {err}");
            assert_ne!(w.scanner_state_reasons(), ["None"], "{kind}");
            assert!(!w.drain_pending_condition_changes().is_empty(), "{kind}");
            assert!(w.status().fault_plan[0].fired, "{kind}");
        }
    }

    #[test]
    fn mid_page_by_bytes_and_fraction_cut_and_do_not_count_the_page() {
        for (when, want) in [
            (json!({"page":2,"bytes":1000}), Cut::Bytes(1000)),
            (json!({"page":2,"fraction":0.5}), Cut::Fraction(0.5)),
        ] {
            let mut w = world(4);
            plan(
                &mut w,
                json!({"faults":[{"kind":"cover_open","when":when}]}),
            );
            let (id, tok) = start(&mut w);
            w.retrieve_image(id, &tok).expect("page 1");
            assert!(w.take_pending_cut().is_none());
            assert!(!w.cover_open());
            let (job, img) = w
                .retrieve_image(id, &tok)
                .expect("page 2 served, cut on the wire");
            assert!(!img.is_empty());
            assert_eq!(job.page_index, 1, "cut page is not delivered");
            assert_eq!(w.get_job(id).unwrap().page_index, 1);
            assert_eq!(w.get_job(id).unwrap().pages_left, 3);
            assert_eq!(w.take_pending_cut(), Some(want));
            assert!(w.cover_open());
            assert_eq!(w.get_job(id).unwrap().state_reason, "ScannerStopped");
            assert!(w
                .retrieve_image(id, &tok)
                .unwrap_err()
                .to_string()
                .contains("CoverOpen"));
        }
    }

    #[test]
    fn timed_trigger_with_injected_clock() {
        let mut w = world(3);
        plan(
            &mut w,
            json!({"faults":[{"kind":"cover_open","when":{"after_ms":5000}}]}),
        );
        // No job yet: nothing to time against.
        w.tick_fault_plan();
        assert!(!w.cover_open());
        let (id, tok) = start(&mut w);
        w.tick_fault_plan();
        assert!(!w.cover_open(), "anchored at create");
        w.advance_plan_clock(Duration::from_secs(4));
        w.tick_fault_plan();
        assert!(!w.cover_open());
        w.advance_plan_clock(Duration::from_secs(2));
        w.tick_fault_plan();
        assert!(
            w.cover_open(),
            "fires on the next protocol point after the deadline"
        );
        assert!(w
            .retrieve_image(id, &tok)
            .unwrap_err()
            .to_string()
            .contains("CoverOpen"));
    }

    #[test]
    fn timed_from_first_retrieve_fires_at_retrieve() {
        let mut w = world(3);
        plan(
            &mut w,
            json!({"faults":[{"kind":"jam","when":{"after_ms":100,"from":"first_retrieve"}}]}),
        );
        let (id, tok) = start(&mut w);
        w.advance_plan_clock(Duration::from_secs(60));
        w.tick_fault_plan();
        assert!(!w.jam(), "no first retrieve yet");
        w.retrieve_image(id, &tok).expect("page 1");
        w.advance_plan_clock(Duration::from_millis(200));
        let err = w.retrieve_image(id, &tok).unwrap_err().to_string();
        assert!(err.contains("ClientErrorMediaJam"), "{err}");
    }

    #[test]
    fn now_applies_to_running_job_and_cuts_live_stream() {
        let mut w = world(4);
        let (id, tok) = start(&mut w);
        w.retrieve_image(id, &tok).expect("page 1");
        let handle = w.live_cut_handle(id);
        assert!(!handle.tripped());
        plan(&mut w, json!({"faults":[{"kind":"jam","when":"now"}]}));
        assert!(w.jam(), "applied immediately");
        assert!(handle.tripped(), "in-flight stream is cut");
        assert!(w
            .retrieve_image(id, &tok)
            .unwrap_err()
            .to_string()
            .contains("ClientErrorMediaJam"));
        assert_eq!(w.get_job(id).unwrap().state_reason, "ScannerStopped");
    }

    #[test]
    fn now_transient_faults_once_and_records_history() {
        let mut w = world(4);
        let (id, tok) = start(&mut w);
        w.retrieve_image(id, &tok).expect("page 1");
        plan(
            &mut w,
            json!({"faults":[{"kind":"cover_open","when":"now","sticky":false}]}),
        );
        assert!(!w.cover_open(), "transient condition already cleared");
        let err = w.retrieve_image(id, &tok).unwrap_err().to_string();
        assert!(err.contains("ClientErrorCoverOpen"), "{err}");
        w.retrieve_image(id, &tok).expect("recovers");
        assert!(w.condition_history().iter().any(|h| h.name == "CoverOpen"));
    }

    #[test]
    fn one_shot_vs_repeating() {
        let mut w = world(5);
        plan(
            &mut w,
            json!({"faults":[{"kind":"busy","when":{"after_pages":1}}]}),
        );
        let (id, tok) = start(&mut w);
        w.retrieve_image(id, &tok).expect("p1");
        assert!(w
            .retrieve_image(id, &tok)
            .unwrap_err()
            .to_string()
            .contains("busy"));
        w.retrieve_image(id, &tok).expect("one_shot consumed");

        let mut w = world(5);
        plan(
            &mut w,
            json!({"faults":[{"kind":"busy","when":{"after_pages":1},"one_shot":false}]}),
        );
        let (id, tok) = start(&mut w);
        w.retrieve_image(id, &tok).expect("p1");
        assert!(w.retrieve_image(id, &tok).is_err());
        assert!(w.retrieve_image(id, &tok).is_err());
        assert_eq!(w.status().fault_plan[0].fired_count, 2);
    }

    #[test]
    fn plan_survives_until_fired_filters_and_clear() {
        let mut w = world(4);
        plan(
            &mut w,
            json!({"faults":[
                {"kind":"busy","when":{"after_pages":0},"job":999},
                {"kind":"busy","when":{"after_pages":0},"pages":[3,3]}
            ]}),
        );
        let (id, tok) = start(&mut w);
        w.retrieve_image(id, &tok).expect("p1");
        w.retrieve_image(id, &tok).expect("p2");
        let v = w.status().fault_plan;
        assert_eq!(v.len(), 2);
        assert!(!v[0].fired && !v[1].fired, "still pending");
        assert!(
            w.retrieve_image(id, &tok).is_err(),
            "page 3 matches the range filter"
        );
        let v = w.status().fault_plan;
        assert!(!v[0].fired && v[1].fired);
        w.clear_fault_plan();
        assert!(w.status().fault_plan.is_empty());
    }

    #[test]
    fn abort_kind_cuts_without_condition() {
        let mut w = world(3);
        plan(
            &mut w,
            json!({"faults":[{"kind":"abort","when":{"page":1,"fraction":0.25}}]}),
        );
        let (id, tok) = start(&mut w);
        w.retrieve_image(id, &tok).expect("served");
        assert_eq!(w.take_pending_cut(), Some(Cut::Fraction(0.25)));
        assert_eq!(w.scanner_state_reasons(), ["None"]);
        assert_eq!(w.get_job(id).unwrap().page_index, 0);
        w.retrieve_image(id, &tok).expect("retry works");
    }

    #[test]
    fn empty_kind_and_empty_mid_job_end_the_job_terminally() {
        let mut w = world(4);
        plan(
            &mut w,
            json!({"faults":[{"kind":"empty","when":{"after_pages":1}}]}),
        );
        let (id, tok) = start(&mut w);
        w.retrieve_image(id, &tok).expect("p1");
        let err = w.retrieve_image(id, &tok).unwrap_err().to_string();
        assert!(err.contains("ClientErrorNoImagesAvailable"), "{err}");
        let j = w.get_job(id).unwrap();
        assert_eq!(
            (j.job_state(), j.state_reason.as_str()),
            ("Completed", "JobCompletedWithErrors")
        );

        let mut w = world(5);
        w.apply_faults(&FaultBody {
            empty_mid_job: Some(2),
            ..Default::default()
        });
        let (id, tok) = start(&mut w);
        w.retrieve_image(id, &tok).unwrap();
        w.retrieve_image(id, &tok).unwrap();
        assert!(w.retrieve_image(id, &tok).is_err());
        assert_eq!(
            w.get_job(id).unwrap().state_reason,
            "JobCompletedWithErrors"
        );
    }

    #[test]
    fn legacy_counters_are_listed_read_only() {
        let mut w = world(3);
        w.apply_faults(&FaultBody {
            jam_after_n: Some(2),
            ..Default::default()
        });
        let v = w.status().fault_plan;
        assert_eq!(v.len(), 1);
        assert!(v[0].legacy && v[0].kind == "jam" && !v[0].fired);
    }

    #[test]
    fn bad_plans_are_rejected_atomically() {
        let mut w = world(1);
        let b: PlanBody = serde_json::from_value(json!({"faults":[
            {"kind":"jam","when":"now"},
            {"kind":"jam","when":{"page":1}}
        ]}))
        .unwrap();
        assert!(w.add_fault_plan(b).is_err());
        assert!(w.status().fault_plan.is_empty());
        assert!(!w.jam());
    }

    #[test]
    fn cut_len_clamps() {
        assert_eq!(Cut::Fraction(0.5).len(100), 50);
        assert_eq!(Cut::Bytes(10).len(100), 10);
        assert_eq!(Cut::Bytes(500).len(100), 99);
        assert_eq!(Cut::Fraction(0.0001).len(100), 1);
    }

    #[test]
    fn when_parsing() {
        let b: PlanBody = serde_json::from_value(json!({"faults":[
            {"kind":"cover_open","when":{"after_pages":2}},
            {"kind":"jam","when":{"page":2,"fraction":0.5}},
            {"kind":"no_media","when":"now"},
            {"kind":"offline","when":{"after_ms":50,"from":"first_retrieve"}}
        ]}))
        .unwrap();
        let v = b.into_entries();
        assert_eq!(v.len(), 4);
        assert_eq!(
            parse_when(v[0].when.as_ref()).unwrap(),
            Trigger::AfterPages(2)
        );
        assert!(matches!(
            parse_when(v[1].when.as_ref()).unwrap(),
            Trigger::MidPage { page: 2, .. }
        ));
        assert_eq!(v[2].kind, FaultKind::Empty);
        assert!(matches!(
            parse_when(v[3].when.as_ref()).unwrap(),
            Trigger::AfterMs {
                ms: 50,
                from: Anchor::FirstRetrieve
            }
        ));
        let one: PlanBody = serde_json::from_value(json!({"kind":"jam"})).unwrap();
        assert_eq!(one.into_entries().len(), 1);
        let bad = WhenIn::Obj(WhenObj {
            page: Some(1),
            ..Default::default()
        });
        assert!(parse_when(Some(&bad)).is_err());
    }
}
