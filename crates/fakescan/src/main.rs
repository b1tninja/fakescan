//! fakescan binary: control plane + optional WSD discovery.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use clap::Parser;
use fakescan::discovery::DiscoveryConfig;
use fakescan::profile::Profile;
use fakescan::state::{FeederBody, PagesBody, Status, World};
use fakescan::{discovery, wsd};
use serde::Deserialize;
use tokio::sync::RwLock;
use tracing::info;
use uuid::Uuid;

#[derive(Debug, Parser)]
#[command(name = "fakescan", about = "Fake scanner for WIA / WSD testing")]
struct Args {
    /// Listen address for the control API and Scan Service stub.
    #[arg(long, default_value = "127.0.0.1:3920")]
    listen: SocketAddr,

    /// Advertise this host in WS-Discovery XAddrs (defaults to listen IP).
    /// Use a LAN address (not 127.0.0.1) if another PC should discover you.
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

    /// Stable device UUID for discovery (random if omitted).
    #[arg(long)]
    device_uuid: Option<Uuid>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "fakescan=info,tower_http=info".into()),
        )
        .init();

    let args = Args::parse();
    let wsd_on = !args.no_wsd;
    let profile = Profile::load(&args.profiles, &args.profile)?;
    info!(id = %profile.id, "loaded profile");

    let world = Arc::new(RwLock::new(World::new(args.profiles.clone(), profile)));

    let mut app = Router::new()
        .route("/health", get(health))
        .route("/v1/status", get(status))
        .route("/v1/profile", post(set_profile))
        .route("/v1/feeder", post(set_feeder))
        .route("/v1/pages", post(set_pages))
        .route("/v1/reset", post(reset));

    if wsd_on {
        app = app
            .route("/ScannerService", post(wsd::scanner_service))
            .route("/ScannerService/", post(wsd::scanner_service));
    }

    let app = app.with_state(world);

    if wsd_on {
        let adv_ip = args.advertise.unwrap_or_else(|| args.listen.ip());
        let adv_ip = if adv_ip.is_unspecified() {
            IpAddr::V4(Ipv4Addr::LOCALHOST)
        } else {
            adv_ip
        };
        let endpoint = format!("http://{}:{}/ScannerService", adv_ip, args.listen.port());
        let cfg = DiscoveryConfig {
            endpoint: endpoint.clone(),
            device_uuid: args.device_uuid.unwrap_or_else(Uuid::new_v4),
            friendly_name: "fakescan".into(),
        };
        info!(%endpoint, "WSD Scan Service stub + discovery enabled");
        tokio::spawn(async move {
            if let Err(e) = discovery::run(cfg).await {
                tracing::error!(error = %e, "WS-Discovery stopped");
            }
        });
    }

    info!(%args.listen, "listening");
    let listener = tokio::net::TcpListener::bind(args.listen).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn health() -> &'static str {
    "ok"
}

async fn status(State(world): State<Arc<RwLock<World>>>) -> Json<Status> {
    Json(world.read().await.status())
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
    Ok(Json(w.status()))
}

async fn set_feeder(
    State(world): State<Arc<RwLock<World>>>,
    Json(body): Json<FeederBody>,
) -> Json<Status> {
    let mut w = world.write().await;
    w.set_feeder(body.ready);
    Json(w.status())
}

async fn set_pages(
    State(world): State<Arc<RwLock<World>>>,
    Json(body): Json<PagesBody>,
) -> Json<Status> {
    let mut w = world.write().await;
    w.set_pages(body.count);
    Json(w.status())
}

async fn reset(State(world): State<Arc<RwLock<World>>>) -> Json<Status> {
    let mut w = world.write().await;
    w.reset();
    Json(w.status())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    info!("shutting down");
}
