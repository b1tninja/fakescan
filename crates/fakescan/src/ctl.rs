//! Unified `GET /v1/ctl` (caps + identity) and `POST /v1/ctl` (command dispatch).
//!
//! The same [`dispatch`] function is shared by the HTTP handler (`main.rs`) and
//! the Windows named-pipe server ([`crate::pipe`]), so both surfaces accept
//! identical JSONL.
//!
//! **Registry-level commands** (list, add, remove, ensure, autostart) live in
//! `scripts/fakescan.ps1` and `fakescan-ctl`, not inside the device process.

use std::sync::Arc;

use axum::http::StatusCode;
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::RwLock;

use crate::effects::EffectsBody;
use crate::state::{FaultBody, LoopbackBody, PagesBody, World};

/// Every cmd accepted by `POST /v1/ctl`.
pub const CTL_CMDS: &[&str] = &[
    "status",
    "health",
    "jobs",
    "activity",
    "profile",
    "pattern",
    "pages",
    "feeder",
    "jpeg_com",
    "effects",
    "scenario",
    "fault",
    "slow",
    "loopback",
    "push_scan",
    "reset",
    "save",
    "shutdown",
];

/// Body for `POST /v1/ctl`.
#[derive(Debug, Deserialize)]
pub struct CtlRequest {
    pub cmd: String,
    #[serde(default)]
    pub args: Value,
}

/// Response body for `GET /v1/ctl` — capabilities + running identity.
#[derive(Debug, Serialize)]
pub struct CtlInfo {
    pub cmds: &'static [&'static str],
    pub port: u16,
    pub uuid: String,
    pub base_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// `GET /v1/ctl` — return available commands and this instance's identity.
pub async fn get_ctl(State(world): State<Arc<RwLock<World>>>) -> Json<CtlInfo> {
    let w = world.read().await;
    Json(CtlInfo {
        cmds: CTL_CMDS,
        port: w.lab_port(),
        uuid: w.device_uuid().to_string(),
        base_url: w.lab_base_url(),
        name: w.lab_name().map(str::to_owned),
    })
}

/// `POST /v1/ctl` — dispatch a named command (JSON body: `{cmd, args}`).
pub async fn post_ctl(
    State(world): State<Arc<RwLock<World>>>,
    Json(req): Json<CtlRequest>,
) -> Result<Json<Value>, (StatusCode, String)> {
    dispatch(&world, req)
        .await
        .map(Json)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))
}

/// Core dispatch — shared by the HTTP handler and the Windows named-pipe server.
///
/// Returns the JSON response value on success, or an error message.
pub async fn dispatch(world: &Arc<RwLock<World>>, req: CtlRequest) -> anyhow::Result<Value> {
    match req.cmd.as_str() {
        "status" | "health" => {
            let mut w = world.write().await;
            w.sweep_timed_out_jobs();
            Ok(serde_json::to_value(w.status())?)
        }

        "jobs" => {
            let w = world.read().await;
            Ok(serde_json::to_value(w.jobs_view())?)
        }

        "activity" => {
            let after = args_after(&req.args);
            let w = world.read().await;
            Ok(serde_json::to_value(w.trace_since(after))?)
        }

        "profile" => {
            let id = req.args["id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("profile: missing args.id (string)"))?
                .to_owned();
            let mut w = world.write().await;
            w.load_profile(&id)?;
            crate::wsd::job_events::maybe_emit_elements_change(
                &w,
                crate::wsd::soap::SCAN_NS_AIRSCAN,
                w.profile(),
            );
            Ok(serde_json::to_value(w.status())?)
        }

        "pattern" => {
            let id = req.args["id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("pattern: missing args.id (string)"))?
                .to_owned();
            let mut w = world.write().await;
            w.set_image_pattern_id(&id)?;
            crate::wsd::job_events::maybe_emit_elements_change(
                &w,
                crate::wsd::soap::SCAN_NS_AIRSCAN,
                w.profile(),
            );
            Ok(serde_json::to_value(w.status())?)
        }

        "pages" => {
            let body: PagesBody =
                serde_json::from_value(req.args).map_err(|e| anyhow::anyhow!("pages: {e}"))?;
            let mut w = world.write().await;
            w.apply_pages(&body)?;
            crate::wsd::job_events::maybe_emit_elements_change(
                &w,
                crate::wsd::soap::SCAN_NS_AIRSCAN,
                w.profile(),
            );
            Ok(serde_json::to_value(w.status())?)
        }

        "feeder" => {
            let ready = req.args["ready"]
                .as_bool()
                .ok_or_else(|| anyhow::anyhow!("feeder: missing args.ready (bool)"))?;
            let mut w = world.write().await;
            w.set_feeder(ready);
            crate::wsd::job_events::maybe_emit_elements_change(
                &w,
                crate::wsd::soap::SCAN_NS_AIRSCAN,
                w.profile(),
            );
            Ok(serde_json::to_value(w.status())?)
        }

        "jpeg_com" => {
            let enabled = req.args["enabled"]
                .as_bool()
                .ok_or_else(|| anyhow::anyhow!("jpeg_com: missing args.enabled (bool)"))?;
            let mut w = world.write().await;
            w.set_jpeg_com(enabled);
            Ok(serde_json::to_value(w.status())?)
        }

        "effects" => {
            let body: EffectsBody =
                serde_json::from_value(req.args).map_err(|e| anyhow::anyhow!("effects: {e}"))?;
            let mut w = world.write().await;
            w.apply_effects(&body)?;
            crate::wsd::job_events::maybe_emit_elements_change(
                &w,
                crate::wsd::soap::SCAN_NS_AIRSCAN,
                w.profile(),
            );
            Ok(serde_json::to_value(w.status())?)
        }

        "scenario" => {
            let id = req.args["id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("scenario: missing args.id (string)"))?
                .to_owned();
            let mut w = world.write().await;
            w.apply_scenario(&id)?;
            crate::wsd::job_events::maybe_emit_elements_change(
                &w,
                crate::wsd::soap::SCAN_NS_AIRSCAN,
                w.profile(),
            );
            Ok(serde_json::to_value(w.status())?)
        }

        "fault" => {
            let body: FaultBody = serde_json::from_value(req.args).unwrap_or_default();
            let mut w = world.write().await;
            let changes = w.apply_faults(&body);
            // Best-effort NotifyTo for raised/cleared ScannerStatusCondition*.
            crate::wsd::job_events::maybe_emit_condition_changes(
                &w,
                crate::wsd::soap::SCAN_NS_AIRSCAN,
                &changes,
            );
            if !changes.is_empty() {
                crate::wsd::job_events::maybe_emit_status_summary(
                    &w,
                    crate::wsd::soap::SCAN_NS_AIRSCAN,
                );
            }
            Ok(serde_json::to_value(w.status())?)
        }

        "slow" => {
            let body: crate::state::SlowBody = serde_json::from_value(req.args.clone())
                .map_err(|e| anyhow::anyhow!("slow: {e}"))?;
            if req.args.get("ms").is_none() && req.args.get("bps").is_none() {
                anyhow::bail!("slow: need args.ms and/or args.bps");
            }
            let mut w = world.write().await;
            w.apply_slow(&body);
            Ok(serde_json::to_value(w.status())?)
        }

        "loopback" => {
            let body: LoopbackBody =
                serde_json::from_value(req.args).map_err(|e| anyhow::anyhow!("loopback: {e}"))?;
            let mut w = world.write().await;
            w.set_loopback(&body)?;
            Ok(serde_json::to_value(w.status())?)
        }

        "push_scan" => {
            // Lab stand-in for ScanAvailableEvent: mint ScanIdentifier for a registered
            // DestinationToken (from Subscribe). Optional args.destination_token.
            // When Subscribe stored a NotifyTo URL, POST the event best-effort (ctl
            // still returns the ids if the sink is down).
            let token = req
                .args
                .get("destination_token")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(str::to_owned);
            let pending = {
                let mut w = world.write().await;
                w.mint_push_scan(token.as_deref())?
            };
            if let Some(ref url) = pending.notify_to {
                let _ = crate::wsd::notify::deliver_scan_available(
                    url,
                    crate::wsd::notify::SCAN_AVAILABLE_NS,
                    &pending.scan_identifier,
                    &pending.client_context,
                )
                .await;
            }
            Ok(serde_json::to_value(pending)?)
        }

        "reset" => {
            let mut w = world.write().await;
            w.reset();
            Ok(serde_json::to_value(w.status())?)
        }

        "save" => {
            // Write the running state to the devices.json row for this lab_name.
            let (lab_name, lab_listen, uuid_str, profile_id) = {
                let w = world.read().await;
                (
                    w.lab_name().map(str::to_owned),
                    w.lab_listen(),
                    w.device_uuid().to_string(),
                    w.profile().id.clone(),
                )
            };
            let name = lab_name.ok_or_else(|| {
                anyhow::anyhow!(
                    "save: no --lab-name set; cannot identify registry row.\n\
                     Start with --lab-name default (or FAKESCAN_NAME=default)."
                )
            })?;
            let listen_str = lab_listen
                .map(|a| a.to_string())
                .unwrap_or_else(|| format!("0.0.0.0:{}", crate::lab::LAB_PORT));

            use crate::registry::{AddedInfo, DeviceRow, Registry};
            let mut reg = Registry::load().unwrap_or_default();
            if let Some(row) = reg.find_mut(&name) {
                row.profile = profile_id.clone();
                row.listen = listen_str.clone();
                row.device_uuid = uuid_str.clone();
            } else {
                reg.devices.push(DeviceRow {
                    name: name.clone(),
                    listen: listen_str.clone(),
                    advertise: String::new(),
                    device_uuid: uuid_str.clone(),
                    profile: profile_id.clone(),
                    exe: None,
                    autostart: false,
                    harness: None,
                    added: AddedInfo::default(),
                });
            }
            reg.save_to_disk()?;
            Ok(json!({
                "saved": true,
                "name": name,
                "listen": listen_str,
                "device_uuid": uuid_str,
                "profile": profile_id,
                "path": crate::registry::registry_path().display().to_string(),
            }))
        }

        "shutdown" => {
            // A stray POST must not take the scanner offline. `yes` is required.
            let yes = req
                .args
                .get("yes")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if !yes {
                anyhow::bail!("shutdown needs yes: true");
            }
            world.read().await.request_exit()?;
            Ok(json!({ "shutting_down": true }))
        }

        other => anyhow::bail!(
            "unknown ctl cmd {other:?}; see GET /v1/ctl for the list of supported commands"
        ),
    }
}

/// `args.after` from a probe: a JSON number, a decimal string, or omitted.
///
/// `as_u64` alone drops `"12"` and `12.0` (PowerShell's JSON numbers are
/// doubles). That cursor has to be honored, not turned into 0 or a 400.
fn args_after(args: &Value) -> u64 {
    let Some(value) = args.get("after") else {
        return 0;
    };
    if let Some(n) = value.as_u64() {
        return n;
    }
    if let Some(n) = value.as_i64() {
        return if n < 0 { 0 } else { n as u64 };
    }
    if let Some(n) = value.as_f64() {
        if n.is_finite() && n >= 0.0 && n.fract() == 0.0 && n <= u64::MAX as f64 {
            return n as u64;
        }
        return 0;
    }
    value.as_str().map(crate::state::parse_cursor).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    fn test_world() -> Arc<RwLock<World>> {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../profiles");
        let profile = crate::profile::Profile::load(&dir, "brother-wsd").expect("profile");
        Arc::new(RwLock::new(World::new(dir, profile)))
    }

    #[tokio::test]
    async fn ctl_shutdown_waits_for_yes_then_signals() {
        let w = test_world();
        let (tx, mut rx) = tokio::sync::watch::channel(false);
        w.write().await.arm_exit(tx);
        let missing = dispatch(
            &w,
            CtlRequest {
                cmd: "shutdown".into(),
                args: json!({}),
            },
        )
        .await
        .expect_err("yes is required");
        assert!(missing.to_string().contains("yes"), "{missing}");
        assert!(!*rx.borrow());
        let v = dispatch(
            &w,
            CtlRequest {
                cmd: "shutdown".into(),
                args: json!({ "yes": true }),
            },
        )
        .await
        .expect("shutdown");
        assert_eq!(v["shutting_down"], true);
        rx.changed().await.expect("exit signal");
        assert!(*rx.borrow());
    }

    #[tokio::test]
    async fn ctl_status_returns_profile_id() {
        let w = test_world();
        let req = CtlRequest {
            cmd: "status".into(),
            args: Value::Null,
        };
        let v = dispatch(&w, req).await.unwrap();
        assert!(
            v["profile_id"].as_str().is_some(),
            "profile_id missing: {v}"
        );
    }

    #[tokio::test]
    async fn ctl_health_returns_status() {
        let w = test_world();
        let req = CtlRequest {
            cmd: "health".into(),
            args: Value::Null,
        };
        let v = dispatch(&w, req).await.unwrap();
        assert!(v.is_object());
    }

    #[tokio::test]
    async fn ctl_reset_clears_pages() {
        let w = test_world();
        {
            let mut g = w.write().await;
            g.set_pages(5);
        }
        let req = CtlRequest {
            cmd: "reset".into(),
            args: Value::Null,
        };
        let v = dispatch(&w, req).await.unwrap();
        assert_eq!(v["pages_remaining"], 0, "reset should clear pages");
    }

    #[tokio::test]
    async fn ctl_pages_sets_count() {
        let w = test_world();
        let req = CtlRequest {
            cmd: "pages".into(),
            args: json!({ "count": 3 }),
        };
        let v = dispatch(&w, req).await.unwrap();
        assert_eq!(v["pages_remaining"], 3);
    }

    #[tokio::test]
    async fn ctl_feeder_toggles() {
        let w = test_world();
        // brother-wsd has sticky_ready=true, feeder starts ready.
        let req = CtlRequest {
            cmd: "feeder".into(),
            args: json!({ "ready": false }),
        };
        let v = dispatch(&w, req).await.unwrap();
        assert_eq!(v["feeder_ready"], false);
    }

    #[tokio::test]
    async fn ctl_pattern_changes_image_pattern() {
        let w = test_world();
        let req = CtlRequest {
            cmd: "pattern".into(),
            args: json!({ "id": "wedges" }),
        };
        let v = dispatch(&w, req).await.unwrap();
        assert_eq!(v["image_pattern"], "wedges");
    }

    #[tokio::test]
    async fn ctl_fault_sets_jam() {
        let w = test_world();
        let req = CtlRequest {
            cmd: "fault".into(),
            args: json!({ "jam": true }),
        };
        let v = dispatch(&w, req).await.unwrap();
        assert_eq!(v["jam"], true);
    }

    #[tokio::test]
    async fn ctl_slow_sets_ms() {
        let w = test_world();
        let req = CtlRequest {
            cmd: "slow".into(),
            args: json!({ "ms": 100 }),
        };
        let v = dispatch(&w, req).await.unwrap();
        assert_eq!(v["slow_retrieve_ms"], 100);
    }

    #[tokio::test]
    async fn ctl_slow_sets_bps() {
        let w = test_world();
        let req = CtlRequest {
            cmd: "slow".into(),
            args: json!({ "bps": 50000 }),
        };
        let v = dispatch(&w, req).await.unwrap();
        assert_eq!(v["retrieve_bps"], 50000);
        assert_eq!(v["slow_retrieve_ms"], 0);
    }

    #[tokio::test]
    async fn ctl_jpeg_com_toggle() {
        let w = test_world();
        let req = CtlRequest {
            cmd: "jpeg_com".into(),
            args: json!({ "enabled": false }),
        };
        let v = dispatch(&w, req).await.unwrap();
        assert_eq!(v["jpeg_com"], false);
    }

    #[tokio::test]
    async fn ctl_scenario_applies() {
        let w = test_world();
        let req = CtlRequest {
            cmd: "scenario".into(),
            args: json!({ "id": "order" }),
        };
        let v = dispatch(&w, req).await.unwrap();
        assert_eq!(v["scenario"], "order");
        assert_eq!(v["pages_remaining"], 3);
    }

    #[tokio::test]
    async fn ctl_unknown_cmd_errors() {
        let w = test_world();
        let req = CtlRequest {
            cmd: "doesnotexist".into(),
            args: Value::Null,
        };
        let err = dispatch(&w, req).await.unwrap_err();
        assert!(err.to_string().contains("unknown ctl cmd"));
    }

    #[tokio::test]
    async fn ctl_jobs_and_activity_shape() {
        let w = test_world();
        let jobs = dispatch(
            &w,
            CtlRequest {
                cmd: "jobs".into(),
                args: Value::Null,
            },
        )
        .await
        .unwrap();
        assert_eq!(jobs["jobs"].as_array().map(|a| a.len()), Some(0), "{jobs}");

        let activity = dispatch(
            &w,
            CtlRequest {
                cmd: "activity".into(),
                args: Value::Null,
            },
        )
        .await
        .unwrap();
        assert!(activity["seq"].as_u64().is_some(), "{activity}");
        assert!(
            activity["events"].as_array().unwrap().is_empty(),
            "{activity}"
        );

        {
            let mut g = w.write().await;
            g.note("wsd", "Get", "ok", "soap {peer}");
        }
        let seq = w.read().await.trace_seq();
        let text = w.read().await.log_since(0);
        assert!(text.contains("soap {peer}"), "{text}");

        let string_after = dispatch(
            &w,
            CtlRequest {
                cmd: "activity".into(),
                args: json!({ "after": seq.to_string() }),
            },
        )
        .await
        .expect("string after must not reject the probe");
        assert!(string_after["seq"].as_u64().is_some(), "{string_after}");
        assert!(
            string_after["events"].as_array().unwrap().is_empty(),
            "{string_after}"
        );

        let float_after = Value::Object({
            let mut map = serde_json::Map::new();
            map.insert(
                "after".into(),
                Value::Number(serde_json::Number::from_f64(seq as f64).unwrap()),
            );
            map
        });
        let float_view = dispatch(
            &w,
            CtlRequest {
                cmd: "activity".into(),
                args: float_after,
            },
        )
        .await
        .expect("decimal after must not reject the probe");
        assert!(float_view["seq"].as_u64().is_some(), "{float_view}");
        assert!(float_view["events"].is_array(), "{float_view}");
        assert!(
            float_view["events"].as_array().unwrap().is_empty(),
            "{float_view}"
        );

        let token = {
            let mut g = w.write().await;
            g.create_job(
                r#"
          <wscn:Format>jfif</wscn:Format>
          <wscn:ImagesToTransfer>1</wscn:ImagesToTransfer>
          <wscn:InputSource>Platen</wscn:InputSource>
          <wscn:Resolution><wscn:Width>200</wscn:Width><wscn:Height>200</wscn:Height></wscn:Resolution>
        "#,
            )
            .unwrap()
            .token
        };
        let jobs = dispatch(
            &w,
            CtlRequest {
                cmd: "jobs".into(),
                args: Value::Null,
            },
        )
        .await
        .unwrap();
        assert!(jobs["jobs"][0].get("token").is_none(), "{jobs}");
        assert!(!jobs.to_string().contains(&token), "{jobs}");

        dispatch(
            &w,
            CtlRequest {
                cmd: "reset".into(),
                args: Value::Null,
            },
        )
        .await
        .unwrap();
        let activity = dispatch(
            &w,
            CtlRequest {
                cmd: "activity".into(),
                args: json!({ "after": 0 }),
            },
        )
        .await
        .unwrap();
        let actions: Vec<_> = activity["events"]
            .as_array()
            .unwrap()
            .iter()
            .map(|event| event["action"].as_str().unwrap())
            .collect();
        assert!(
            actions.contains(&"Get"),
            "reset wiped the trace: {actions:?}"
        );
        assert!(actions.contains(&"reset"), "{actions:?}");
    }

    #[tokio::test]
    async fn ctl_save_errors_without_lab_name() {
        let w = test_world();
        // World has no lab_name by default → save should fail gracefully.
        let req = CtlRequest {
            cmd: "save".into(),
            args: Value::Null,
        };
        let err = dispatch(&w, req).await.unwrap_err();
        assert!(
            err.to_string().contains("lab-name"),
            "expected lab-name error: {err}"
        );
    }
}
