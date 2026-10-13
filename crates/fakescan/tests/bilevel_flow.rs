//! BlackAndWhite1 / Grayscale8 tickets produce real 1-bit / 8-bit pages over SOAP.

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
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../profiles");
    let profile = Profile::load(&dir, "brother-wsd").expect("profile");
    Arc::new(RwLock::new(World::new(dir, profile)))
}

fn app(w: Arc<RwLock<World>>) -> axum::Router {
    axum::Router::new()
        .route("/ScannerService", axum::routing::post(wsd::scanner_service))
        .with_state(w)
}

async fn post(app: axum::Router, xml: String) -> (StatusCode, Vec<u8>) {
    let req = Request::builder()
        .method("POST")
        .uri("/ScannerService")
        .header("content-type", "application/soap+xml")
        .body(Body::from(xml))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    let st = res.status();
    let b = axum::body::to_bytes(res.into_body(), 64 * 1024 * 1024)
        .await
        .unwrap();
    (st, b.to_vec())
}

fn envelope(action: &str, inner: &str) -> String {
    format!(
        r#"<?xml version="1.0"?>
<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope"
  xmlns:wsa="http://schemas.xmlsoap.org/ws/2004/08/addressing"
  xmlns:wscn="http://schemas.microsoft.com/windows/2006/01/wdp/scan">
  <soap:Header>
    <wsa:Action>http://schemas.microsoft.com/windows/2006/01/wdp/scan/{action}</wsa:Action>
    <wsa:MessageID>uuid:b1b1b1b1-b1b1-b1b1-b1b1-b1b1b1b1b1b1</wsa:MessageID>
  </soap:Header>
  <soap:Body>{inner}</soap:Body>
</soap:Envelope>"#
    )
}

fn create_xml(format: &str, color: &str, dpi: u32) -> String {
    envelope(
        "CreateScanJob",
        &format!(
            r#"<wscn:CreateScanJobRequest><wscn:ScanTicket><wscn:DocumentParameters>
  <wscn:Format>{format}</wscn:Format>
  <wscn:ImagesToTransfer>1</wscn:ImagesToTransfer>
  <wscn:InputSource>Platen</wscn:InputSource>
  <wscn:MediaSides><wscn:MediaFront>
    <wscn:ColorProcessing>{color}</wscn:ColorProcessing>
    <wscn:Resolution><wscn:Width>{dpi}</wscn:Width><wscn:Height>{dpi}</wscn:Height></wscn:Resolution>
  </wscn:MediaFront></wscn:MediaSides>
</wscn:DocumentParameters></wscn:ScanTicket></wscn:CreateScanJobRequest>"#
        ),
    )
}

fn tag<'a>(body: &'a str, name: &str) -> &'a str {
    body.split(&format!("<{name}>"))
        .nth(1)
        .and_then(|s| s.split('<').next())
        .unwrap_or_else(|| panic!("missing {name} in {body}"))
}

/// CreateScanJob + RetrieveImage; returns (create response text, raw MTOM bytes).
async fn scan(w: &Arc<RwLock<World>>, format: &str, color: &str, dpi: u32) -> (String, Vec<u8>) {
    let (st, body) = post(app(w.clone()), create_xml(format, color, dpi)).await;
    let body = String::from_utf8_lossy(&body).into_owned();
    assert_eq!(st, StatusCode::OK, "{body}");
    let id = tag(&body, "wscn:JobId");
    let token = tag(&body, "wscn:JobToken");
    let (st, raw) = post(
        app(w.clone()),
        envelope(
            "RetrieveImage",
            &format!(
                "<wscn:RetrieveImageRequest><wscn:JobId>{id}</wscn:JobId><wscn:JobToken>{token}</wscn:JobToken></wscn:RetrieveImageRequest>"
            ),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    (body, raw)
}

fn find(hay: &[u8], needle: &[u8]) -> usize {
    hay.windows(needle.len())
        .position(|w| w == needle)
        .expect("payload magic")
}

#[tokio::test]
async fn bw1_tiff_single_uncompressed_is_one_bit() {
    for (pol, photo) in [("black_is_zero", 1u16), ("white_is_zero", 0u16)] {
        let w = world();
        w.write()
            .await
            .apply_effects(&EffectsBody {
                bilevel_polarity: Some(pol.into()),
                ..Default::default()
            })
            .unwrap();
        let (create, raw) = scan(&w, "tiff-single-uncompressed", "BlackAndWhite1", 100).await;
        let width: u32 = tag(&create, "wscn:PixelsPerLine").parse().unwrap();
        let bpl: u32 = tag(&create, "wscn:BytesPerLine").parse().unwrap();
        assert_eq!(bpl, width.div_ceil(8), "BPL is the 1-bit stride");
        let tiff = &raw[find(&raw, &[0x49, 0x49, 0x2A, 0x00])..];
        let mut dec = tiff::decoder::Decoder::new(std::io::Cursor::new(tiff)).unwrap();
        assert_eq!(dec.colortype().unwrap(), tiff::ColorType::Gray(1));
        assert_eq!(dec.dimensions().unwrap().0, width);
        let ph = dec
            .get_tag_u32(tiff::tags::Tag::PhotometricInterpretation)
            .unwrap();
        assert_eq!(ph, u32::from(photo), "{pol}");
        assert!(String::from_utf8_lossy(&raw).contains("image/tiff"));
    }
}

#[tokio::test]
async fn gray8_tiff_is_eight_bit_single_channel() {
    let w = world();
    let (create, raw) = scan(&w, "tiff-single-uncompressed", "Grayscale8", 100).await;
    let width: u32 = tag(&create, "wscn:PixelsPerLine").parse().unwrap();
    assert_eq!(
        tag(&create, "wscn:BytesPerLine").parse::<u32>().unwrap(),
        width
    );
    let tiff = &raw[find(&raw, &[0x49, 0x49, 0x2A, 0x00])..];
    let mut dec = tiff::decoder::Decoder::new(std::io::Cursor::new(tiff)).unwrap();
    assert_eq!(dec.colortype().unwrap(), tiff::ColorType::Gray(8));
}

fn validate_xml(format: &str, color: &str, dpi: u32) -> String {
    create_xml(format, color, dpi)
        .replace("CreateScanJob", "ValidateScanTicket")
        .replace("CreateScanJobRequest", "ValidateScanTicketRequest")
}

fn color_tokens(body: &str) -> Vec<String> {
    body.split("<wscn:ColorProcessing")
        .skip(1)
        .map(|s| {
            let after = &s[s.find('>').unwrap() + 1..];
            after.split('<').next().unwrap().to_string()
        })
        .collect()
}

/// BW1 over JPEG-family formats, both `jfif_bw` modes: Validate and Create agree, and the
/// stream matches the answer (gray8: Grayscale8 + BPL=width + 1 component; rgb24: the legacy
/// Brother-style RGB24 + BPL=width*3 + 3 components). Samples are bilevel either way.
#[tokio::test]
async fn bw1_jpeg_family_answers_follow_jfif_bw() {
    for (mode, token, mult, comps) in [
        (None, "Grayscale8", 1u32, 1usize), // default
        (Some("gray8"), "Grayscale8", 1, 1),
        (Some("rgb24"), "RGB24", 3, 3),
    ] {
        for format in ["jfif", "jpeg", "exif"] {
            let w = world();
            if let Some(m) = mode {
                w.write()
                    .await
                    .apply_effects(&EffectsBody {
                        jfif_bw: Some(m.into()),
                        ..Default::default()
                    })
                    .unwrap();
            }
            let tag_s = format!("{format} {mode:?}");
            // ValidateScanTicket.
            let (st, v) = post(app(w.clone()), validate_xml(format, "BlackAndWhite1", 100)).await;
            let v = String::from_utf8_lossy(&v).into_owned();
            assert_eq!(st, StatusCode::OK, "{tag_s}: {v}");
            let vw: u32 = tag(&v, "wscn:PixelsPerLine").parse().unwrap();
            assert_eq!(
                tag(&v, "wscn:BytesPerLine").parse::<u32>().unwrap(),
                vw * mult,
                "{tag_s}: {v}"
            );
            // CreateScanJob + RetrieveImage.
            let (create, raw) = scan(&w, format, "BlackAndWhite1", 100).await;
            assert_eq!(color_tokens(&create), vec![token], "{tag_s}: {create}");
            assert!(create.contains(r#"Override="true""#), "{tag_s}: {create}");
            let width: u32 = tag(&create, "wscn:PixelsPerLine").parse().unwrap();
            assert_eq!(width, vw, "{tag_s}");
            assert_eq!(
                tag(&create, "wscn:BytesPerLine").parse::<u32>().unwrap(),
                width * mult,
                "{tag_s}"
            );
            let jpg = &raw[find(&raw, &[0xFF, 0xD8, 0xFF])..];
            assert_eq!(
                fakescan::image_gen::jpeg_sof0_components(jpg).len(),
                comps,
                "{tag_s}"
            );
            let img = image::load_from_memory_with_format(jpg, image::ImageFormat::Jpeg).unwrap();
            assert_eq!(img.width(), width, "{tag_s}");
            let img = img.to_luma8();
            let n = img.pixels().count();
            let extreme = img
                .pixels()
                .filter(|p| p.0[0] <= 40 || p.0[0] >= 215)
                .count();
            assert!(extreme * 100 >= n * 97, "{tag_s}: {extreme}/{n}");
        }
    }
}

/// The 1-bit formats keep their real BW answer whatever `jfif_bw` says.
#[tokio::test]
async fn jfif_bw_does_not_touch_real_one_bit_formats() {
    for mode in ["gray8", "rgb24"] {
        let w = world();
        w.write()
            .await
            .apply_effects(&EffectsBody {
                jfif_bw: Some(mode.into()),
                ..Default::default()
            })
            .unwrap();
        let (create, _) = scan(&w, "tiff-single-uncompressed", "BlackAndWhite1", 100).await;
        assert_eq!(color_tokens(&create), vec!["BlackAndWhite1"], "{mode}");
        let width: u32 = tag(&create, "wscn:PixelsPerLine").parse().unwrap();
        assert_eq!(
            tag(&create, "wscn:BytesPerLine").parse::<u32>().unwrap(),
            width.div_ceil(8)
        );
    }
}

#[tokio::test]
async fn jfif_bw_effect_api_and_presets() {
    let w = world();
    assert_eq!(w.read().await.status().effects.jfif_bw, "gray8");
    let set = |m: &str| EffectsBody {
        jfif_bw: Some(m.into()),
        ..Default::default()
    };
    w.write().await.apply_effects(&set("rgb24")).unwrap();
    assert_eq!(w.read().await.status().effects.jfif_bw, "rgb24");
    // Presets keep the format knob; clear resets it.
    w.write()
        .await
        .apply_effects(&EffectsBody {
            preset: Some("skewed".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(w.read().await.status().effects.jfif_bw, "rgb24");
    assert!(w.write().await.apply_effects(&set("purple")).is_err());
    assert_eq!(w.read().await.status().effects.jfif_bw, "rgb24");
    w.write()
        .await
        .apply_effects(&EffectsBody {
            clear: true,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(w.read().await.status().effects.jfif_bw, "gray8");
}

/// `jfif_bw=reject`: BW1 over a JPEG-family Format is refused in Validate and Create
/// (spec: unsupported Format+colour -> ClientErrorDocumentFormatNotSupported); everything
/// else (other formats, other colours) is unaffected.
#[tokio::test]
async fn jfif_bw_reject_faults_validate_and_create() {
    let w = world();
    w.write()
        .await
        .apply_effects(&EffectsBody {
            jfif_bw: Some("reject".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(w.read().await.status().effects.jfif_bw, "reject");
    for format in ["jfif", "jpeg", "exif"] {
        for xml in [
            validate_xml(format, "BlackAndWhite1", 100),
            create_xml(format, "BlackAndWhite1", 100),
        ] {
            let (_, b) = post(app(w.clone()), xml).await;
            let b = String::from_utf8_lossy(&b).into_owned();
            assert!(b.contains("Fault"), "{format}: {b}");
            assert!(
                b.contains("FormatNotSupported"),
                "{format}: expected format-not-supported fault: {b}"
            );
        }
    }
    // No job was created by the refused Creates.
    // Unaffected: other colours over JFIF, BW1 over real 1-bit formats.
    for (format, color) in [
        ("jfif", "Grayscale8"),
        ("jfif", "RGB24"),
        ("exif", "RGB24"),
        ("tiff-single-uncompressed", "BlackAndWhite1"),
    ] {
        let (v, _) = {
            let (st, b) = post(app(w.clone()), validate_xml(format, color, 100)).await;
            (String::from_utf8_lossy(&b).into_owned(), st)
        };
        assert!(!v.contains("Fault"), "{format} {color}: {v}");
        let (create, _) = scan(&w, format, color, 100).await;
        assert!(!create.contains("Fault"), "{format} {color}: {create}");
    }
    // Switching back restores service.
    w.write()
        .await
        .apply_effects(&EffectsBody {
            jfif_bw: Some("gray8".into()),
            ..Default::default()
        })
        .unwrap();
    let (create, _) = scan(&w, "jfif", "BlackAndWhite1", 100).await;
    assert_eq!(color_tokens(&create), vec!["Grayscale8"]);
}
