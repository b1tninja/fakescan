use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use fakescan::profile::Profile;
use fakescan::state::World;
use fakescan::wsd;
use tokio::sync::RwLock;
use tower::ServiceExt;

fn world() -> Arc<RwLock<World>> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../profiles");
    let profile = Profile::load(&dir, "brother-wsd").expect("profile");
    Arc::new(RwLock::new(World::new(dir, profile)))
}

fn app(world: Arc<RwLock<World>>) -> axum::Router {
    axum::Router::new()
        .route("/ScannerService", axum::routing::post(wsd::scanner_service))
        .with_state(world)
}

async fn post_soap(app: axum::Router, xml: &str) -> (StatusCode, String) {
    let req = Request::builder()
        .method("POST")
        .uri("/ScannerService")
        .header("content-type", "application/soap+xml")
        .body(Body::from(xml.to_string()))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 16 * 1024 * 1024)
        .await
        .unwrap();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

#[tokio::test]
async fn get_scanner_elements_and_create_retrieve() {
    let w = world();
    {
        let mut g = w.write().await;
        g.set_pages(1);
    }

    let elements = r#"<?xml version="1.0"?>
<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope"
  xmlns:wsa="http://schemas.xmlsoap.org/ws/2004/08/addressing"
  xmlns:wscn="http://schemas.microsoft.com/windows/2006/01/wdp/scan">
  <soap:Header>
    <wsa:Action>http://schemas.microsoft.com/windows/2006/01/wdp/scan/GetScannerElements</wsa:Action>
    <wsa:MessageID>uuid:11111111-1111-1111-1111-111111111111</wsa:MessageID>
  </soap:Header>
  <soap:Body>
    <wscn:GetScannerElementsRequest>
      <wscn:RequestedElements>
        <wscn:Name>wscn:ScannerConfiguration</wscn:Name>
      </wscn:RequestedElements>
    </wscn:GetScannerElementsRequest>
  </soap:Body>
</soap:Envelope>"#;

    let (st, body) = post_soap(app(w.clone()), elements).await;
    assert_eq!(st, StatusCode::OK);
    assert!(body.contains("ScannerConfiguration"));
    assert!(body.contains("jfif"));

    let create = r#"<?xml version="1.0"?>
<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope"
  xmlns:wsa="http://schemas.xmlsoap.org/ws/2004/08/addressing"
  xmlns:wscn="http://schemas.microsoft.com/windows/2006/01/wdp/scan">
  <soap:Header>
    <wsa:Action>http://schemas.microsoft.com/windows/2006/01/wdp/scan/CreateScanJob</wsa:Action>
    <wsa:MessageID>uuid:22222222-2222-2222-2222-222222222222</wsa:MessageID>
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
</soap:Envelope>"#;

    let (st, body) = post_soap(app(w.clone()), create).await;
    assert_eq!(st, StatusCode::OK, "{body}");
    assert!(body.contains("<wscn:JobId>"));
    let job_id = body
        .split("<wscn:JobId>")
        .nth(1)
        .unwrap()
        .split('<')
        .next()
        .unwrap()
        .to_string();
    let token = body
        .split("<wscn:JobToken>")
        .nth(1)
        .unwrap()
        .split('<')
        .next()
        .unwrap()
        .to_string();

    let retrieve = format!(
        r#"<?xml version="1.0"?>
<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope"
  xmlns:wsa="http://schemas.xmlsoap.org/ws/2004/08/addressing"
  xmlns:wscn="http://schemas.microsoft.com/windows/2006/01/wdp/scan">
  <soap:Header>
    <wsa:Action>http://schemas.microsoft.com/windows/2006/01/wdp/scan/RetrieveImage</wsa:Action>
    <wsa:MessageID>uuid:33333333-3333-3333-3333-333333333333</wsa:MessageID>
  </soap:Header>
  <soap:Body>
    <wscn:RetrieveImageRequest>
      <wscn:JobId>{job_id}</wscn:JobId>
      <wscn:JobToken>{token}</wscn:JobToken>
    </wscn:RetrieveImageRequest>
  </soap:Body>
</soap:Envelope>"#
    );

    let req = Request::builder()
        .method("POST")
        .uri("/ScannerService")
        .header("content-type", "application/soap+xml")
        .body(Body::from(retrieve))
        .unwrap();
    let res = app(w).oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let ct = res
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(ct.contains("multipart/related"), "{ct}");
    let bytes = axum::body::to_bytes(res.into_body(), 16 * 1024 * 1024)
        .await
        .unwrap();
    assert!(bytes.windows(3).any(|w| w == b"\xff\xd8\xff"), "jpeg soi");
}
