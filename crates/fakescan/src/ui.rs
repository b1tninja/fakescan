//! Operator console: `GET /ui` (embedded single-file page), `GET /v1/profiles`, and the checks
//! behind ctl `open_settings` (docs/multi-device.md section 12). Loopback-only.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use axum::extract::{ConnectInfo, Extension, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

use crate::manager::DeviceManager;
use crate::profile::Profile;

/// The page, embedded in the binary. Relative URLs only; no external requests.
pub const INDEX_HTML: &str = include_str!("ui/index.html");

/// True for 127.0.0.0/8, `::1`, and IPv4-mapped IPv6 forms of 127.0.0.0/8.
pub fn is_loopback_peer(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback(),
        IpAddr::V6(v6) => {
            v6.is_loopback() || v6.to_ipv4_mapped().is_some_and(|v4| v4.is_loopback())
        }
    }
}

/// Validate an `open_settings` request: the peer must be known and loopback, and only the
/// `printers` page may be opened.
pub fn check_open_settings(peer: Option<IpAddr>, page: &str) -> Result<(), String> {
    match peer {
        Some(ip) if is_loopback_peer(ip) => {}
        _ => return Err("open_settings: loopback peers only".to_string()),
    }
    if page != "printers" {
        return Err(format!(
            "open_settings: page {page:?} not allowed (only \"printers\")"
        ));
    }
    Ok(())
}

fn forbidden() -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(serde_json::json!({ "error": "the device manager UI is loopback-only" })),
    )
        .into_response()
}

/// `GET /ui`, `GET /ui/`.
pub async fn ui_page(peer: Option<Extension<ConnectInfo<SocketAddr>>>) -> Response {
    match peer {
        Some(Extension(ConnectInfo(a))) if is_loopback_peer(a.ip()) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "text/html; charset=utf-8"),
                (header::CACHE_CONTROL, "no-store"),
            ],
            INDEX_HTML,
        )
            .into_response(),
        _ => forbidden(),
    }
}

#[derive(Debug, Serialize)]
pub struct FeederSummary {
    pub capacity: Option<u32>,
    pub media_sizes: Vec<String>,
    pub duplex: bool,
}

#[derive(Debug, Serialize)]
pub struct ProfileSummary {
    pub id: String,
    pub connection: String,
    pub formats: Vec<String>,
    pub feeder: FeederSummary,
    pub description: String,
}

/// Every loadable profile in `dir`, sorted by id. Unreadable or invalid files are skipped.
pub fn profile_summaries(dir: &std::path::Path) -> Vec<ProfileSummary> {
    let ids = match Profile::list(dir) {
        Ok(ids) => ids,
        Err(e) => {
            tracing::warn!(dir = %dir.display(), error = %e, "listing profiles failed");
            return Vec::new();
        }
    };
    let mut out: Vec<ProfileSummary> = ids
        .into_iter()
        .filter_map(|id| match Profile::load(dir, &id) {
            Ok(p) => Some(ProfileSummary {
                feeder: FeederSummary {
                    capacity: p.feeder.capacity,
                    media_sizes: p.feeder.media_sizes.clone().unwrap_or_default(),
                    duplex: p.feeder.duplex,
                },
                id: p.id,
                connection: p.connection,
                formats: p.formats,
                description: p.label,
            }),
            Err(e) => {
                tracing::warn!(profile = %id, error = %format!("{e:#}"), "skipping unusable profile");
                None
            }
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// `GET /v1/profiles`.
pub async fn list_profiles(State(manager): State<Arc<DeviceManager>>) -> Json<Vec<ProfileSummary>> {
    let dir = manager.config().profiles.clone();
    let list = tokio::task::spawn_blocking(move || profile_summaries(&dir))
        .await
        .unwrap_or_default();
    Json(list)
}
