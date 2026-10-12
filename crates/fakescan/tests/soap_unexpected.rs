//! What the scanner does when a SOAP header or body value is not the expected one.
//!
//! Reply headers and the response body are still checked. Request-shape misses
//! (no JobDescription, and so on) are reported by `body_problems` and do not fail
//! the call.

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use fakescan::profile::Profile;
use fakescan::state::World;
use fakescan::wsd;
use fakescan::wsd::soap::{body_problems, reply_problems};
use tokio::sync::RwLock;
use tower::ServiceExt;

fn app() -> axum::Router {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../profiles");
    let profile = Profile::load(&dir, "brother-wsd").expect("profile");
    let world = Arc::new(RwLock::new(World::new(dir, profile)));
    axum::Router::new()
        .route("/ScannerService", axum::routing::post(wsd::scanner_service))
        .with_state(world)
}

async fn post(xml: &str) -> (StatusCode, String) {
    let req = Request::builder()
        .method("POST")
        .uri("/ScannerService")
        .header("content-type", "application/soap+xml")
        .body(Body::from(xml.to_string()))
        .unwrap();
    let res = app().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 16 * 1024 * 1024)
        .await
        .unwrap();
    let body = String::from_utf8_lossy(&bytes).into_owned();
    let mut problems = reply_problems(&body);
    problems.extend(body_problems(&body));
    assert!(problems.is_empty(), "schema: {problems:?}\n{body}");
    (status, body)
}

fn create_ticket(header: &str) -> String {
    format!(
        r#"<?xml version="1.0"?>
<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope"
  xmlns:wsa="http://schemas.xmlsoap.org/ws/2004/08/addressing"
  xmlns:wscn="http://schemas.microsoft.com/windows/2006/01/wdp/scan">
  <soap:Header>
    {header}
  </soap:Header>
  <soap:Body>
    <wscn:CreateScanJobRequest>
      <wscn:ScanTicket>
        <wscn:DocumentParameters>
          <wscn:Format>jfif</wscn:Format>
          <wscn:InputSource>Platen</wscn:InputSource>
          <wscn:MediaSides>
            <wscn:MediaFront>
              <wscn:Resolution>
                <wscn:Width>200</wscn:Width>
                <wscn:Height>200</wscn:Height>
              </wscn:Resolution>
            </wscn:MediaFront>
          </wscn:MediaSides>
        </wscn:DocumentParameters>
      </wscn:ScanTicket>
    </wscn:CreateScanJobRequest>
  </soap:Body>
</soap:Envelope>"#
    )
}

const ANON: &str = "http://schemas.xmlsoap.org/ws/2004/08/addressing/role/anonymous";

#[tokio::test]
async fn missing_message_id_still_creates_a_job() {
    let xml = create_ticket(
        r#"<wsa:Action>http://schemas.microsoft.com/windows/2006/01/wdp/scan/CreateScanJob</wsa:Action>"#,
    );
    let (status, body) = post(&xml).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!body.contains("<wsa:RelatesTo>"), "{body}");
    assert!(body.contains("<wsa:MessageID>uuid:"), "{body}");
    assert!(body.contains(&format!("<wsa:To>{ANON}</wsa:To>")), "{body}");
}

#[tokio::test]
async fn bare_uuid_message_id_is_not_echoed() {
    let xml = create_ticket(
        r#"<wsa:Action>http://schemas.microsoft.com/windows/2006/01/wdp/scan/CreateScanJob</wsa:Action>
           <wsa:MessageID>bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb</wsa:MessageID>"#,
    );
    let (status, body) = post(&xml).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!body.contains("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"), "{body}");
    assert!(body.contains("<wsa:MessageID>uuid:"), "{body}");
}

#[tokio::test]
async fn repeated_message_id_is_echoed_and_the_reply_id_is_fresh() {
    let id = "uuid:aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    let header = format!(
        r#"<wsa:Action>http://schemas.microsoft.com/windows/2006/01/wdp/scan/CreateScanJob</wsa:Action>
           <wsa:MessageID>{id}</wsa:MessageID>"#
    );
    let (status, body) = post(&create_ticket(&header)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains(&format!("<wsa:RelatesTo>{id}</wsa:RelatesTo>")), "{body}");
    let reply_id = body
        .split("<wsa:MessageID>")
        .nth(1)
        .unwrap()
        .split('<')
        .next()
        .unwrap();
    assert_ne!(reply_id, id);
    assert!(reply_id.starts_with("uuid:"));
}

#[tokio::test]
async fn missing_action_uses_the_body_element() {
    let xml = create_ticket(r#"<wsa:MessageID>uuid:cccccccc-cccc-4ccc-8ccc-cccccccccccc</wsa:MessageID>"#);
    let (status, body) = post(&xml).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("CreateScanJobResponse"), "{body}");
}

#[tokio::test]
async fn non_iri_action_uses_the_body_element() {
    let xml = create_ticket(
        r#"<wsa:Action>CreateScanJob</wsa:Action>
           <wsa:MessageID>uuid:dddddddd-dddd-4ddd-8ddd-dddddddddddd</wsa:MessageID>"#,
    );
    let (status, body) = post(&xml).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("CreateScanJobResponse"), "{body}");
}

#[tokio::test]
async fn unknown_must_understand_faults() {
    let xml = create_ticket(
        r#"<wsa:Action soap:mustUnderstand="true">http://schemas.microsoft.com/windows/2006/01/wdp/scan/CreateScanJob</wsa:Action>
           <wsa:MessageID>uuid:eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee</wsa:MessageID>
           <ex:Trace soap:mustUnderstand="1">no</ex:Trace>"#,
    );
    let (status, body) = post(&xml).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
    assert!(body.contains("MustUnderstand"), "{body}");
}

#[tokio::test]
async fn must_understand_on_action_is_accepted() {
    let xml = create_ticket(
        r#"<wsa:Action soap:mustUnderstand="true">http://schemas.microsoft.com/windows/2006/01/wdp/scan/CreateScanJob</wsa:Action>
           <wsa:MessageID>uuid:ffffffff-ffff-4fff-8fff-ffffffffffff</wsa:MessageID>"#,
    );
    let (status, body) = post(&xml).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("CreateScanJobResponse"), "{body}");
}

#[tokio::test]
async fn request_to_is_not_used_for_routing() {
    let xml = create_ticket(
        r#"<wsa:To>http://example.test/not-the-scanner</wsa:To>
           <wsa:Action>http://schemas.microsoft.com/windows/2006/01/wdp/scan/CreateScanJob</wsa:Action>
           <wsa:MessageID>uuid:12121212-1212-4121-8121-121212121212</wsa:MessageID>"#,
    );
    let (status, body) = post(&xml).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains(&format!("<wsa:To>{ANON}</wsa:To>")), "{body}");
    assert!(!body.contains("example.test"), "{body}");
}

#[tokio::test]
async fn omitted_job_description_still_creates_a_job() {
    let xml = create_ticket(
        r#"<wsa:Action>http://schemas.microsoft.com/windows/2006/01/wdp/scan/CreateScanJob</wsa:Action>
           <wsa:MessageID>uuid:13131313-1313-4131-8131-131313131313</wsa:MessageID>"#,
    );
    let request = body_problems(&xml);
    assert!(request.iter().any(|p| p.contains("missing JobDescription")), "{request:?}");
    let (status, body) = post(&xml).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("<wscn:JobId>"), "{body}");
}

#[tokio::test]
async fn must_honor_unknown_dpi_faults() {
    let xml = r#"<?xml version="1.0"?>
<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope"
  xmlns:wsa="http://schemas.xmlsoap.org/ws/2004/08/addressing"
  xmlns:wscn="http://schemas.microsoft.com/windows/2006/01/wdp/scan">
  <soap:Header>
    <wsa:Action>http://schemas.microsoft.com/windows/2006/01/wdp/scan/CreateScanJob</wsa:Action>
    <wsa:MessageID>uuid:14141414-1414-4141-8141-141414141414</wsa:MessageID>
  </soap:Header>
  <soap:Body>
    <wscn:CreateScanJobRequest>
      <wscn:ScanTicket>
        <wscn:DocumentParameters>
          <wscn:Format>jfif</wscn:Format>
          <wscn:InputSource>Platen</wscn:InputSource>
          <wscn:MediaSides>
            <wscn:MediaFront>
              <wscn:Resolution wscn:MustHonor="true">
                <wscn:Width>999</wscn:Width>
                <wscn:Height>999</wscn:Height>
              </wscn:Resolution>
            </wscn:MediaFront>
          </wscn:MediaSides>
        </wscn:DocumentParameters>
      </wscn:ScanTicket>
    </wscn:CreateScanJobRequest>
  </soap:Body>
</soap:Envelope>"#;
    let (status, body) = post(xml).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
    assert!(body.contains("Fault"), "{body}");
}

#[tokio::test]
async fn retrieve_without_job_id_faults() {
    let xml = r#"<?xml version="1.0"?>
<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope"
  xmlns:wsa="http://schemas.xmlsoap.org/ws/2004/08/addressing"
  xmlns:wscn="http://schemas.microsoft.com/windows/2006/01/wdp/scan">
  <soap:Header>
    <wsa:Action>http://schemas.microsoft.com/windows/2006/01/wdp/scan/RetrieveImage</wsa:Action>
    <wsa:MessageID>uuid:15151515-1515-4151-8151-151515151515</wsa:MessageID>
  </soap:Header>
  <soap:Body>
    <wscn:RetrieveImageRequest>
      <wscn:JobToken>urn:uuid:aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa</wscn:JobToken>
    </wscn:RetrieveImageRequest>
  </soap:Body>
</soap:Envelope>"#;
    let (status, body) = post(xml).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
    assert!(body.contains("JobId"), "{body}");
}
