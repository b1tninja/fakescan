//! Per-device ring of every ScanService request that carries a scan ticket
//! (`ValidateScanTicket`, `CreateScanJob`), for `GET /v1/tickets`.
//!
//! Lets a tester see the sequence of tickets a driver offers (Windows sends ~5 Validates per
//! Create). Recorded after the handler decides, so `outcome` reflects what was answered.

use std::collections::VecDeque;
use std::sync::Mutex;

use serde::Serialize;
use serde_json::{Map, Value};

pub const DEFAULT_CAP: usize = 200;
/// Request XML kept per entry (bytes; cut on a char boundary).
pub const BODY_CAP: usize = 32 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct TicketEntry {
    pub seq: u64,
    pub ts: String,
    pub device_id: String,
    /// `ValidateScanTicket` or `CreateScanJob`.
    pub operation: String,
    /// `valid` | `invalid` | `fault:<subcode>` | `accepted` | `refused:<subcode>`.
    pub outcome: String,
    /// Ticket-validation reason when the ticket was not clean (extra context).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Ticket fields the parser found (only those present).
    pub parsed: Map<String, Value>,
    /// What the response carried (`color_processing`, `bits`, `bytes_per_line`, `format`), when answered.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_params: Option<Value>,
    pub body: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TicketsView {
    pub count: usize,
    /// Pass as `?since=` to get only newer entries (highest seq ever assigned).
    pub next_seq: u64,
    pub tickets: Vec<TicketEntry>,
}

#[derive(Debug)]
struct Inner {
    seq: u64,
    cap: usize,
    ring: VecDeque<TicketEntry>,
}

#[derive(Debug)]
pub struct TicketLog(Mutex<Inner>);

impl Default for TicketLog {
    fn default() -> Self {
        Self::with_cap(DEFAULT_CAP)
    }
}

pub fn cap_body(body: &str) -> String {
    if body.len() <= BODY_CAP {
        return body.to_string();
    }
    let mut end = BODY_CAP;
    while !body.is_char_boundary(end) {
        end -= 1;
    }
    body[..end].to_string()
}

/// `bits` from a WSD colour token (`BlackAndWhite1` -> 1, `Grayscale8` -> 8, `RGB24` -> 24).
pub fn bits_for_token(token: &str) -> Option<u32> {
    let digits: String = token
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    digits.parse().ok()
}

impl TicketLog {
    pub fn with_cap(cap: usize) -> Self {
        Self(Mutex::new(Inner {
            seq: 0,
            cap: cap.max(1),
            ring: VecDeque::new(),
        }))
    }

    pub fn set_cap(&self, cap: usize) {
        let mut g = self.0.lock().unwrap();
        g.cap = cap.max(1);
        while g.ring.len() > g.cap {
            g.ring.pop_front();
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &self,
        device_id: &str,
        operation: &str,
        outcome: String,
        reason: Option<String>,
        parsed: Map<String, Value>,
        final_params: Option<Value>,
        body: &str,
    ) {
        let mut g = self.0.lock().unwrap();
        g.seq += 1;
        let seq = g.seq;
        g.ring.push_back(TicketEntry {
            seq,
            ts: crate::timeutil::utc_now_iso(),
            device_id: device_id.to_string(),
            operation: operation.to_string(),
            outcome,
            reason,
            parsed,
            final_params,
            body: cap_body(body),
        });
        while g.ring.len() > g.cap {
            g.ring.pop_front();
        }
    }

    /// Entries with `seq > since`, optionally one operation, oldest first, at most `limit`.
    pub fn view(&self, since: u64, op: Option<&str>, limit: Option<usize>) -> TicketsView {
        let g = self.0.lock().unwrap();
        let tickets: Vec<TicketEntry> = g
            .ring
            .iter()
            .filter(|e| e.seq > since)
            .filter(|e| op.is_none_or(|o| e.operation.eq_ignore_ascii_case(o)))
            .take(limit.unwrap_or(usize::MAX))
            .cloned()
            .collect();
        TicketsView {
            count: tickets.len(),
            next_seq: g.seq,
            tickets,
        }
    }

    /// Drop the entries; `seq` keeps counting so old cursors stay valid.
    pub fn clear(&self) {
        self.0.lock().unwrap().ring.clear();
    }
}

/// Query for `GET /v1/tickets`. Strings so a stray `?since=` or `?limit=x` does not 400.
#[derive(Debug, Default, serde::Deserialize)]
pub struct TicketsQuery {
    pub since: Option<String>,
    pub op: Option<String>,
    pub limit: Option<String>,
}

impl TicketsQuery {
    pub fn run(&self, log: &TicketLog) -> TicketsView {
        let since = self
            .since
            .as_deref()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(0);
        let op = self.op.as_deref().map(str::trim).filter(|s| !s.is_empty());
        let limit = self.limit.as_deref().and_then(|s| s.trim().parse().ok());
        log.view(since, op, limit)
    }
}
