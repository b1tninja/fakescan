//! `jfif_gray` effect: how a Grayscale8 + JFIF/EXIF ticket is answered over SOAP.
//!
//! Live 2026-10-12 (Windows 11): the legacy RGB24 coercion of a gray JFIF ticket makes
//! wsdscdrv skip RetrieveImage; consistent gray (`gray8`, the default) works.
//! The wire shape is switchable live; these tests pin each mode.

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use fakescan::effects::EffectsBody;
use fakescan::profile::Profile;
use fakescan::state::World;
use fakescan::wsd;
use tokio::sync::RwLock;
use tower::ServiceExt;

fn world() -> Arc<RwLock<World>> {
    world_profile("brother-wsd")
}

fn world_profile(id: &str) -> Arc<RwLock<World>> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../profiles");
    let profile = Profile::load(&dir, id).expect("profile");
    Arc::new(RwLock::new(World::new(dir, profile)))
}

fn app(world: Arc<RwLock<World>>) -> axum::Router {
    axum::Router::new()
        .route("/ScannerService", axum::routing::post(wsd::scanner_service))
        .with_state(world)
}

async fn set_mode(w: &Arc<RwLock<World>>, mode: &str) {
    w.write()
        .await
        .apply_effects(&EffectsBody {
            jfif_gray: Some(mode.into()),
            ..Default::default()
        })
        .unwrap();
}

async fn post(w: &Arc<RwLock<World>>, xml: String) -> (StatusCode, Vec<u8>) {
    let req = Request::builder()
        .method("POST")
        .uri("/ScannerService")
        .header("content-type", "application/soap+xml")
        .body(Body::from(xml))
        .unwrap();
    let res = app(w.clone()).oneshot(req).await.unwrap();
    let st = res.status();
    let b = axum::body::to_bytes(res.into_body(), 64 * 1024 * 1024)
        .await
        .unwrap();
    (st, b.to_vec())
}

fn ticket(action: &str, format: &str, source: &str, color: &str, n: u32) -> String {
    format!(
        r#"<?xml version="1.0"?>
<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope"
  xmlns:wsa="http://schemas.xmlsoap.org/ws/2004/08/addressing"
  xmlns:wscn="http://schemas.microsoft.com/windows/2006/08/wdp/scan">
  <soap:Header>
    <wsa:Action>http://schemas.microsoft.com/windows/2006/08/wdp/scan/{action}</wsa:Action>
    <wsa:MessageID>urn:uuid:aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa</wsa:MessageID>
  </soap:Header>
  <soap:Body>
    <wscn:{action}Request>
      <wscn:ScanTicket>
        <wscn:JobDescription>
          <wscn:JobName>gray</wscn:JobName>
          <wscn:JobOriginatingUserName>lab</wscn:JobOriginatingUserName>
        </wscn:JobDescription>
        <wscn:DocumentParameters>
          <wscn:Format>{format}</wscn:Format>
          <wscn:ImagesToTransfer>{n}</wscn:ImagesToTransfer>
          <wscn:InputSource>{source}</wscn:InputSource>
          <wscn:MediaSides>
            <wscn:MediaFront>
              <wscn:ColorProcessing>{color}</wscn:ColorProcessing>
              <wscn:Resolution>
                <wscn:Width>100</wscn:Width>
                <wscn:Height>100</wscn:Height>
              </wscn:Resolution>
            </wscn:MediaFront>
          </wscn:MediaSides>
        </wscn:DocumentParameters>
      </wscn:ScanTicket>
    </wscn:{action}Request>
  </soap:Body>
</soap:Envelope>"#
    )
}

fn tag(body: &str, name: &str) -> String {
    let open = format!("<wscn:{name}");
    let at = body
        .find(&open)
        .unwrap_or_else(|| panic!("no {name}: {body}"));
    let rest = &body[at..];
    let gt = rest.find('>').unwrap();
    rest[gt + 1..].split('<').next().unwrap().to_string()
}

fn retrieve_xml(job_id: &str, token: &str) -> String {
    format!(
        r#"<?xml version="1.0"?>
<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope"
  xmlns:wsa="http://schemas.xmlsoap.org/ws/2004/08/addressing"
  xmlns:wscn="http://schemas.microsoft.com/windows/2006/08/wdp/scan">
  <soap:Header>
    <wsa:Action>http://schemas.microsoft.com/windows/2006/08/wdp/scan/RetrieveImage</wsa:Action>
    <wsa:MessageID>urn:uuid:bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb</wsa:MessageID>
  </soap:Header>
  <soap:Body>
    <wscn:RetrieveImageRequest>
      <wscn:JobId>{job_id}</wscn:JobId>
      <wscn:JobToken>{token}</wscn:JobToken>
    </wscn:RetrieveImageRequest>
  </soap:Body>
</soap:Envelope>"#
    )
}

/// Slice of the MTOM body from the first `magic` to the closing MIME boundary.
fn payload_from<'a>(mtom: &'a [u8], magic: &[u8]) -> &'a [u8] {
    let at = mtom
        .windows(magic.len())
        .position(|w| w == magic)
        .expect("payload magic in MTOM body");
    let rest = &mtom[at..];
    let end = rest
        .windows(4)
        .rposition(|w| w == b"\r\n--")
        .unwrap_or(rest.len());
    &rest[..end]
}

struct Run {
    create: String,
    mtom: Vec<u8>,
}

async fn create_retrieve(w: &Arc<RwLock<World>>, format: &str, source: &str, n: u32) -> Run {
    let (st, b) = post(w, ticket("CreateScanJob", format, source, "Grayscale8", n)).await;
    let create = String::from_utf8_lossy(&b).into_owned();
    assert_eq!(st, StatusCode::OK, "{create}");
    let (st, mtom) = post(
        w,
        retrieve_xml(&tag(&create, "JobId"), &tag(&create, "JobToken")),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    Run { create, mtom }
}

fn bpl_and_width(body: &str) -> (u32, u32) {
    (
        tag(body, "BytesPerLine").parse().unwrap(),
        tag(body, "PixelsPerLine").parse().unwrap(),
    )
}

/// Text of every `<wscn:ColorProcessing ...>X</wscn:ColorProcessing>` element.
fn color_tokens(body: &str) -> Vec<String> {
    body.split("<wscn:ColorProcessing")
        .skip(1)
        .map(|s| {
            let after = &s[s.find('>').unwrap() + 1..];
            after.split('<').next().unwrap().to_string()
        })
        .collect()
}

#[tokio::test]
async fn default_mode_is_gray8_single_component() {
    let w = world();
    assert_eq!(w.read().await.status().effects.jfif_gray, "gray8");
    let run = create_retrieve(&w, "jfif", "Platen", 1).await;
    assert_eq!(
        color_tokens(&run.create),
        vec!["Grayscale8"],
        "{}",
        run.create
    );
    let (bpl, width) = bpl_and_width(&run.create);
    assert_eq!(bpl, width);
    let jpeg = payload_from(&run.mtom, &[0xFF, 0xD8, 0xFF]);
    assert_eq!(fakescan::image_gen::jpeg_sof0_components(jpeg).len(), 1);
}

/// Legacy opt-in: the old coercion Windows rejects, kept for tests.
#[tokio::test]
async fn legacy_rgb24_mode_coerces_gray_to_rgb() {
    let w = world();
    set_mode(&w, "rgb24").await;
    assert_eq!(w.read().await.status().effects.jfif_gray, "rgb24");
    let run = create_retrieve(&w, "jfif", "Platen", 1).await;
    assert_eq!(color_tokens(&run.create), vec!["RGB24"], "{}", run.create);
    let (bpl, width) = bpl_and_width(&run.create);
    assert_eq!(bpl, width * 3);
    let jpeg = payload_from(&run.mtom, &[0xFF, 0xD8, 0xFF]);
    assert_eq!(fakescan::image_gen::jpeg_sof0_components(jpeg).len(), 3);
}

#[tokio::test]
async fn gray8_mode_is_consistent_end_to_end() {
    let w = world();
    set_mode(&w, "gray8").await;
    assert_eq!(w.read().await.status().effects.jfif_gray, "gray8");
    // Validate agrees with Create.
    let (_, v) = post(
        &w,
        ticket("ValidateScanTicket", "jfif", "Platen", "Grayscale8", 1),
    )
    .await;
    let v = String::from_utf8_lossy(&v).into_owned();
    assert!(
        v.contains("<wscn:ValidTicket>true</wscn:ValidTicket>"),
        "{v}"
    );
    let (vbpl, vw) = bpl_and_width(&v);
    assert_eq!(vbpl, vw, "Validate BytesPerLine = width for gray8: {v}");

    let run = create_retrieve(&w, "jfif", "Platen", 1).await;
    assert_eq!(
        color_tokens(&run.create),
        vec!["Grayscale8"],
        "{}",
        run.create
    );
    assert!(!run.create.contains("RGB24"), "{}", run.create);
    assert!(
        !run.create.contains("Override=\"true\">Grayscale8"),
        "gray is not an override: {}",
        run.create
    );
    let (bpl, width) = bpl_and_width(&run.create);
    assert_eq!((bpl, width), (816, 816));
    let jpeg = payload_from(&run.mtom, &[0xFF, 0xD8, 0xFF]);
    let comps = fakescan::image_gen::jpeg_sof0_components(jpeg);
    assert_eq!(comps.len(), 1, "one gray component: {comps:?}");
    assert_eq!(
        fakescan::image_gen::jpeg_sof0_dimensions(jpeg),
        Some((816, 1144))
    );
    let img = image::load_from_memory(jpeg).unwrap();
    assert_eq!(img.color(), image::ColorType::L8);
    assert_eq!((img.width(), img.height()), (816, 1144));
}

#[tokio::test]
async fn gray8_rgb_stride_mode_gray_token_and_triple_stride() {
    let w = world();
    set_mode(&w, "gray8_rgb_stride").await;
    let run = create_retrieve(&w, "jfif", "Platen", 1).await;
    assert_eq!(
        color_tokens(&run.create),
        vec!["Grayscale8"],
        "{}",
        run.create
    );
    let (bpl, width) = bpl_and_width(&run.create);
    assert_eq!(bpl, width * 3);
    let jpeg = payload_from(&run.mtom, &[0xFF, 0xD8, 0xFF]);
    assert_eq!(fakescan::image_gen::jpeg_sof0_components(jpeg).len(), 1);
}

#[tokio::test]
async fn gray8_tiff_mode_overrides_format_and_delivers_gray_tiff() {
    let w = world();
    set_mode(&w, "gray8_tiff").await;
    let run = create_retrieve(&w, "jfif", "Platen", 1).await;
    assert!(
        run.create
            .contains("Override=\"true\">tiff-single-uncompressed</wscn:Format>"),
        "{}",
        run.create
    );
    assert_eq!(
        color_tokens(&run.create),
        vec!["Grayscale8"],
        "{}",
        run.create
    );
    let (bpl, width) = bpl_and_width(&run.create);
    assert_eq!(bpl, width);
    let tiff = payload_from(&run.mtom, &[0x49, 0x49, 0x2A, 0x00]);
    let mut dec = tiff::decoder::Decoder::new(std::io::Cursor::new(tiff)).unwrap();
    assert_eq!(dec.colortype().unwrap(), tiff::ColorType::Gray(8));
    assert_eq!(dec.dimensions().unwrap(), (816, 1144));
}

#[tokio::test]
async fn exif_follows_gray8_mode_and_keeps_app1() {
    let w = world_profile("wia-rich");
    set_mode(&w, "gray8").await;
    let run = create_retrieve(&w, "exif", "Platen", 1).await;
    assert_eq!(
        color_tokens(&run.create),
        vec!["Grayscale8"],
        "{}",
        run.create
    );
    let jpeg = payload_from(&run.mtom, &[0xFF, 0xD8, 0xFF]);
    assert!(jpeg.windows(6).any(|w| w == b"Exif\0\0"), "EXIF APP1 kept");
    assert_eq!(fakescan::image_gen::jpeg_sof0_components(jpeg).len(), 1);
}

#[tokio::test]
async fn rgb_tickets_keep_rgb_stride_and_three_components() {
    let w = world();
    set_mode(&w, "gray8").await;
    let (_, b) = post(&w, ticket("CreateScanJob", "jfif", "Platen", "RGB24", 1)).await;
    let create = String::from_utf8_lossy(&b).into_owned();
    assert_eq!(color_tokens(&create), vec!["RGB24"]);
    let (bpl, width) = bpl_and_width(&create);
    assert_eq!(bpl, width * 3);
}

#[tokio::test]
async fn gray8_adf_two_sheets_retrieve_twice() {
    let w = world();
    {
        let mut g = w.write().await;
        g.set_pages(2);
    }
    set_mode(&w, "gray8").await;
    let (st, b) = post(&w, ticket("CreateScanJob", "jfif", "ADF", "Grayscale8", 2)).await;
    let create = String::from_utf8_lossy(&b).into_owned();
    assert_eq!(st, StatusCode::OK, "{create}");
    assert_eq!(color_tokens(&create), vec!["Grayscale8"]);
    for _ in 0..2 {
        let (st, mtom) = post(
            &w,
            retrieve_xml(&tag(&create, "JobId"), &tag(&create, "JobToken")),
        )
        .await;
        assert_eq!(st, StatusCode::OK);
        let jpeg = payload_from(&mtom, &[0xFF, 0xD8, 0xFF]);
        assert_eq!(fakescan::image_gen::jpeg_sof0_components(jpeg).len(), 1);
    }
}

#[tokio::test]
async fn unknown_mode_rejected_presets_keep_it_clear_and_reset_drop_it() {
    let w = world();
    let bad = w.write().await.apply_effects(&EffectsBody {
        jfif_gray: Some("purple".into()),
        ..Default::default()
    });
    assert!(bad.is_err());
    set_mode(&w, "rgb24").await;
    w.write()
        .await
        .apply_effects(&EffectsBody {
            preset: Some("dirty_adf".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(w.read().await.status().effects.jfif_gray, "rgb24");
    w.write()
        .await
        .apply_effects(&EffectsBody {
            clear: true,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(w.read().await.status().effects.jfif_gray, "gray8");
    set_mode(&w, "gray8_tiff").await;
    w.write().await.reset();
    assert_eq!(w.read().await.status().effects.jfif_gray, "gray8");
}

#[tokio::test]
async fn mode_is_isolated_per_world() {
    let a = world();
    let b = world();
    set_mode(&a, "rgb24").await;
    assert_eq!(a.read().await.status().effects.jfif_gray, "rgb24");
    assert_eq!(b.read().await.status().effects.jfif_gray, "gray8");
    let run = create_retrieve(&b, "jfif", "Platen", 1).await;
    assert_eq!(color_tokens(&run.create), vec!["Grayscale8"]);
}
