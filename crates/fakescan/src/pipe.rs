//! Windows named pipe server: `\\.\pipe\fakescan-<name>`
//!
//! Accepts JSONL ctl commands and proxies them to [`crate::ctl::dispatch`] —
//! the same function as `POST /v1/ctl` — so scripts can control a running
//! instance without knowing its HTTP port.
//!
//! **Non-Windows:** this module compiles but [`serve`] is a no-op.  Use
//! `POST /v1/ctl` directly on Linux/macOS.
//!
//! ## Wire protocol
//!
//! One JSON object per line (JSONL), same schema as `POST /v1/ctl`:
//!
//! ```text
//! → {"cmd":"pages","args":{"count":3}}\n
//! ← {"ok":true,"result":{...status...}}\n
//!
//! → {"cmd":"bad"}\n
//! ← {"ok":false,"error":"unknown ctl cmd \"bad\""}\n
//! ```

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::manager::DeviceCtx;
use crate::state::World;

/// Start `\\.\pipe\fakescan-<name>`.
///
/// On Windows, this loops forever accepting JSONL clients and dispatching each
/// line via [`crate::ctl::dispatch`].  The function is `async` so callers can
/// `tokio::spawn` it without blocking.
///
/// On non-Windows platforms this returns immediately after logging a debug note.
pub async fn serve(name: &str, world: Arc<RwLock<World>>, ctx: Option<DeviceCtx>) {
    #[cfg(target_os = "windows")]
    serve_windows(name, world, ctx).await;

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (name, world, ctx);
        tracing::debug!("named pipe server is Windows-only; skipped on this platform");
    }
}

#[cfg(target_os = "windows")]
async fn serve_windows(name: &str, world: Arc<RwLock<World>>, ctx: Option<DeviceCtx>) {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::windows::named_pipe::ServerOptions;

    let pipe_name = format!(r"\\.\pipe\fakescan-{name}");
    tracing::info!(pipe = %pipe_name, "named pipe server starting");

    // The first instance must set first_pipe_instance(true); subsequent ones false.
    let mut first_instance = true;
    loop {
        let server = match ServerOptions::new()
            .first_pipe_instance(first_instance)
            .create(&pipe_name)
        {
            Ok(s) => {
                first_instance = false;
                s
            }
            Err(e) => {
                tracing::error!(
                    error = %e,
                    pipe = %pipe_name,
                    "named pipe create failed; retrying in 1 s"
                );
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                continue;
            }
        };

        // Wait for the next client to connect.
        match server.connect().await {
            Ok(()) => {}
            Err(e) => {
                tracing::warn!(error = %e, "named pipe client-connect error; continuing");
                continue;
            }
        }

        let world2 = Arc::clone(&world);
        let ctx2 = ctx.clone();
        tokio::spawn(async move {
            let (reader, mut writer) = tokio::io::split(server);
            let mut lines = BufReader::new(reader).lines();
            while let Ok(Some(raw)) = lines.next_line().await {
                let raw = raw.trim().to_owned();
                if raw.is_empty() {
                    continue;
                }
                let resp = match serde_json::from_str::<crate::ctl::CtlRequest>(&raw) {
                    Ok(req) => match crate::ctl::dispatch_ctx(ctx2.as_ref(), &world2, req).await {
                        Ok(v) => serde_json::json!({ "ok": true, "result": v }),
                        Err(e) => {
                            serde_json::json!({ "ok": false, "error": e.to_string() })
                        }
                    },
                    Err(e) => {
                        serde_json::json!({ "ok": false, "error": format!("parse: {e}") })
                    }
                };
                let mut out = serde_json::to_string(&resp).unwrap_or_default();
                out.push('\n');
                if writer.write_all(out.as_bytes()).await.is_err() {
                    break;
                }
            }
        });
    }
}
