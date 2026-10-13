//! `GET /v1/tickets`: ring of every ValidateScanTicket / CreateScanJob request.

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use fakescan::effects::EffectsBody;
use fakescan::profile::Profile;
use fakescan::state::World;
use serde_json::{json, Value};
use tokio::sync::RwLock;
use tower::ServiceExt;

fn world() -> Arc<RwLock<World>> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../profiles");
    let profile = Profile::load(&dir, "brother-wsd").expect("profile");
    Arc::new(RwLock::new(World::new(dir, profile)))
}

fn app(w: &Arc<RwLock<World>>) -> axum::Router {
    fakescan::server::device_router(w.clone(), true)
}

async fn send(
    w: &Arc<RwLock<World>>,
    method: &str,
    uri: &str,
    body: String,
) -> (StatusCode, String) {
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/soap+xml")
        .body(Body::from(body))
        .unwrap();
    let res = app(w).oneshot(req).await.unwrap();
    let st = res.status();
    let b = axum::body::to_bytes(res.into_body(), 16 * 1024 * 1024)
        .await
        .unwrap();
    (st, String::from_utf8_lossy(&b).into_owned())
}

fn ticket(op: &str, format: &str, color: &str, dpi: u32) -> String {
    format!(
        r#"<?xml version="1.0"?>
<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope"
  xmlns:wsa="http://schemas.xmlsoap.org/ws/2004/08/addressing"
  xmlns:wscn="http://schemas.microsoft.com/windows/2006/01/wdp/scan">
  <soap:Header>
    <wsa:Action>http://schemas.microsoft.com/windows/2006/01/wdp/scan/{op}</wsa:Action>
    <wsa:MessageID>uuid:c1c1c1c1-c1c1-c1c1-c1c1-c1c1c1c1c1c1</wsa:MessageID>
  </soap:Header>
  <soap:Body><wscn:{op}Request><wscn:ScanTicket><wscn:DocumentParameters>
    <wscn:Format>{format}</wscn:Format>
    <wscn:ImagesToTransfer>1</wscn:ImagesToTransfer>
    <wscn:InputSource>Platen</wscn:InputSource>
    <wscn:MediaSides><wscn:MediaFront>
      <wscn:ColorProcessing>{color}</wscn:ColorProcessing>
      <wscn:Resolution><wscn:Width>{dpi}</wscn:Width><wscn:Height>{dpi}</wscn:Height></wscn:Resolution>
    </wscn:MediaFront></wscn:MediaSides>
  </wscn:DocumentParameters></wscn:ScanTicket></wscn:{op}Request></soap:Body>
</soap:Envelope>"#
    )
}

async fn post_ticket(
    w: &Arc<RwLock<World>>,
    op: &str,
    format: &str,
    color: &str,
    dpi: u32,
) -> StatusCode {
    send(w, "POST", "/ScannerService", ticket(op, format, color, dpi))
        .await
        .0
}

async fn tickets(w: &Arc<RwLock<World>>, query: &str) -> Value {
    let (st, b) = send(w, "GET", &format!("/v1/tickets{query}"), String::new()).await;
    assert_eq!(st, StatusCode::OK, "{b}");
    serde_json::from_str(&b).unwrap()
}

async fn validate_then_create(w: &Arc<RwLock<World>>) {
    assert_eq!(
        post_ticket(w, "ValidateScanTicket", "jfif", "RGB24", 200).await,
        StatusCode::OK
    );
    assert_eq!(
        post_ticket(w, "CreateScanJob", "jfif", "RGB24", 200).await,
        StatusCode::OK
    );
}

#[tokio::test]
async fn validate_then_create_recorded_in_order_with_parsed_fields() {
    let w = world();
    validate_then_create(&w).await;
    let v = tickets(&w, "").await;
    assert_eq!(v["count"], 2, "{v}");
    assert_eq!(v["next_seq"], 2);
    let t = &v["tickets"];
    assert_eq!(t[0]["seq"], 1);
    assert_eq!(t[0]["operation"], "ValidateScanTicket");
    assert_eq!(t[0]["outcome"], "valid");
    assert_eq!(t[1]["seq"], 2);
    assert_eq!(t[1]["operation"], "CreateScanJob");
    assert_eq!(t[1]["outcome"], "accepted");
    assert!(t[0]["device_id"].is_string());
    assert!(t[0]["ts"].as_str().unwrap().contains('T'));
    let p = &t[0]["parsed"];
    assert_eq!(p["input_source"], "Platen");
    assert_eq!(p["format"], "jfif");
    assert_eq!(p["color_processing"], "RGB24");
    assert_eq!(p["resolution_x"], 200);
    assert_eq!(p["resolution_y"], 200);
    assert_eq!(p["images_to_transfer"], 1);
    assert!(
        p.get("compression_quality").is_none(),
        "absent fields omitted: {p}"
    );
    let f = &t[1]["final_params"];
    assert_eq!(f["color_processing"], "RGB24");
    assert_eq!(f["bits"], 24);
    assert_eq!(f["format"], "jfif");
    assert!(f["bytes_per_line"].as_u64().unwrap() > 0);
    assert!(t[1]["body"]
        .as_str()
        .unwrap()
        .contains("CreateScanJobRequest"));
}

#[tokio::test]
async fn jfif_bw_reject_is_recorded_as_fault() {
    let w = world();
    w.write()
        .await
        .apply_effects(&EffectsBody {
            jfif_bw: Some("reject".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        post_ticket(&w, "ValidateScanTicket", "jfif", "BlackAndWhite1", 100).await,
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        post_ticket(&w, "CreateScanJob", "jfif", "BlackAndWhite1", 100).await,
        StatusCode::INTERNAL_SERVER_ERROR
    );
    let v = tickets(&w, "").await;
    let o0 = v["tickets"][0]["outcome"].as_str().unwrap();
    let o1 = v["tickets"][1]["outcome"].as_str().unwrap();
    assert!(o0.starts_with("fault:wscn:"), "{o0}");
    assert!(o1.starts_with("refused:wscn:"), "{o1}");
    assert_eq!(
        o0.trim_start_matches("fault:"),
        o1.trim_start_matches("refused:")
    );
    assert_eq!(
        v["tickets"][0]["parsed"]["color_processing"],
        "BlackAndWhite1"
    );
}

#[tokio::test]
async fn busy_refusal_and_invalid_ticket_outcomes() {
    let w = world();
    let xml = ticket("ValidateScanTicket", "jfif", "RGB24", 7).replace(
        "<wscn:Resolution>",
        r#"<wscn:Resolution wscn:MustHonor="true">"#,
    );
    send(&w, "POST", "/ScannerService", xml).await;
    w.write().await.set_busy(true);
    assert_eq!(
        post_ticket(&w, "CreateScanJob", "jfif", "RGB24", 200).await,
        StatusCode::INTERNAL_SERVER_ERROR
    );
    let v = tickets(&w, "").await;
    assert_eq!(v["tickets"][0]["outcome"], "invalid", "{v}");
    assert!(v["tickets"][0]["reason"].is_string());
    let o = v["tickets"][1]["outcome"].as_str().unwrap();
    assert!(o.starts_with("refused:wscn:"), "{v}");
}

#[tokio::test]
async fn since_op_and_limit_filter() {
    let w = world();
    validate_then_create(&w).await;
    validate_then_create(&w).await;
    let v = tickets(&w, "?since=2").await;
    assert_eq!(v["count"], 2);
    assert_eq!(v["tickets"][0]["seq"], 3);
    assert_eq!(v["next_seq"], 4);
    let v = tickets(&w, "?op=CreateScanJob").await;
    assert_eq!(v["count"], 2);
    assert!(v["tickets"]
        .as_array()
        .unwrap()
        .iter()
        .all(|t| t["operation"] == "CreateScanJob"));
    let v = tickets(&w, "?op=ValidateScanTicket&since=1&limit=1").await;
    assert_eq!(v["count"], 1);
    assert_eq!(v["tickets"][0]["seq"], 3);
    let v = tickets(&w, "?since=&limit=x").await;
    assert_eq!(v["count"], 4);
}

#[tokio::test]
async fn ring_drops_oldest_beyond_cap() {
    let w = world();
    w.read().await.tickets().set_cap(3);
    validate_then_create(&w).await;
    validate_then_create(&w).await;
    let v = tickets(&w, "").await;
    assert_eq!(v["count"], 3);
    assert_eq!(v["tickets"][0]["seq"], 2, "oldest dropped: {v}");
    assert_eq!(v["next_seq"], 4);
}

#[tokio::test]
async fn body_is_capped_at_32k() {
    let w = world();
    let pad = "x".repeat(40 * 1024);
    let xml = ticket("ValidateScanTicket", "jfif", "RGB24", 200).replace(
        "<wscn:ImagesToTransfer>",
        &format!("<!--{pad}--><wscn:ImagesToTransfer>"),
    );
    send(&w, "POST", "/ScannerService", xml).await;
    let v = tickets(&w, "").await;
    assert_eq!(v["tickets"][0]["body"].as_str().unwrap().len(), 32 * 1024);
}

#[tokio::test]
async fn reset_and_delete_clear_the_ring() {
    let w = world();
    validate_then_create(&w).await;
    let (st, _) = send(&w, "POST", "/v1/reset", String::new()).await;
    assert_eq!(st, StatusCode::OK);
    let v = tickets(&w, "").await;
    assert_eq!(v["count"], 0);
    assert_eq!(v["next_seq"], 2, "seq keeps counting");
    validate_then_create(&w).await;
    assert_eq!(tickets(&w, "").await["count"], 2);
    let (st, _) = send(&w, "DELETE", "/v1/tickets", String::new()).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(tickets(&w, "").await["count"], 0);
}

#[tokio::test]
async fn ctl_tickets_round_trip() {
    let w = world();
    validate_then_create(&w).await;
    let call = |args: Value| fakescan::ctl::CtlRequest {
        cmd: "tickets".into(),
        args,
    };
    let v = fakescan::ctl::dispatch(&w, call(json!({ "op": "CreateScanJob" })))
        .await
        .unwrap();
    assert_eq!(v["count"], 1);
    assert_eq!(v["tickets"][0]["outcome"], "accepted");
    let v = fakescan::ctl::dispatch(&w, call(json!({ "since": 1, "clear": true })))
        .await
        .unwrap();
    assert_eq!(v["count"], 1);
    let v = fakescan::ctl::dispatch(&w, call(Value::Null))
        .await
        .unwrap();
    assert_eq!(v["count"], 0);
}
