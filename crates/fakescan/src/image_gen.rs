//! Synthetic page bitmaps for RetrieveImage.

use anyhow::{bail, Result};
use image::{ImageBuffer, Rgb, RgbImage};
use qrcode::{EcLevel, QrCode};
use serde::{Deserialize, Serialize};

/// Letter width in 1/1000 inch (WSD InputMediaSize units).
pub const LETTER_W_THOU: u32 = 8500;
/// Platen / ticket height in 1/1000 inch. Live Brother MFC GSE uses **11700**
/// (not 11000). WIA item extent uses this; CreateScanJob ImageInformation uses
/// the smaller [`optical_page_pixels`] crop (Brother live behavior).
pub const LETTER_H_THOU: u32 = 11700;

/// JPEG COM payload prefix so multi-page order is machine-readable without decoding pixels.
pub const PAGE_MARKER_PREFIX: &[u8] = b"fakescan-page:";

/// JPEG COM payload prefix for manual-duplex front/back tagging.
pub const SIDE_MARKER_PREFIX: &[u8] = b"fakescan-side:";

/// QR / text payload scheme (no OCR required — decode with zbar / zxing / rqrr).
/// Example: `fakescan|v1|page=0|pat=page_id|side=front`
pub const QR_SCHEME: &str = "fakescan|v1";

/// Max RGB pixels before we drop to 150 dpi (CI / memory).
const MAX_PIXELS: u32 = 4_000_000;

/// JPEG quality when a QR is stamped (JPEG softens modules; keep this high).
const JPEG_QUALITY_WITH_QR: u8 = 95;

/// Reasonable baseline quality for WIA accept trials (no QR to protect).
const JPEG_QUALITY_PLAIN: u8 = 80;

/// Front vs back face for manual duplex / `--reverse` stacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageSide {
    Front,
    Back,
}

impl PageSide {
    pub fn as_str(self) -> &'static str {
        match self {
            PageSide::Front => "front",
            PageSide::Back => "back",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "front" | "f" => Ok(PageSide::Front),
            "back" | "b" => Ok(PageSide::Back),
            other => bail!("unknown side {other:?}; expected front or back"),
        }
    }
}

/// Per-request page context for diagnostic patterns ([`PatternId::Info`], [`PatternId::Anac`]).
///
/// Built by [`crate::state::World::retrieve_image_with_peer`] from the live job and the
/// WSD TCP peer address.  All fields degrade gracefully when absent.
#[derive(Debug, Clone, Default)]
pub struct PageContext {
    /// Active profile id (`brother-wsd`, …).
    pub profile_id: String,
    /// Device UUID as a hyphenated string.
    pub device_uuid: String,
    /// Active pattern id string (as returned by [`PatternId::as_str`]).
    pub pattern_id: String,
    pub dpi: u32,
    /// WSD `Format` token (`jfif`, …).
    pub format: String,
    /// Scan-job id assigned by CreateScanJob.
    pub job_id: i32,
    /// WSD RetrieveImage TCP peer address (`ip:port`).
    /// `None` or `"-"` means the address was not available.
    pub peer: Option<String>,
    /// UNIX seconds at render time.
    pub timestamp_secs: u64,
}

/// Named synthetic layout for RetrieveImage JPEGs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PatternId {
    /// Numbered bars + center block (default; multi-page order cue).
    #[default]
    PageId,
    /// Gray step wedges for brightness / contrast / threshold tests.
    Wedges,
    /// R/G/B + gray patches for color-mode checks.
    Patches,
    /// Large asymmetric F in the top-left for orientation.
    SlantF,
    /// L-marks in four corners + center cross for crop / region.
    Corners,
    /// Even indices blue/front, odd red/back (manual duplex / `--reverse`).
    DuplexPair,
    /// Large centered QR only (decode robustness); still COM-tagged.
    Qr,
    /// WIA accept trial: solid + gray wedges only (no banner, no QR).
    /// Pair with [`EncodeOpts::WIA_PLAIN`] (forced by [`encode_opts_for`]).
    WiaPlain,
    /// ~5° slanted edge (ISO 16067-1 style visual; no MTF math).
    SlantEdge,
    /// Text size ladder: the same phrase rendered at decreasing glyph scales.
    TextLadder,
    /// All-in-one composite: wedges + slant edge + colour patches + text ladder + corner marks.
    Testpage,
    /// Server diagnostics: profile, UUID, DPI, format, job id, client IP, time.
    Info,
    /// ANAC-style client IP readback (alias: `whoami`).
    Anac,
    /// CMYK bars + tint ladders (RGB-approximated; C / M / Y / K labelled).
    Cmyk,
    /// Nozzle-check style horizontal and vertical band groups (7 colour channels).
    Nozzle,
    /// Alignment / registration crosses + grid tick marks along all four edges.
    Align,
    /// Margin frames + scale rules (inch / quarter-inch tick cues).
    Grid,
    /// Resolution line ladder: coarse-to-fine horizontal and vertical line groups.
    Lines,
}

impl PatternId {
    pub const ALL: &'static [PatternId] = &[
        PatternId::PageId,
        PatternId::Wedges,
        PatternId::Patches,
        PatternId::SlantF,
        PatternId::Corners,
        PatternId::DuplexPair,
        PatternId::Qr,
        PatternId::WiaPlain,
        PatternId::SlantEdge,
        PatternId::TextLadder,
        PatternId::Testpage,
        PatternId::Info,
        PatternId::Anac,
        PatternId::Cmyk,
        PatternId::Nozzle,
        PatternId::Align,
        PatternId::Grid,
        PatternId::Lines,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            PatternId::PageId => "page_id",
            PatternId::Wedges => "wedges",
            PatternId::Patches => "patches",
            PatternId::SlantF => "slant_f",
            PatternId::Corners => "corners",
            PatternId::DuplexPair => "duplex_pair",
            PatternId::Qr => "qr",
            PatternId::WiaPlain => "wia_plain",
            PatternId::SlantEdge => "slant_edge",
            PatternId::TextLadder => "textladder",
            PatternId::Testpage => "testpage",
            PatternId::Info => "info",
            PatternId::Anac => "anac",
            PatternId::Cmyk => "cmyk",
            PatternId::Nozzle => "nozzle",
            PatternId::Align => "align",
            PatternId::Grid => "grid",
            PatternId::Lines => "lines",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "page_id" | "page-id" | "default" => Ok(PatternId::PageId),
            "wedges" => Ok(PatternId::Wedges),
            "patches" => Ok(PatternId::Patches),
            "slant_f" | "slant-f" | "slantf" => Ok(PatternId::SlantF),
            "corners" => Ok(PatternId::Corners),
            "duplex_pair" | "duplex-pair" | "duplex" => Ok(PatternId::DuplexPair),
            "qr" | "qrcode" => Ok(PatternId::Qr),
            "wia_plain" | "wia-plain" | "plain" => Ok(PatternId::WiaPlain),
            "slant_edge" | "slant-edge" | "slantedge" => Ok(PatternId::SlantEdge),
            "textladder" | "text_ladder" | "text-ladder" => Ok(PatternId::TextLadder),
            "testpage" | "test_page" | "test-page" => Ok(PatternId::Testpage),
            "info" => Ok(PatternId::Info),
            "anac" | "whoami" => Ok(PatternId::Anac),
            "cmyk" => Ok(PatternId::Cmyk),
            "nozzle" => Ok(PatternId::Nozzle),
            "align" => Ok(PatternId::Align),
            "grid" => Ok(PatternId::Grid),
            "lines" => Ok(PatternId::Lines),
            other => bail!(
                "unknown pattern {other:?}; expected one of: {}",
                PatternId::ALL
                    .iter()
                    .map(|p| p.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }

    /// True when this pattern is the WIA minimal JPEG layout.
    pub fn is_wia_plain(self) -> bool {
        matches!(self, PatternId::WiaPlain)
    }
}

/// Machine-readable page identity carried in the QR (and documented for host decoders).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QrPayload {
    pub page: u32,
    pub pattern: PatternId,
    pub side: Option<PageSide>,
}

/// Build the QR text payload for a page.
pub fn qr_payload(pattern: PatternId, page_index: u32, side: Option<PageSide>) -> String {
    let side = side.map(|s| s.as_str()).unwrap_or("none");
    format!(
        "{QR_SCHEME}|page={page_index}|pat={}|side={side}",
        pattern.as_str()
    )
}

/// Parse a QR / text payload produced by [`qr_payload`].
pub fn parse_qr_payload(text: &str) -> Option<QrPayload> {
    let text = text.trim();
    if !text.starts_with(QR_SCHEME) {
        return None;
    }
    let mut page = None;
    let mut pattern = None;
    let mut side = None;
    for part in text.split('|').skip(2) {
        if let Some(v) = part.strip_prefix("page=") {
            page = v.parse().ok();
        } else if let Some(v) = part.strip_prefix("pat=") {
            pattern = PatternId::parse(v).ok();
        } else if let Some(v) = part.strip_prefix("side=") {
            side = match v {
                "none" | "" => None,
                other => PageSide::parse(other).ok(),
            };
        }
    }
    Some(QrPayload {
        page: page?,
        pattern: pattern?,
        side,
    })
}

pub fn page_pixels(dpi: u32) -> (u32, u32) {
    let w = (LETTER_W_THOU * dpi) / 1000;
    let h = (LETTER_H_THOU * dpi) / 1000;
    (w.max(1), h.max(1))
}

/// Brother MFC-9330CDW live deliverable width in 1/1000 inch (Transfer @200 → 1648 px).
/// Platen / WIA item extent stays [`LETTER_W_THOU`]; ImageInformation + SOF use this crop.
const OPTICAL_W_THOU: u32 = 8240;
/// Brother live deliverable height in 1/1000 inch (Transfer @200 → 2289 px).
const OPTICAL_H_THOU: u32 = 11445;

/// Pixel size Brother WSD actually returns (not full letter).
///
/// Lab: WIA extent stays letter (`page_pixels`), but CreateScanJob ImageInformation
/// and RetrieveImage SOF are this crop. Transfer @200 accepted at **1648×2289**.
/// A fake full-letter **1700×2340** (same JFIF layout) gets CancelJob ~30s after RetrieveImage.
/// Exact triples match live Brother samples @100 / @200 / @300.
pub fn optical_page_pixels(dpi: u32) -> (u32, u32) {
    match dpi {
        100 => (816, 1144),
        200 => (1648, 2289),
        300 => (2480, 3440),
        d => {
            let w = (OPTICAL_W_THOU * d) / 1000;
            let h = (OPTICAL_H_THOU * d) / 1000;
            (w.max(1), h.max(1))
        }
    }
}

/// Pixel size for Validate/Create `ImageInformation` **and** RetrieveImage encode.
///
/// Uses [`optical_page_pixels`] (Brother crop), then caps to 150 dpi optical when
/// the product exceeds [`MAX_PIXELS`]. SOF must match declared PixelsPerLine /
/// NumberOfLines.
pub fn job_page_pixels(dpi: u32) -> (u32, u32) {
    capped_page_pixels(dpi)
}

fn capped_page_pixels(dpi: u32) -> (u32, u32) {
    let (w, h) = optical_page_pixels(dpi);
    if w.saturating_mul(h) > MAX_PIXELS {
        optical_page_pixels(150)
    } else {
        (w, h)
    }
}

/// Encode a JPEG with the default `page_id` pattern and COM `fakescan-page:N`.
pub fn jpeg_page(dpi: u32, page_index: u32) -> Result<Vec<u8>> {
    jpeg_pattern(PatternId::PageId, dpi, page_index)
}

/// Encode options for WSD Format payloads (JPEG COM, QR stamp, quality, …).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncodeOpts {
    /// Inject `fakescan-page:` / `fakescan-side:` JPEG COM markers (default true).
    /// Airscan / multi-page order tests rely on COM; WIA experiments may disable.
    pub inject_jpeg_com: bool,
    /// Stamp a corner / focus QR into the RGB buffer before encode.
    pub stamp_qr: bool,
    /// Stamp the tester banner across the top margin.
    pub stamp_banner: bool,
    /// When set, overrides effects / default JPEG quality (1–100).
    pub jpeg_quality: Option<u8>,
}

impl Default for EncodeOpts {
    fn default() -> Self {
        Self {
            inject_jpeg_com: true,
            stamp_qr: true,
            stamp_banner: true,
            jpeg_quality: None,
        }
    }
}

impl EncodeOpts {
    pub const WITH_COM: Self = Self {
        inject_jpeg_com: true,
        stamp_qr: true,
        stamp_banner: true,
        jpeg_quality: None,
    };
    pub const NO_COM: Self = Self {
        inject_jpeg_com: false,
        stamp_qr: true,
        stamp_banner: true,
        jpeg_quality: None,
    };
    /// Baseline JFIF for WIA RetrieveImage accept: no COM, no QR, no banner, Q≈80.
    pub const WIA_PLAIN: Self = Self {
        inject_jpeg_com: false,
        stamp_qr: false,
        stamp_banner: false,
        jpeg_quality: Some(JPEG_QUALITY_PLAIN),
    };
}

/// Resolve encode knobs from the active pattern and `POST /v1/jpeg-com`.
///
/// [`PatternId::WiaPlain`] always forces [`EncodeOpts::WIA_PLAIN`] (COM-off is
/// not enough alone — QR/banner/high-Q remain on other patterns).
pub fn encode_opts_for(pattern: PatternId, jpeg_com: bool) -> EncodeOpts {
    if pattern.is_wia_plain() {
        EncodeOpts::WIA_PLAIN
    } else if jpeg_com {
        EncodeOpts::WITH_COM
    } else {
        EncodeOpts::NO_COM
    }
}

/// Encode a JPEG for the given pattern id (always injects `fakescan-page:N`
/// and a corner QR with [`qr_payload`]).
///
/// `duplex_pair` also injects `fakescan-side:front|back` from the page index
/// (even → front/blue, odd → back/red).
pub fn jpeg_pattern(pattern: PatternId, dpi: u32, page_index: u32) -> Result<Vec<u8>> {
    jpeg_pattern_with_side(pattern, dpi, page_index, None)
}

/// Like [`jpeg_pattern`], but an explicit `side` overrides duplex auto-tagging
/// (and tags non-duplex patterns for front/back stack tests).
pub fn jpeg_pattern_with_side(
    pattern: PatternId,
    dpi: u32,
    page_index: u32,
    side: Option<PageSide>,
) -> Result<Vec<u8>> {
    encode_pattern(
        "jfif",
        pattern,
        dpi,
        page_index,
        side,
        None,
        crate::effects::ColorMode::Rgb24,
        EncodeOpts::WITH_COM,
    )
}

/// True when one RetrieveImage should return every remaining job page in a single file.
pub fn is_multipage_format(wsd_format: &str) -> bool {
    matches!(
        wsd_format,
        "tiff-multi-uncompressed"
            | "tiff-multi-g4"
            | "tiff-multi-g3mh"
            | "tiff-multi-jpeg-tn2"
            | "pdf-a"
            | "pdf"
            | "xps"
    )
}

/// How many feeder pages one RetrieveImage consumes for this Format.
pub fn pages_per_retrieve(wsd_format: &str, pages_left: u32) -> u32 {
    if pages_left == 0 {
        0
    } else if is_multipage_format(wsd_format) {
        pages_left
    } else {
        1
    }
}

/// Render pattern (+ optional effects / color) and encode as the WSD Format token.
#[allow(clippy::too_many_arguments)] // format, pattern, geometry, and encode options stay positional
pub fn encode_pattern(
    wsd_format: &str,
    pattern: PatternId,
    dpi: u32,
    page_index: u32,
    side: Option<PageSide>,
    effects: Option<&crate::effects::ImageEffects>,
    color: crate::effects::ColorMode,
    opts: EncodeOpts,
) -> Result<Vec<u8>> {
    encode_pattern_pages(
        wsd_format,
        pattern,
        dpi,
        &[page_index],
        side,
        effects,
        color,
        opts,
    )
}

/// Encode one or more sheet indices into a (possibly multi-page) Format payload.
///
/// For diagnostic patterns ([`PatternId::Info`], [`PatternId::Anac`]) prefer
/// [`encode_pattern_pages_ctx`] so server / client context appears on the page.
#[allow(clippy::too_many_arguments)]
pub fn encode_pattern_pages(
    wsd_format: &str,
    pattern: PatternId,
    dpi: u32,
    sheet_indices: &[u32],
    side: Option<PageSide>,
    effects: Option<&crate::effects::ImageEffects>,
    color: crate::effects::ColorMode,
    opts: EncodeOpts,
) -> Result<Vec<u8>> {
    encode_pattern_pages_ctx(
        wsd_format,
        pattern,
        dpi,
        sheet_indices,
        side,
        effects,
        color,
        opts,
        None,
    )
}

/// Like [`encode_pattern_pages`] but threads a [`PageContext`] into diagnostic patterns.
#[allow(clippy::too_many_arguments)]
pub fn encode_pattern_pages_ctx(
    wsd_format: &str,
    pattern: PatternId,
    dpi: u32,
    sheet_indices: &[u32],
    side: Option<PageSide>,
    effects: Option<&crate::effects::ImageEffects>,
    color: crate::effects::ColorMode,
    opts: EncodeOpts,
    ctx: Option<&PageContext>,
) -> Result<Vec<u8>> {
    if sheet_indices.is_empty() {
        bail!("encode_pattern_pages: no sheets");
    }
    let mut pages = Vec::with_capacity(sheet_indices.len());
    let jpeg_q = opts
        .jpeg_quality
        .filter(|&q| q > 0)
        .or_else(|| effects.map(|fx| fx.effective_jpeg_quality()))
        .unwrap_or(JPEG_QUALITY_WITH_QR);
    for &sheet in sheet_indices {
        let (mut img, effective_side) =
            render_pattern_rgb_ctx(pattern, dpi, sheet, side, opts, ctx)?;
        if let Some(fx) = effects {
            crate::effects::apply_pipeline(&mut img, fx, sheet, color);
        } else if color != crate::effects::ColorMode::Rgb24 {
            crate::effects::apply_pipeline(
                &mut img,
                &crate::effects::ImageEffects::default(),
                sheet,
                color,
            );
        }
        pages.push((img, sheet, effective_side));
    }
    // APP0 density must match the pixels we actually rendered (cap may drop dpi).
    let effective_dpi = {
        let (cw, _) = capped_page_pixels(dpi);
        let (ow, _) = optical_page_pixels(dpi);
        if cw != ow {
            150
        } else {
            dpi
        }
    };
    encode_images(wsd_format, &pages, jpeg_q, opts, effective_dpi)
}

/// Back-compat alias: always JFIF with COM markers.
pub fn jpeg_pattern_processed(
    pattern: PatternId,
    dpi: u32,
    page_index: u32,
    side: Option<PageSide>,
    effects: Option<&crate::effects::ImageEffects>,
    color: crate::effects::ColorMode,
) -> Result<Vec<u8>> {
    encode_pattern(
        "jfif",
        pattern,
        dpi,
        page_index,
        side,
        effects,
        color,
        EncodeOpts::WITH_COM,
    )
}

/// Draw pattern, banner, and QR into an RGB buffer (no JPEG yet).
///
/// Uses default encode decorations (banner + QR on). Prefer
/// [`render_pattern_rgb_opts`] when applying [`EncodeOpts::WIA_PLAIN`].
pub fn render_pattern_rgb(
    pattern: PatternId,
    dpi: u32,
    page_index: u32,
    side: Option<PageSide>,
) -> Result<(RgbImage, Option<PageSide>)> {
    render_pattern_rgb_opts(pattern, dpi, page_index, side, EncodeOpts::default())
}

/// Like [`render_pattern_rgb`], honoring QR / banner encode knobs.
///
/// For diagnostic patterns [`PatternId::Info`] and [`PatternId::Anac`] the caller should
/// prefer [`render_pattern_rgb_ctx`] so that server / client context is embedded.
pub fn render_pattern_rgb_opts(
    pattern: PatternId,
    dpi: u32,
    page_index: u32,
    side: Option<PageSide>,
    opts: EncodeOpts,
) -> Result<(RgbImage, Option<PageSide>)> {
    render_pattern_rgb_ctx(pattern, dpi, page_index, side, opts, None)
}

/// Like [`render_pattern_rgb_opts`] but threads a [`PageContext`] into diagnostic patterns.
///
/// `ctx` is used by [`PatternId::Info`] and [`PatternId::Anac`] to embed server / client
/// diagnostics on the page and to append `client=` to the QR payload when the peer is known.
pub fn render_pattern_rgb_ctx(
    pattern: PatternId,
    dpi: u32,
    page_index: u32,
    side: Option<PageSide>,
    opts: EncodeOpts,
    ctx: Option<&PageContext>,
) -> Result<(RgbImage, Option<PageSide>)> {
    let (w, h) = capped_page_pixels(dpi);
    let effective_side = side.or_else(|| match pattern {
        PatternId::DuplexPair => Some(if page_index.is_multiple_of(2) {
            PageSide::Front
        } else {
            PageSide::Back
        }),
        _ => None,
    });
    let mut img = match pattern {
        PatternId::PageId => draw_page_id(w, h, page_index),
        PatternId::Wedges | PatternId::WiaPlain => draw_wedges(w, h, page_index),
        PatternId::Patches => draw_patches(w, h, page_index),
        PatternId::SlantF => draw_slant_f(w, h, page_index),
        PatternId::Corners => draw_corners(w, h, page_index),
        PatternId::DuplexPair => {
            draw_duplex_pair(w, h, page_index, effective_side.unwrap_or(PageSide::Front))
        }
        PatternId::Qr => draw_qr_focus(w, h, page_index),
        PatternId::SlantEdge => crate::image_iso::draw_slant_edge(w, h, page_index),
        PatternId::TextLadder => crate::image_iso::draw_text_ladder(w, h, page_index),
        PatternId::Testpage => crate::image_iso::draw_testpage(w, h, page_index),
        PatternId::Info => draw_info(w, h, page_index, ctx),
        PatternId::Anac => draw_anac(w, h, page_index, ctx),
        PatternId::Cmyk => crate::image_charts::draw_cmyk(w, h, page_index),
        PatternId::Nozzle => crate::image_charts::draw_nozzle(w, h, page_index),
        PatternId::Align => crate::image_charts::draw_align(w, h, page_index),
        PatternId::Grid => crate::image_charts::draw_grid(w, h, page_index),
        PatternId::Lines => crate::image_charts::draw_lines(w, h, page_index),
    };
    if opts.stamp_banner {
        let banner = pattern_banner(pattern, page_index, effective_side);
        stamp_banner(&mut img, &banner);
    }

    if opts.stamp_qr {
        // Append `|client=<ip>` to the QR when a peer is available (Info/Anac or any pattern).
        let base = qr_payload(pattern, page_index, effective_side);
        let payload = match ctx
            .and_then(|c| c.peer.as_deref())
            .filter(|p| !p.is_empty() && *p != "-")
        {
            Some(peer) => format!("{base}|client={peer}"),
            None => base,
        };
        if pattern != PatternId::Qr {
            stamp_qr(
                &mut img,
                &payload,
                QrPlacement::BottomRight,
                qr_corner_size(w, h),
            )?;
        } else {
            stamp_qr(&mut img, &payload, QrPlacement::Center, qr_focus_size(w, h))?;
        }
    }
    Ok((img, effective_side))
}

/// Banner drawn at the top of every page (tester-facing, no OCR needed to spot).
pub fn pattern_banner(pattern: PatternId, page_index: u32, side: Option<PageSide>) -> String {
    let step = crate::scenarios::PROGRESSION
        .iter()
        .find(|s| s.pattern == pattern)
        .map(|s| s.title)
        .unwrap_or(pattern.as_str());
    let side = side.map(|s| s.as_str()).unwrap_or("-");
    format!("{step}  P{page_index}  {side}")
}

fn stamp_banner(img: &mut RgbImage, text: &str) {
    let (w, h) = img.dimensions();
    if w < 80 || h < 40 {
        return;
    }
    let scale = (w.min(h) / 180).clamp(2, 6);
    let pad = scale * 2;
    let glyph_h = 7 * scale;
    let band = glyph_h + pad * 2;
    fill_rect(img, 0, 0, w, band.min(h), Rgb([20, 24, 32]));
    let mut x = pad;
    let y = pad;
    let ink = Rgb([240, 240, 235]);
    for ch in text.chars().take(48) {
        let advance = draw_glyph(img, ch, x, y, scale, ink);
        x = x.saturating_add(advance + scale);
        if x + 5 * scale >= w {
            break;
        }
    }
}

/// Tiny 5×7 glyphs (uppercase / digits / a few punctuation). Unknown → blank.
/// Returns the advance width (5 × scale); caller adds an inter-character gap.
pub(crate) fn draw_glyph(
    img: &mut RgbImage,
    ch: char,
    x0: u32,
    y0: u32,
    scale: u32,
    color: Rgb<u8>,
) -> u32 {
    let rows = glyph_rows(ch);
    for (row, bits) in rows.iter().enumerate() {
        for col in 0..5u32 {
            if bits & (1 << (4 - col)) != 0 {
                let px = x0 + col * scale;
                let py = y0 + (row as u32) * scale;
                fill_rect(img, px, py, px + scale, py + scale, color);
            }
        }
    }
    5 * scale
}

fn glyph_rows(ch: char) -> [u8; 7] {
    // 5-bit rows, MSB = left pixel.
    match ch.to_ascii_uppercase() {
        ' ' => [0, 0, 0, 0, 0, 0, 0],
        '-' => [0, 0, 0, 0b11111, 0, 0, 0],
        '.' => [0, 0, 0, 0, 0, 0b00100, 0b00100],
        ':' => [0, 0b00100, 0b00100, 0, 0b00100, 0b00100, 0],
        '/' => [0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0, 0],
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110,
        ],
        '6' => [
            0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110,
        ],
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'B' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
        'C' => [
            0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110,
        ],
        'D' => [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'G' => [
            0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110,
        ],
        'H' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'I' => [
            0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10001, 0b10001, 0b10001, 0b10001,
        ],
        'N' => [
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'Q' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'V' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ],
        'W' => [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010,
        ],
        'X' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ],
        'Y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'Z' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ],
        _ => [0, 0, 0, 0b00100, 0, 0, 0],
    }
}

/// Encode RGB buffer as a WSD Scan Format payload.
pub fn encode_image(
    wsd_format: &str,
    img: RgbImage,
    page_index: u32,
    side: Option<PageSide>,
) -> Result<Vec<u8>> {
    let dpi = jfif_dpi_from_img(&img);
    encode_images(
        wsd_format,
        &[(img, page_index, side)],
        JPEG_QUALITY_WITH_QR,
        EncodeOpts::WITH_COM,
        dpi,
    )
}

/// Encode one or more RGB pages. Multi-page Formats pack every page into one blob.
fn encode_images(
    wsd_format: &str,
    pages: &[(RgbImage, u32, Option<PageSide>)],
    jpeg_quality: u8,
    opts: EncodeOpts,
    dpi: u32,
) -> Result<Vec<u8>> {
    if pages.is_empty() {
        bail!("no pages to encode");
    }
    match wsd_format {
        "jfif" | "jpeg" | "jpg" => {
            let (img, idx, side) = take_single(pages, wsd_format)?;
            encode_jpeg(img.clone(), idx, side, jpeg_quality, opts, dpi)
        }
        "exif" => {
            let (img, idx, side) = take_single(pages, wsd_format)?;
            encode_exif_jpeg(img.clone(), idx, side, jpeg_quality, opts, dpi)
        }
        "dib" | "bmp" => {
            let (img, _, _) = take_single(pages, wsd_format)?;
            encode_bmp(img.clone())
        }
        "png" => {
            let (img, _, _) = take_single(pages, wsd_format)?;
            encode_png(img.clone())
        }
        "tiff-single-uncompressed" | "tiff" | "tif" => {
            let (img, _, _) = take_single(pages, wsd_format)?;
            encode_tiff(img.clone())
        }
        "tiff-single-g4" => {
            let (img, _, _) = take_single(pages, wsd_format)?;
            encode_tiff_g4(&[img])
        }
        "tiff-single-g3mh" => {
            let (img, _, _) = take_single(pages, wsd_format)?;
            encode_tiff_g3mh(&[img])
        }
        "tiff-single-jpeg-tn2" => encode_tiff_jpeg_tn2(pages, jpeg_quality, opts, dpi),
        "tiff-multi-uncompressed" => {
            let imgs: Vec<&RgbImage> = pages.iter().map(|(i, _, _)| i).collect();
            encode_tiff_multi_uncompressed(&imgs)
        }
        "tiff-multi-g4" => {
            let imgs: Vec<&RgbImage> = pages.iter().map(|(i, _, _)| i).collect();
            encode_tiff_g4(&imgs)
        }
        "tiff-multi-g3mh" => {
            let imgs: Vec<&RgbImage> = pages.iter().map(|(i, _, _)| i).collect();
            encode_tiff_g3mh(&imgs)
        }
        "tiff-multi-jpeg-tn2" => encode_tiff_jpeg_tn2(pages, jpeg_quality, opts, dpi),
        "jpeg2k" => {
            let (img, _, _) = take_single(pages, wsd_format)?;
            encode_jpeg2k(img)
        }
        "pdf-a" | "pdf" => encode_pdf_pages(pages, jpeg_quality, opts, dpi),
        "xps" => encode_xps_pages(pages, jpeg_quality, opts, dpi),
        other => bail!("encode not implemented for format {other:?}"),
    }
}

fn take_single<'a>(
    pages: &'a [(RgbImage, u32, Option<PageSide>)],
    format: &str,
) -> Result<(&'a RgbImage, u32, Option<PageSide>)> {
    if pages.len() != 1 {
        bail!("{format} is single-page; got {} sheets", pages.len());
    }
    Ok((&pages[0].0, pages[0].1, pages[0].2))
}

/// Infer JFIF density from letter-sized RGB dimensions (fallback for encode_image).
fn jfif_dpi_from_img(img: &RgbImage) -> u32 {
    let inferred = (img.width().saturating_mul(1000) + LETTER_W_THOU / 2) / LETTER_W_THOU;
    inferred.clamp(72, 1200)
}

/// MCU columns for 4:2:0 (16×16 MCU) — Brother uses DRI = one MCU row.
fn restart_interval_mcu_row(width: u32) -> u16 {
    let cols = width.saturating_add(15) / 16;
    cols.clamp(1, u16::MAX as u32) as u16
}

/// Encode JFIF toward Brother WSD / wsdscdrv shape: baseline SOF0, 4:2:0,
/// APP0 units=dpi, DRI = one MCU row. (`image` crate emits 4:4:4 + aspect 1×1.)
fn encode_jpeg(
    img: RgbImage,
    page_index: u32,
    side: Option<PageSide>,
    quality: u8,
    opts: EncodeOpts,
    dpi: u32,
) -> Result<Vec<u8>> {
    let q = if quality == 0 {
        JPEG_QUALITY_WITH_QR
    } else {
        quality.clamp(1, 100)
    };
    let w = img.width();
    let h = img.height();
    if w == 0 || h == 0 {
        bail!("empty image for jpeg");
    }
    if w > u16::MAX as u32 || h > u16::MAX as u32 {
        bail!("image {w}x{h} exceeds JPEG u16 dimensions");
    }
    let dens = dpi.clamp(1, u16::MAX as u32) as u16;
    let mut out = Vec::new();
    {
        let mut encoder = jpeg_encoder::Encoder::new(&mut out, q);
        encoder.set_density(jpeg_encoder::PixelDensity::dpi(dens));
        // Force 4:2:0 even at Q≥90 (crate default would be 4:4:4).
        encoder.set_sampling_factor(jpeg_encoder::SamplingFactor::F_2_2);
        encoder.set_restart_interval(restart_interval_mcu_row(w));
        encoder
            .encode(
                img.as_raw(),
                w as u16,
                h as u16,
                jpeg_encoder::ColorType::Rgb,
            )
            .map_err(|e| anyhow::anyhow!("jpeg encode: {e}"))?;
    }
    if out.is_empty() {
        bail!("empty jpeg");
    }
    // jpeg-encoder uses component ids 0/1/2; Brother / JFIF scanners use 1/2/3.
    remap_jpeg_component_ids_1_based(&mut out)?;
    // jpeg-encoder order is APP0→SOF→DQT×N→DHT×N→DRI→SOS; Brother / wsdscdrv
    // samples are APP0→DQT→DRI→SOF→DHT→SOS with tables packed into one segment each.
    out = reshape_jpeg_brother_layout(&out)?;
    if opts.inject_jpeg_com {
        inject_page_com(&mut out, page_index)?;
        if let Some(side) = side {
            inject_side_com(&mut out, side)?;
        }
    }
    Ok(out)
}

/// Rebuild header segments toward Brother WSD / WIA JPEG layout.
///
/// Target order: `SOI APP0 [APP*/COM…] DQT DRI SOF0 DHT SOS … EOI`, with all
/// quantization tables in one DQT and all Huffman tables in one DHT.
fn reshape_jpeg_brother_layout(jpeg: &[u8]) -> Result<Vec<u8>> {
    if jpeg.len() < 4 || jpeg[0] != 0xFF || jpeg[1] != 0xD8 {
        bail!("not a JPEG (missing SOI)");
    }
    let mut app0: Option<Vec<u8>> = None;
    let mut misc: Vec<Vec<u8>> = Vec::new(); // APP1.. / COM / other APPn
    let mut dqt_payload = Vec::new();
    let mut dht_payload = Vec::new();
    let mut dri: Option<Vec<u8>> = None;
    let mut sof: Option<Vec<u8>> = None;
    let mut sos: Option<Vec<u8>> = None;
    let mut i = 2usize;
    while i + 1 < jpeg.len() {
        if jpeg[i] != 0xFF {
            bail!("jpeg desync before SOS at offset {i}");
        }
        let mut j = i + 1;
        while j < jpeg.len() && jpeg[j] == 0xFF {
            j += 1;
        }
        if j >= jpeg.len() {
            bail!("truncated JPEG marker");
        }
        let marker = jpeg[j];
        i = j + 1;
        if marker == 0xD9 {
            bail!("EOI before SOS");
        }
        if (0xD0..=0xD7).contains(&marker) || marker == 0x01 {
            continue;
        }
        if i + 2 > jpeg.len() {
            bail!("truncated JPEG segment length");
        }
        let len = u16::from_be_bytes([jpeg[i], jpeg[i + 1]]) as usize;
        if len < 2 || i + len > jpeg.len() {
            bail!("bad JPEG segment length {len} at marker {marker:#04x}");
        }
        let segment = jpeg[j - 1..i + len].to_vec(); // FF marker + len + payload
        let payload = &jpeg[i + 2..i + len];
        i += len;
        match marker {
            0xE0 => {
                if app0.is_none() {
                    app0 = Some(segment);
                } else {
                    misc.push(segment);
                }
            }
            0xDB => dqt_payload.extend_from_slice(payload),
            0xC4 => dht_payload.extend_from_slice(payload),
            0xDD => dri = Some(segment),
            0xC0 => sof = Some(segment),
            0xDA => {
                sos = Some(segment);
                break;
            }
            m if (0xC0..=0xCF).contains(&m) && m != 0xC4 && m != 0xC8 && m != 0xCC => {
                // Other SOF — keep position as frame header.
                sof = Some(segment);
            }
            _ => misc.push(segment),
        }
    }
    let app0 = app0.ok_or_else(|| anyhow::anyhow!("JPEG missing APP0/JFIF"))?;
    let sof = sof.ok_or_else(|| anyhow::anyhow!("JPEG missing SOF"))?;
    let sos = sos.ok_or_else(|| anyhow::anyhow!("JPEG missing SOS"))?;
    if dqt_payload.is_empty() || dht_payload.is_empty() {
        bail!("JPEG missing DQT or DHT");
    }
    let dqt = pack_jpeg_segment(0xDB, &dqt_payload)?;
    let dht = pack_jpeg_segment(0xC4, &dht_payload)?;
    let mut out = Vec::with_capacity(jpeg.len() + 32);
    out.extend_from_slice(&[0xFF, 0xD8]);
    out.extend_from_slice(&app0);
    for seg in misc {
        out.extend_from_slice(&seg);
    }
    out.extend_from_slice(&dqt);
    if let Some(dri) = dri {
        out.extend_from_slice(&dri);
    }
    out.extend_from_slice(&sof);
    out.extend_from_slice(&dht);
    out.extend_from_slice(&sos);
    out.extend_from_slice(&jpeg[i..]); // entropy + EOI
    Ok(out)
}

fn pack_jpeg_segment(marker: u8, payload: &[u8]) -> Result<Vec<u8>> {
    let len = 2 + payload.len();
    if len > u16::MAX as usize {
        bail!("JPEG segment {marker:#04x} too long ({len})");
    }
    let mut out = Vec::with_capacity(2 + len);
    out.push(0xFF);
    out.push(marker);
    out.extend_from_slice(&(len as u16).to_be_bytes());
    out.extend_from_slice(payload);
    Ok(out)
}

/// Rewrite SOF/SOS component selectors from 0-based (jpeg-encoder) to 1-based (Brother).
fn remap_jpeg_component_ids_1_based(jpeg: &mut [u8]) -> Result<()> {
    let mut i = 0;
    while i + 4 < jpeg.len() {
        if jpeg[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = jpeg[i + 1];
        if marker == 0xD8 {
            i += 2;
            continue;
        }
        if marker == 0xD9 {
            break;
        }
        if (0xD0..=0xD7).contains(&marker) {
            i += 2;
            continue;
        }
        if i + 4 > jpeg.len() {
            break;
        }
        let len = u16::from_be_bytes([jpeg[i + 2], jpeg[i + 3]]) as usize;
        if len < 2 || i + 2 + len > jpeg.len() {
            break;
        }
        let payload = &mut jpeg[i + 4..i + 2 + len];
        if (0xC0..=0xCF).contains(&marker) && marker != 0xC4 && marker != 0xC8 && marker != 0xCC {
            // SOF: [P Y Y X X Nf] then Nf × [id HV Tq]
            if payload.len() >= 6 {
                let nf = payload[5] as usize;
                let mut off = 6;
                for _ in 0..nf {
                    if off + 3 > payload.len() {
                        break;
                    }
                    if payload[off] < 3 {
                        payload[off] = payload[off].saturating_add(1);
                    }
                    off += 3;
                }
            }
        } else if marker == 0xDA {
            // SOS: [Ns] then Ns × [Cs TdTa], then 3 spectral bytes
            if !payload.is_empty() {
                let ns = payload[0] as usize;
                let mut off = 1;
                for _ in 0..ns {
                    if off + 2 > payload.len() {
                        break;
                    }
                    if payload[off] < 3 {
                        payload[off] = payload[off].saturating_add(1);
                    }
                    off += 2;
                }
            }
        }
        if marker == 0xDA {
            break; // entropy follows
        }
        i += 2 + len;
    }
    Ok(())
}

fn encode_bmp(img: RgbImage) -> Result<Vec<u8>> {
    use image::ImageEncoder;
    let mut out = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut out);
    let encoder = image::codecs::bmp::BmpEncoder::new(&mut cursor);
    encoder
        .write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|e| anyhow::anyhow!("bmp encode: {e}"))?;
    if out.is_empty() {
        bail!("empty bmp");
    }
    Ok(out)
}

fn encode_png(img: RgbImage) -> Result<Vec<u8>> {
    use image::ImageEncoder;
    let mut out = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut out);
    let encoder = image::codecs::png::PngEncoder::new(&mut cursor);
    encoder
        .write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|e| anyhow::anyhow!("png encode: {e}"))?;
    if out.is_empty() {
        bail!("empty png");
    }
    Ok(out)
}

fn encode_tiff(img: RgbImage) -> Result<Vec<u8>> {
    encode_tiff_multi_uncompressed(&[&img])
}

fn encode_tiff_multi_uncompressed(imgs: &[&RgbImage]) -> Result<Vec<u8>> {
    use std::io::Cursor;
    let mut out = Vec::new();
    {
        let mut cursor = Cursor::new(&mut out);
        let mut enc = tiff::encoder::TiffEncoder::new(&mut cursor)
            .map_err(|e| anyhow::anyhow!("tiff: {e}"))?;
        for img in imgs {
            enc.write_image::<tiff::encoder::colortype::RGB8>(
                img.width(),
                img.height(),
                img.as_raw(),
            )
            .map_err(|e| anyhow::anyhow!("tiff page: {e}"))?;
        }
    }
    if out.is_empty() {
        bail!("empty tiff");
    }
    Ok(out)
}

/// CCITT Group 4 (WSD `tiff-*-g4`). Threshold RGB → bilevel, wrap with [`fax::tiff`].
fn encode_tiff_g4(imgs: &[&RgbImage]) -> Result<Vec<u8>> {
    if imgs.is_empty() {
        bail!("empty g4 tiff");
    }
    if imgs.len() == 1 {
        let g4 = encode_g4_strip(imgs[0])?;
        return Ok(fax::tiff::wrap(&g4, imgs[0].width(), imgs[0].height()));
    }
    // Multi-IFD G4: chain IFDs like fax::tiff::wrap but with next pointers.
    wrap_g4_multipage(
        &imgs
            .iter()
            .map(|img| {
                let data = encode_g4_strip(img)?;
                Ok((data, img.width(), img.height()))
            })
            .collect::<Result<Vec<_>>>()?,
    )
}

fn encode_g4_strip(img: &RgbImage) -> Result<Vec<u8>> {
    let w = img.width();
    let h = img.height();
    if w == 0 || h == 0 {
        bail!("empty image for g4");
    }
    if w > u16::MAX as u32 {
        bail!("image width {w} exceeds G4 u16 limit");
    }
    let width = w as u16;
    let writer = fax::VecWriter::with_capacity(((w * h) / 8 + 64) as usize);
    let mut enc = fax::encoder::Encoder::new(writer);
    for y in 0..h {
        let colors = (0..w).map(|x| {
            let p = img.get_pixel(x, y).0;
            let luma = (u16::from(p[0]) + u16::from(p[1]) + u16::from(p[2])) / 3;
            if luma < 128 {
                fax::Color::Black
            } else {
                fax::Color::White
            }
        });
        enc.encode_line(colors, width)
            .map_err(|_| anyhow::anyhow!("g4 encode line {y}"))?;
    }
    let writer = enc.finish().map_err(|_| anyhow::anyhow!("g4 finish"))?;
    Ok(writer.finish())
}

/// CCITT T.4 Group 3 1-D Modified Huffman (WSD `tiff-single-g3mh`).
fn encode_tiff_g3mh(imgs: &[&RgbImage]) -> Result<Vec<u8>> {
    if imgs.is_empty() {
        bail!("empty g3mh tiff");
    }
    let pages: Vec<(Vec<u8>, u32, u32)> = imgs
        .iter()
        .map(|img| {
            let data = encode_g3mh_strip(img)?;
            Ok((data, img.width(), img.height()))
        })
        .collect::<Result<_>>()?;
    wrap_bilevel_tiff_multipage(
        &pages,
        /* compression */ 3,
        /* t4_options */ Some(0),
    )
}

fn encode_g3mh_strip(img: &RgbImage) -> Result<Vec<u8>> {
    use fax::maps::{black, white, EOL};
    use fax::{BitWriter, Color, VecWriter};

    let w = img.width();
    let h = img.height();
    if w == 0 || h == 0 {
        bail!("empty image for g3mh");
    }
    if w > u16::MAX as u32 {
        bail!("image width {w} exceeds G3 u16 limit");
    }
    let width = w as u16;
    let mut writer = VecWriter::with_capacity(((w * h) / 8 + 64) as usize);

    let write_run = |writer: &mut VecWriter, color: Color, mut n: u16| -> Result<()> {
        let encode = |n: u16| match color {
            Color::White => white::encode(n),
            Color::Black => black::encode(n),
        };
        while n >= 2560 {
            let bits = encode(2560).ok_or_else(|| anyhow::anyhow!("g3 makeup"))?;
            writer
                .write(bits)
                .map_err(|_| anyhow::anyhow!("g3 write"))?;
            n -= 2560;
        }
        if n >= 64 {
            let d = n & !63;
            let bits = encode(d).ok_or_else(|| anyhow::anyhow!("g3 makeup"))?;
            writer
                .write(bits)
                .map_err(|_| anyhow::anyhow!("g3 write"))?;
            n -= d;
        }
        let bits = encode(n).ok_or_else(|| anyhow::anyhow!("g3 term"))?;
        writer
            .write(bits)
            .map_err(|_| anyhow::anyhow!("g3 write"))?;
        Ok(())
    };

    for y in 0..h {
        // Optional EOL before each line (common for T.4); start with white runs.
        writer.write(EOL).map_err(|_| anyhow::anyhow!("g3 eol"))?;
        let mut run_color = Color::White;
        let mut run_len: u16 = 0;
        for x in 0..width {
            let p = img.get_pixel(u32::from(x), y).0;
            let luma = (u16::from(p[0]) + u16::from(p[1]) + u16::from(p[2])) / 3;
            let c = if luma < 128 {
                Color::Black
            } else {
                Color::White
            };
            if c == run_color {
                run_len = run_len.saturating_add(1);
            } else {
                write_run(&mut writer, run_color, run_len)?;
                run_color = c;
                run_len = 1;
            }
        }
        write_run(&mut writer, run_color, run_len)?;
    }
    // RTC: six EOLs
    for _ in 0..6 {
        writer.write(EOL).map_err(|_| anyhow::anyhow!("g3 rtc"))?;
    }
    Ok(writer.finish())
}

/// TIFF JPEG Technical Note #2 (`Compression=7`): each strip is a full JFIF stream.
fn encode_tiff_jpeg_tn2(
    pages: &[(RgbImage, u32, Option<PageSide>)],
    jpeg_quality: u8,
    opts: EncodeOpts,
    dpi: u32,
) -> Result<Vec<u8>> {
    let mut strips = Vec::with_capacity(pages.len());
    for (img, idx, side) in pages {
        let jpeg = encode_jpeg(img.clone(), *idx, *side, jpeg_quality, opts, dpi)?;
        strips.push((jpeg, img.width(), img.height()));
    }
    wrap_jpeg_tiff_tn2(&strips)
}

fn wrap_jpeg_tiff_tn2(pages: &[(Vec<u8>, u32, u32)]) -> Result<Vec<u8>> {
    if pages.is_empty() {
        bail!("empty jpeg-tn2 tiff");
    }
    // Per-page IFD; BitsPerSample [8,8,8] stored after IFD; Photometric=RGB (2); Compression=7.
    let n = pages.len();
    let entries_per = 12usize;
    let ifd_bytes = 2 + 12 * entries_per + 4;
    let bps_bytes = 6; // three SHORTs
    let header = 8usize;
    let ifd_block = n * (ifd_bytes + bps_bytes);
    let mut strip_offsets = Vec::with_capacity(n);
    let mut cursor = header + ifd_block;
    for (data, _, _) in pages {
        strip_offsets.push(cursor as u32);
        cursor += data.len();
    }

    let mut out = Vec::with_capacity(cursor);
    out.extend_from_slice(&[b'I', b'I', 42, 0]);
    let mut next_ifd = header as u32;
    out.extend_from_slice(&next_ifd.to_le_bytes());

    for (i, (data, width, height)) in pages.iter().enumerate() {
        let ifd_at = out.len();
        assert_eq!(ifd_at, next_ifd as usize);
        let bps_at = ifd_at + ifd_bytes;
        let next = if i + 1 < n {
            (bps_at + bps_bytes) as u32
        } else {
            0
        };
        out.extend_from_slice(&(entries_per as u16).to_le_bytes());
        let tags: [(u16, u16, u32, u32); 12] = [
            (256, 4, 1, *width),           // ImageWidth
            (257, 4, 1, *height),          // ImageLength
            (258, 3, 3, bps_at as u32),    // BitsPerSample → [8,8,8]
            (259, 3, 1, 7),                // Compression JPEG TN2
            (262, 3, 1, 2),                // Photometric RGB
            (273, 4, 1, strip_offsets[i]), // StripOffsets
            (274, 3, 1, 1),                // Orientation
            (277, 3, 1, 3),                // SamplesPerPixel
            (278, 4, 1, *height),          // RowsPerStrip
            (279, 4, 1, data.len() as u32),
            (284, 3, 1, 1), // PlanarConfiguration chunky
            (296, 3, 1, 2), // ResolutionUnit inch
        ];
        for (tag, typ, count, val) in tags {
            out.extend_from_slice(&tag.to_le_bytes());
            out.extend_from_slice(&typ.to_le_bytes());
            out.extend_from_slice(&count.to_le_bytes());
            out.extend_from_slice(&val.to_le_bytes());
        }
        out.extend_from_slice(&next.to_le_bytes());
        for _ in 0..3 {
            out.extend_from_slice(&8u16.to_le_bytes());
        }
        next_ifd = next;
    }
    for (data, _, _) in pages {
        out.extend_from_slice(data);
    }
    Ok(out)
}

fn encode_jpeg2k(img: &RgbImage) -> Result<Vec<u8>> {
    use openjp2::{ColorSpace, EncodeOptions, Encoder, Format, Image};

    let image =
        Image::from_interleaved_u8(img.width(), img.height(), ColorSpace::Srgb, 3, img.as_raw())
            .map_err(|e| anyhow::anyhow!("jpeg2k image: {e:?}"))?;
    let opts = EncodeOptions {
        format: Format::Jp2,
        irreversible: true,
        use_mct: true,
        rates: vec![20.0],
        ..EncodeOptions::default()
    };
    Encoder::encode(&image, &opts).map_err(|e| anyhow::anyhow!("jpeg2k encode: {e:?}"))
}

/// Multi-page G4 TIFF (II* little-endian), Photometric WhiteIsZero, Compression=4.
fn wrap_g4_multipage(pages: &[(Vec<u8>, u32, u32)]) -> Result<Vec<u8>> {
    wrap_bilevel_tiff_multipage(pages, 4, None)
}

fn wrap_bilevel_tiff_multipage(
    pages: &[(Vec<u8>, u32, u32)],
    compression: u32,
    t4_options: Option<u32>,
) -> Result<Vec<u8>> {
    if pages.is_empty() {
        bail!("no bilevel tiff pages");
    }
    let n = pages.len();
    let extra_t4 = usize::from(t4_options.is_some());
    let entries_per = 11 + extra_t4;
    let ifd_bytes = 2 + 12 * entries_per + 4;
    let rat_bytes = 16;
    let header = 8usize;
    let ifd_block = n * (ifd_bytes + rat_bytes);
    let mut strip_offsets = Vec::with_capacity(n);
    let mut cursor = header + ifd_block;
    for (data, _, _) in pages {
        strip_offsets.push(cursor as u32);
        cursor += data.len();
    }

    let mut out = Vec::with_capacity(cursor);
    out.extend_from_slice(&[b'I', b'I', 42, 0]);
    let mut next_ifd = header as u32;
    out.extend_from_slice(&next_ifd.to_le_bytes());

    for (i, (data, width, height)) in pages.iter().enumerate() {
        let ifd_at = out.len();
        assert_eq!(ifd_at, next_ifd as usize);
        let rat_at = ifd_at + ifd_bytes;
        let next = if i + 1 < n {
            (rat_at + rat_bytes) as u32
        } else {
            0
        };
        out.extend_from_slice(&(entries_per as u16).to_le_bytes());
        let mut tags: Vec<(u16, u16, u32, u32)> = vec![
            (256, 4, 1, *width),
            (257, 4, 1, *height),
            (259, 3, 1, compression),
            (262, 3, 1, 0), // WhiteIsZero
            (273, 4, 1, strip_offsets[i]),
            (274, 3, 1, 1),
            (278, 4, 1, *height),
            (279, 4, 1, data.len() as u32),
            (282, 5, 1, rat_at as u32),
            (283, 5, 1, (rat_at + 8) as u32),
            (296, 3, 1, 2),
        ];
        if let Some(opt) = t4_options {
            // T4Options must stay sorted by tag number (292) among entries.
            tags.push((292, 4, 1, opt));
            tags.sort_by_key(|(tag, _, _, _)| *tag);
        }
        for (tag, typ, count, val) in tags {
            out.extend_from_slice(&tag.to_le_bytes());
            out.extend_from_slice(&typ.to_le_bytes());
            out.extend_from_slice(&count.to_le_bytes());
            out.extend_from_slice(&val.to_le_bytes());
        }
        out.extend_from_slice(&next.to_le_bytes());
        out.extend_from_slice(&200u32.to_le_bytes());
        out.extend_from_slice(&1u32.to_le_bytes());
        out.extend_from_slice(&200u32.to_le_bytes());
        out.extend_from_slice(&1u32.to_le_bytes());
        next_ifd = next;
    }
    for (data, _, _) in pages {
        out.extend_from_slice(data);
    }
    Ok(out)
}

/// JFIF with a minimal EXIF APP1 (WSD `exif` / WIA EXIF transfer).
fn encode_exif_jpeg(
    img: RgbImage,
    page_index: u32,
    side: Option<PageSide>,
    jpeg_quality: u8,
    opts: EncodeOpts,
    dpi: u32,
) -> Result<Vec<u8>> {
    let mut jpeg = encode_jpeg(img, page_index, side, jpeg_quality, opts, dpi)?;
    inject_exif_app1(&mut jpeg)?;
    Ok(jpeg)
}

/// Minimal TIFF IFD APP1 so the blob is recognizably EXIF, not bare JFIF.
fn inject_exif_app1(jpeg: &mut Vec<u8>) -> Result<()> {
    if jpeg.len() < 2 || jpeg[0] != 0xFF || jpeg[1] != 0xD8 {
        bail!("exif inject: not a jpeg");
    }
    // APP1: "Exif\0\0" + little-endian TIFF with empty IFD (count=0).
    let tiff: [u8; 8] = [
        b'I', b'I', // little-endian
        0x2A, 0x00, // magic 42
        0x08, 0x00, 0x00, 0x00, // IFD0 offset
    ];
    // IFD: entry count 0 + next-IFD offset 0.
    let ifd: [u8; 6] = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    let mut payload = Vec::with_capacity(6 + tiff.len() + ifd.len());
    payload.extend_from_slice(b"Exif\0\0");
    payload.extend_from_slice(&tiff);
    payload.extend_from_slice(&ifd);
    let segment_len = (payload.len() + 2) as u16; // length includes itself
    let mut app1 = Vec::with_capacity(4 + payload.len());
    app1.extend_from_slice(&[0xFF, 0xE1]);
    app1.extend_from_slice(&segment_len.to_be_bytes());
    app1.extend_from_slice(&payload);
    jpeg.splice(2..2, app1);
    Ok(())
}

/// PDF with embedded JPEG page(s) (WSD `pdf-a`). Valid PDF; not certified PDF/A-1b.
fn encode_pdf_pages(
    pages: &[(RgbImage, u32, Option<PageSide>)],
    jpeg_quality: u8,
    opts: EncodeOpts,
    dpi: u32,
) -> Result<Vec<u8>> {
    let mut jpegs = Vec::with_capacity(pages.len());
    for (img, idx, side) in pages {
        let w = img.width();
        let h = img.height();
        let jpeg = encode_jpeg(img.clone(), *idx, *side, jpeg_quality, opts, dpi)?;
        jpegs.push((jpeg, w, h));
    }
    wrap_jpegs_as_pdf(&jpegs)
}

fn wrap_jpegs_as_pdf(pages: &[(Vec<u8>, u32, u32)]) -> Result<Vec<u8>> {
    if pages.is_empty() {
        bail!("empty pdf");
    }
    // Obj 1 Catalog, 2 Pages, then per page: Page, Contents, Image (3 objs each).
    let n = pages.len();
    let total_objs = 2 + 3 * n;
    let mut body = Vec::new();
    let mut offsets = vec![0usize; total_objs + 1];

    let write_obj = |out: &mut Vec<u8>, offsets: &mut [usize], num: usize, dict: &[u8]| {
        offsets[num] = out.len();
        out.extend_from_slice(format!("{num} 0 obj\n").as_bytes());
        out.extend_from_slice(dict);
        if !dict.ends_with(b"\n") {
            out.push(b'\n');
        }
        out.extend_from_slice(b"endobj\n");
    };

    body.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
    write_obj(
        &mut body,
        &mut offsets,
        1,
        b"<< /Type /Catalog /Pages 2 0 R >>",
    );

    let kids: String = (0..n).map(|i| format!("{} 0 R ", 3 + 3 * i)).collect();
    write_obj(
        &mut body,
        &mut offsets,
        2,
        format!("<< /Type /Pages /Kids [{kids}] /Count {n} >>").as_bytes(),
    );

    for (i, (jpeg, width, height)) in pages.iter().enumerate() {
        let page_obj = 3 + 3 * i;
        let content_obj = page_obj + 1;
        let image_obj = page_obj + 2;
        write_obj(
            &mut body,
            &mut offsets,
            page_obj,
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width} {height}] \
                 /Contents {content_obj} 0 R /Resources << /XObject << /Im0 {image_obj} 0 R >> >> >>"
            )
            .as_bytes(),
        );
        let content = format!("q\n{width} 0 0 {height} 0 0 cm\n/Im0 Do\nQ\n");
        offsets[content_obj] = body.len();
        body.extend_from_slice(format!("{content_obj} 0 obj\n").as_bytes());
        body.extend_from_slice(format!("<< /Length {} >>\nstream\n", content.len()).as_bytes());
        body.extend_from_slice(content.as_bytes());
        body.extend_from_slice(b"endstream\nendobj\n");

        offsets[image_obj] = body.len();
        body.extend_from_slice(format!("{image_obj} 0 obj\n").as_bytes());
        body.extend_from_slice(
            format!(
                "<< /Type /XObject /Subtype /Image /Width {width} /Height {height} \
                 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode \
                 /Length {} >>\nstream\n",
                jpeg.len()
            )
            .as_bytes(),
        );
        body.extend_from_slice(jpeg);
        body.extend_from_slice(b"\nendstream\nendobj\n");
    }

    let xref_at = body.len();
    body.extend_from_slice(format!("xref\n0 {}\n", total_objs + 1).as_bytes());
    body.extend_from_slice(b"0000000000 65535 f \n");
    for offset in offsets.iter().take(total_objs + 1).skip(1) {
        body.extend_from_slice(format!("{:010} 00000 n \n", offset).as_bytes());
    }
    body.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n",
            total_objs + 1
        )
        .as_bytes(),
    );
    Ok(body)
}

/// Minimal OpenXPS package: one FixedDocument, JPEG resources per page.
fn encode_xps_pages(
    pages: &[(RgbImage, u32, Option<PageSide>)],
    jpeg_quality: u8,
    opts: EncodeOpts,
    dpi: u32,
) -> Result<Vec<u8>> {
    use std::io::{Cursor, Write};
    use zip::write::SimpleFileOptions;
    use zip::CompressionMethod;

    let mut jpegs = Vec::with_capacity(pages.len());
    for (img, idx, side) in pages {
        jpegs.push(encode_jpeg(
            img.clone(),
            *idx,
            *side,
            jpeg_quality,
            opts,
            dpi,
        )?);
    }
    let n = jpegs.len();
    if n == 0 {
        bail!("empty xps");
    }

    let mut buf = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buf);
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        zip.start_file("[Content_Types].xml", opts)
            .map_err(|e| anyhow::anyhow!("xps: {e}"))?;
        let mut types = String::from(
            r#"<?xml version="1.0" encoding="utf-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="fdseq" ContentType="application/vnd.ms-package.xps-fixeddocumentsequence+xml"/>
  <Default Extension="fdoc" ContentType="application/vnd.ms-package.xps-fixeddocument+xml"/>
  <Default Extension="fpage" ContentType="application/vnd.ms-package.xps-fixedpage+xml"/>
  <Default Extension="jpg" ContentType="image/jpeg"/>
"#,
        );
        types.push_str("</Types>");
        zip.write_all(types.as_bytes())?;

        zip.start_file("_rels/.rels", opts)?;
        zip.write_all(
            br#"<?xml version="1.0" encoding="utf-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Type="http://schemas.microsoft.com/xps/2005/06/fixedrepresentation" Target="/FixedDocumentSequence.fdseq" Id="R0"/>
</Relationships>"#,
        )?;

        zip.start_file("FixedDocumentSequence.fdseq", opts)?;
        zip.write_all(
            br#"<?xml version="1.0" encoding="utf-8"?>
<FixedDocumentSequence xmlns="http://schemas.microsoft.com/xps/2005/06">
  <DocumentReference Source="Documents/1/FixedDocument.fdoc"/>
</FixedDocumentSequence>"#,
        )?;

        zip.start_file("Documents/1/FixedDocument.fdoc", opts)?;
        let mut fdoc = String::from(
            r#"<?xml version="1.0" encoding="utf-8"?>
<FixedDocument xmlns="http://schemas.microsoft.com/xps/2005/06">
"#,
        );
        for i in 1..=n {
            fdoc.push_str(&format!("  <PageContent Source=\"Pages/{i}.fpage\"/>\n"));
        }
        fdoc.push_str("</FixedDocument>");
        zip.write_all(fdoc.as_bytes())?;

        zip.start_file("Documents/1/_rels/FixedDocument.fdoc.rels", opts)?;
        let mut rels = String::from(
            r#"<?xml version="1.0" encoding="utf-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
"#,
        );
        for i in 1..=n {
            rels.push_str(&format!(
                r#"  <Relationship Type="http://schemas.microsoft.com/xps/2005/06/required-resource" Target="Pages/{i}.fpage" Id="P{i}"/>
"#
            ));
        }
        rels.push_str("</Relationships>");
        zip.write_all(rels.as_bytes())?;

        for (i, ((img, _, _), jpeg)) in pages.iter().zip(jpegs.iter()).enumerate() {
            let page = i + 1;
            let w = img.width();
            let h = img.height();
            zip.start_file(format!("Documents/1/Pages/{page}.fpage"), opts)?;
            let fpage = format!(
                r#"<?xml version="1.0" encoding="utf-8"?>
<FixedPage xmlns="http://schemas.microsoft.com/xps/2005/06" Width="{w}" Height="{h}" xml:lang="en-US">
  <Path Data="M 0,0 L {w},0 {w},{h} 0,{h} Z">
    <Path.Fill>
      <ImageBrush ImageSource="../Resources/{page}.jpg" Viewbox="0,0,{w},{h}" ViewboxUnits="Absolute" Viewport="0,0,{w},{h}" ViewportUnits="Absolute"/>
    </Path.Fill>
  </Path>
</FixedPage>"#
            );
            zip.write_all(fpage.as_bytes())?;

            zip.start_file(format!("Documents/1/Pages/_rels/{page}.fpage.rels"), opts)?;
            let prel = format!(
                r#"<?xml version="1.0" encoding="utf-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Type="http://schemas.microsoft.com/xps/2005/06/required-resource" Target="../Resources/{page}.jpg" Id="I{page}"/>
</Relationships>"#
            );
            zip.write_all(prel.as_bytes())?;

            zip.start_file(format!("Documents/1/Resources/{page}.jpg"), opts)?;
            zip.write_all(jpeg)?;
        }

        zip.finish()
            .map_err(|e| anyhow::anyhow!("xps finish: {e}"))?;
    }
    Ok(buf.into_inner())
}

/// MIME type for MTOM RetrieveImage binary part.
pub fn mime_for_format(wsd_format: &str) -> &'static str {
    match wsd_format {
        "jfif" | "jpeg" | "jpg" | "exif" => "image/jpeg",
        "dib" | "bmp" => "image/bmp",
        "png" => "image/png",
        "tiff-single-uncompressed"
        | "tiff"
        | "tif"
        | "tiff-single-g4"
        | "tiff-single-g3mh"
        | "tiff-single-jpeg-tn2"
        | "tiff-multi-uncompressed"
        | "tiff-multi-g4"
        | "tiff-multi-g3mh"
        | "tiff-multi-jpeg-tn2" => "image/tiff",
        "jpeg2k" => "image/jp2",
        "pdf-a" | "pdf" => "application/pdf",
        "xps" => "application/vnd.ms-xpsdocument",
        _ => "application/octet-stream",
    }
}

/// `true` when Format is a compressed WSD token (jfif, png, …).
///
/// MS [BytesPerLine remarks](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/bytesperline)
/// say compressed formats should return 0. **Inbox wsdscdrv + Brother MFC do not:**
/// live Brother CreateScanJobResponse for `jfif` returns `BytesPerLine = PixelsPerLine × 3`
/// (lab 2026-10-12: 2480×3440 → 7440). Fake with BPL=0 still CancelJob after RetrieveImage;
/// match Brother's stride for accept experiments.
pub fn format_is_compressed(wsd_format: &str) -> bool {
    !matches!(
        wsd_format,
        "dib" | "bmp" | "tiff-single-uncompressed" | "tiff" | "tif" | "tiff-multi-uncompressed"
    )
}

/// ImageInformation `BytesPerLine` for Validate/CreateScanJob.
///
/// Decoded-bitmap stride for ImageInformation / ValidateScanTicket.
///
/// Brother live **jfif** always reports `width×3` (RGB stride), even when the
/// ticket asks `Grayscale8`. Lab (2026-10-12): Gray8 + `BPL=width` → CreateScanJob
/// Pending then no RetrieveImage (Automation and pdfcraft). Gray8 + `BPL=width×3`
/// matches RGB24 accept. Uncompressed formats keep the real color stride.
///
/// When `ms_strict` is true (profile `bytes_per_line_ms`), compressed formats
/// return 0 per MS remarks; default `false` keeps the Brother stride above.
pub fn bytes_per_line(
    wsd_format: &str,
    width_px: u32,
    color: crate::effects::ColorMode,
    ms_strict: bool,
) -> u32 {
    if ms_strict && format_is_compressed(wsd_format) {
        return 0;
    }
    let compressed_jfif = matches!(
        wsd_format.to_ascii_lowercase().as_str(),
        "jfif" | "jpeg" | "exif"
    );
    if compressed_jfif {
        return width_px.saturating_mul(3);
    }
    match color {
        crate::effects::ColorMode::Rgb24 => width_px.saturating_mul(3),
        crate::effects::ColorMode::Gray8 => width_px,
        crate::effects::ColorMode::Bw1 => width_px.saturating_add(7) / 8,
    }
}

#[derive(Clone, Copy)]
enum QrPlacement {
    BottomRight,
    Center,
}

fn qr_corner_size(w: u32, h: u32) -> u32 {
    // ~18% of the short edge, at least 96px so modules survive JPEG.
    let n = ((w.min(h) as f32) * 0.18).round() as u32;
    n.max(96).min(w.min(h) / 2)
}

fn qr_focus_size(w: u32, h: u32) -> u32 {
    let n = ((w.min(h) as f32) * 0.55).round() as u32;
    n.max(160).min(w.min(h) * 4 / 5)
}

fn stamp_qr(img: &mut RgbImage, payload: &str, place: QrPlacement, target: u32) -> Result<()> {
    let code = QrCode::with_error_correction_level(payload.as_bytes(), EcLevel::M)
        .map_err(|e| anyhow::anyhow!("qr encode: {e}"))?;
    let qr: RgbImage = code
        .render::<Rgb<u8>>()
        .dark_color(Rgb([0, 0, 0]))
        .light_color(Rgb([255, 255, 255]))
        .quiet_zone(true)
        .min_dimensions(target, target)
        .build();
    let (qw, qh) = qr.dimensions();
    let (w, h) = img.dimensions();
    if qw > w || qh > h {
        bail!("page too small for QR ({w}x{h}, need {qw}x{qh})");
    }
    let (x0, y0) = match place {
        QrPlacement::BottomRight => {
            let margin = (w.min(h) / 40).max(4);
            (w - qw - margin, h - qh - margin)
        }
        QrPlacement::Center => ((w - qw) / 2, (h - qh) / 2),
    };
    for y in 0..qh {
        for x in 0..qw {
            img.put_pixel(x0 + x, y0 + y, *qr.get_pixel(x, y));
        }
    }
    Ok(())
}

fn draw_qr_focus(w: u32, h: u32, page_index: u32) -> RgbImage {
    let mut img = ImageBuffer::from_pixel(w, h, Rgb([250, 250, 248]));
    draw_border(&mut img, Rgb([40, 40, 40]));
    // Small page cue in the top margin (QR itself carries the index).
    let ink = Rgb([40, 40, 40]);
    let bars = (page_index + 1).min(12);
    let bar_w = (w / 50).max(3);
    for i in 0..bars {
        let x0 = w / 10 + i * (bar_w + bar_w);
        fill_rect(&mut img, x0, h / 40, x0 + bar_w, h / 40 + bar_w * 2, ink);
    }
    img
}

/// Render multiple lines of glyph text into an image buffer.
///
/// Each character is drawn with [`draw_glyph`] (5×7 pixels × `scale`).
/// Lines that would extend past the image bottom are silently clipped.
fn stamp_text_block(
    img: &mut RgbImage,
    lines: &[&str],
    x0: u32,
    y0: u32,
    scale: u32,
    ink: Rgb<u8>,
) {
    if scale == 0 {
        return;
    }
    let (w, h) = img.dimensions();
    let line_h = 7 * scale + scale; // glyph height + 1-scale gap between lines
    for (i, line) in lines.iter().enumerate() {
        let y = y0.saturating_add(i as u32 * line_h);
        if y + 7 * scale > h {
            break;
        }
        let mut x = x0;
        for ch in line.chars() {
            if x + 5 * scale >= w {
                break;
            }
            let advance = draw_glyph(img, ch, x, y, scale, ink);
            x = x.saturating_add(advance + scale);
        }
    }
}

/// Diagnostic page: server identity, job info, and client peer address.
///
/// Multi-line text is drawn with [`stamp_text_block`] using the existing 5×7 glyphs.
/// The top banner area is left clear for [`stamp_banner`] to overwrite later.
fn draw_info(w: u32, h: u32, page_index: u32, ctx: Option<&PageContext>) -> RgbImage {
    let mut img: RgbImage = ImageBuffer::from_pixel(w, h, Rgb([248, 250, 252]));
    draw_border(&mut img, Rgb([20, 40, 80]));

    // Accent bar below where stamp_banner will appear.
    let scale_b = (w.min(h) / 180).clamp(2, 6);
    let banner_h = 11 * scale_b; // matches stamp_banner band height
    let accent_y = banner_h + scale_b * 2;
    fill_rect(
        &mut img,
        w / 10,
        accent_y,
        w * 9 / 10,
        accent_y + scale_b * 2,
        Rgb([40, 80, 180]),
    );

    // Text scale (slightly smaller so more lines fit).
    let scale = (w.min(h) / 240).clamp(2, 5);
    let ink = Rgb([20, 40, 80]);
    let x0 = w / 20;
    let y0 = banner_h + scale * 6;

    let lines: Vec<String> = if let Some(c) = ctx {
        let peer = c
            .peer
            .as_deref()
            .filter(|p| !p.is_empty() && *p != "-")
            .unwrap_or("-");
        let uuid_short = if c.device_uuid.len() >= 8 {
            &c.device_uuid[..8]
        } else {
            c.device_uuid.as_str()
        };
        vec![
            format!("PROFILE: {}", c.profile_id.to_ascii_uppercase()),
            format!("UUID: {uuid_short}"),
            format!(
                "PATTERN: {}  DPI:{}",
                c.pattern_id.to_ascii_uppercase(),
                c.dpi
            ),
            format!(
                "FORMAT: {}  JOB:{}",
                c.format.to_ascii_uppercase(),
                c.job_id
            ),
            format!("PAGE: {page_index}"),
            format!("CLIENT: {peer}"),
            format!("TIME: {}", c.timestamp_secs),
        ]
    } else {
        vec![
            "FAKESCAN INFO".into(),
            "PROFILE: -".into(),
            "NO CONTEXT".into(),
        ]
    };
    let line_refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
    stamp_text_block(&mut img, &line_refs, x0, y0, scale, ink);
    img
}

/// ANAC-style page: displays the caller's IP address prominently.
///
/// The peer address is rendered in large glyphs centred on the page, suitable
/// for reading on a phone camera without OCR.  See `ANAC` (Automatic Number
/// Announcement Circuit) for the inspiration.
fn draw_anac(w: u32, h: u32, page_index: u32, ctx: Option<&PageContext>) -> RgbImage {
    let mut img: RgbImage = ImageBuffer::from_pixel(w, h, Rgb([245, 252, 248]));
    draw_border(&mut img, Rgb([20, 80, 40]));

    let scale_b = (w.min(h) / 180).clamp(2, 6);
    let banner_h = 11 * scale_b;
    let accent_y = banner_h + scale_b * 2;
    fill_rect(
        &mut img,
        w / 10,
        accent_y,
        w * 9 / 10,
        accent_y + scale_b * 2,
        Rgb([40, 140, 60]),
    );

    let ink_dark = Rgb([20, 60, 20]);
    let ink_mid = Rgb([60, 90, 60]);

    let peer = ctx
        .and_then(|c| c.peer.as_deref())
        .filter(|p| !p.is_empty() && *p != "-")
        .unwrap_or("-");

    // "YOUR IP" header in medium text, horizontally centred.
    let scale_med = (w.min(h) / 280).clamp(2, 4);
    let label = "YOUR IP";
    let label_px_w = label.len() as u32 * (5 * scale_med + scale_med);
    let label_x = w.saturating_sub(label_px_w) / 2;
    let label_y = banner_h + scale_med * 10;
    stamp_text_block(&mut img, &[label], label_x, label_y, scale_med, ink_mid);

    // IP address in large text, horizontally centred.
    let scale_lg = (w.min(h) / 110).clamp(3, 8);
    let ip_px_w = peer.len() as u32 * (5 * scale_lg + scale_lg);
    let ip_x = w.saturating_sub(ip_px_w) / 2;
    let ip_y = h / 3;
    stamp_text_block(&mut img, &[peer], ip_x, ip_y, scale_lg, ink_dark);

    // Page indicator at bottom in small text.
    let scale_sm = (w.min(h) / 320).clamp(1, 3);
    let pg_str = format!("PAGE: {page_index}");
    stamp_text_block(
        &mut img,
        &[pg_str.as_str()],
        w / 20,
        h * 4 / 5,
        scale_sm,
        ink_mid,
    );

    img
}

pub(crate) fn fill_rect(img: &mut RgbImage, x0: u32, y0: u32, x1: u32, y1: u32, color: Rgb<u8>) {
    let (w, h) = img.dimensions();
    let x0 = x0.min(w);
    let y0 = y0.min(h);
    let x1 = x1.min(w);
    let y1 = y1.min(h);
    for y in y0..y1 {
        for x in x0..x1 {
            img.put_pixel(x, y, color);
        }
    }
}

pub(crate) fn draw_border(img: &mut RgbImage, color: Rgb<u8>) {
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return;
    }
    for x in 0..w {
        img.put_pixel(x, 0, color);
        img.put_pixel(x, h - 1, color);
    }
    for y in 0..h {
        img.put_pixel(0, y, color);
        img.put_pixel(w - 1, y, color);
    }
}

/// Current numbered bars + center block + border.
fn draw_page_id(w: u32, h: u32, page_index: u32) -> RgbImage {
    let mut img: RgbImage = ImageBuffer::from_pixel(w, h, Rgb([245, 245, 240]));
    let ink = Rgb([20, 40, 80]);
    let accent = Rgb([180, 60, 40]);
    draw_border(&mut img, ink);

    let band = (h / 20).max(4);
    fill_rect(&mut img, w / 10, band * 2, w * 9 / 10, band * 3, accent);

    let bars = (page_index + 1).min(16);
    let bar_w = (w / 40).max(4);
    let gap = bar_w;
    let top = band * 4;
    let bar_h = band * 2;
    for i in 0..bars {
        let x0 = w / 10 + i * (bar_w + gap);
        let x1 = (x0 + bar_w).min(w.saturating_sub(1));
        fill_rect(&mut img, x0, top, x1, (top + bar_h).min(h), ink);
    }

    let mark = 8 + (page_index % 7) * 12;
    let cx = w / 2;
    let cy = h / 2;
    fill_rect(
        &mut img,
        cx.saturating_sub(mark),
        cy.saturating_sub(mark),
        (cx + mark).min(w),
        (cy + mark).min(h),
        ink,
    );
    img
}

/// Horizontal gray step wedges (black → white). Page index shifts the step count slightly.
fn draw_wedges(w: u32, h: u32, page_index: u32) -> RgbImage {
    let mut img: RgbImage = ImageBuffer::from_pixel(w, h, Rgb([128, 128, 128]));
    let steps = (8 + (page_index % 5)).max(4);
    let step_h = (h / steps).max(1);
    for i in 0..steps {
        let t = if steps <= 1 {
            255u8
        } else {
            ((i * 255) / (steps - 1)) as u8
        };
        let y0 = i * step_h;
        let y1 = if i + 1 == steps {
            h
        } else {
            ((i + 1) * step_h).min(h)
        };
        fill_rect(&mut img, 0, y0, w, y1, Rgb([t, t, t]));
    }
    // Narrow ink border so the frame is visible on mid-gray steps.
    draw_border(&mut img, Rgb([0, 0, 0]));
    img
}

/// R / G / B / gray patches in a 2×2 grid (page index tints the gray cell).
fn draw_patches(w: u32, h: u32, page_index: u32) -> RgbImage {
    let mut img: RgbImage = ImageBuffer::from_pixel(w, h, Rgb([240, 240, 240]));
    let mid_x = w / 2;
    let mid_y = h / 2;
    let gray = (40u32 + (page_index % 8) * 24).min(220) as u8;
    fill_rect(&mut img, 0, 0, mid_x, mid_y, Rgb([220, 40, 40])); // R
    fill_rect(&mut img, mid_x, 0, w, mid_y, Rgb([40, 180, 50])); // G
    fill_rect(&mut img, 0, mid_y, mid_x, h, Rgb([40, 80, 220])); // B
    fill_rect(&mut img, mid_x, mid_y, w, h, Rgb([gray, gray, gray]));
    draw_border(&mut img, Rgb([20, 20, 20]));
    // Thin crosshair so patch boundaries survive mild crop.
    fill_rect(
        &mut img,
        mid_x.saturating_sub(1),
        0,
        (mid_x + 1).min(w),
        h,
        Rgb([20, 20, 20]),
    );
    fill_rect(
        &mut img,
        0,
        mid_y.saturating_sub(1),
        w,
        (mid_y + 1).min(h),
        Rgb([20, 20, 20]),
    );
    img
}

/// Large asymmetric F anchored in the top-left corner (orientation cue).
fn draw_slant_f(w: u32, h: u32, page_index: u32) -> RgbImage {
    let mut img: RgbImage = ImageBuffer::from_pixel(w, h, Rgb([250, 250, 245]));
    let ink = Rgb([15, 15, 15]);
    draw_border(&mut img, ink);

    // Margin grows slightly with page index so pages stay distinct.
    let margin = (w / 12).max(8) + (page_index % 4) * 4;
    let stem_w = (w / 10).max(12);
    let arm_h = (h / 14).max(10);
    let f_h = (h * 2 / 5).max(stem_w * 4);
    let top_arm = (w * 2 / 5).max(stem_w * 3);
    let mid_arm = (w * 3 / 10).max(stem_w * 2); // shorter → asymmetric

    // Vertical stem
    fill_rect(&mut img, margin, margin, margin + stem_w, margin + f_h, ink);
    // Top arm
    fill_rect(
        &mut img,
        margin,
        margin,
        margin + top_arm,
        margin + arm_h,
        ink,
    );
    // Middle arm (shorter, lower third of stem)
    let mid_y = margin + f_h / 3;
    fill_rect(
        &mut img,
        margin,
        mid_y,
        margin + mid_arm,
        mid_y + arm_h,
        ink,
    );
    img
}

/// Blue front / red back faces for manual duplex. Sheet index still drives bar count.
fn draw_duplex_pair(w: u32, h: u32, page_index: u32, side: PageSide) -> RgbImage {
    let (bg, accent, ink) = match side {
        PageSide::Front => (
            Rgb([220, 235, 250]), // cool / blue wash
            Rgb([40, 90, 200]),
            Rgb([15, 35, 90]),
        ),
        PageSide::Back => (
            Rgb([250, 225, 220]), // warm / red wash
            Rgb([200, 50, 40]),
            Rgb([90, 20, 15]),
        ),
    };
    let mut img: RgbImage = ImageBuffer::from_pixel(w, h, bg);
    draw_border(&mut img, ink);

    let band = (h / 20).max(4);
    fill_rect(&mut img, w / 10, band * 2, w * 9 / 10, band * 3, accent);

    // Side stripe along the left (front) or right (back) edge — orientation cue after flip.
    let stripe = (w / 25).max(6);
    match side {
        PageSide::Front => fill_rect(&mut img, 0, 0, stripe, h, accent),
        PageSide::Back => fill_rect(&mut img, w.saturating_sub(stripe), 0, w, h, accent),
    }

    let bars = (page_index + 1).min(16);
    let bar_w = (w / 40).max(4);
    let gap = bar_w;
    let top = band * 4;
    let bar_h = band * 2;
    for i in 0..bars {
        let x0 = w / 10 + i * (bar_w + gap);
        let x1 = (x0 + bar_w).min(w.saturating_sub(1));
        fill_rect(&mut img, x0, top, x1, (top + bar_h).min(h), ink);
    }

    let mark = 8 + (page_index % 7) * 12;
    let cx = w / 2;
    let cy = h / 2;
    fill_rect(
        &mut img,
        cx.saturating_sub(mark),
        cy.saturating_sub(mark),
        (cx + mark).min(w),
        (cy + mark).min(h),
        ink,
    );
    img
}

/// L-marks in four corners + center cross for crop / region tests.
fn draw_corners(w: u32, h: u32, page_index: u32) -> RgbImage {
    let mut img: RgbImage = ImageBuffer::from_pixel(w, h, Rgb([248, 248, 242]));
    let ink = Rgb([30, 30, 30]);
    let accent = Rgb([160, 50, 40]);
    let arm = ((w.min(h) / 12).max(16) + (page_index % 3) * 4).min(w.min(h) / 4);
    let thick = (arm / 6).max(3);

    let draw_l = |img: &mut RgbImage, ox: u32, oy: u32, right: bool, down: bool, color: Rgb<u8>| {
        let x_end = if right {
            (ox + arm).min(w)
        } else {
            ox.saturating_sub(arm)
        };
        let y_end = if down {
            (oy + arm).min(h)
        } else {
            oy.saturating_sub(arm)
        };
        let (hx0, hx1) = if right { (ox, x_end) } else { (x_end, ox) };
        let (vy0, vy1) = if down { (oy, y_end) } else { (y_end, oy) };
        // Horizontal leg
        fill_rect(
            img,
            hx0,
            oy.saturating_sub(thick / 2),
            hx1,
            (oy + thick / 2 + 1).min(h),
            color,
        );
        // Vertical leg
        fill_rect(
            img,
            ox.saturating_sub(thick / 2),
            vy0,
            (ox + thick / 2 + 1).min(w),
            vy1,
            color,
        );
    };

    let inset = (w.min(h) / 40).max(4);
    draw_l(&mut img, inset, inset, true, true, ink); // top-left
    draw_l(
        &mut img,
        w.saturating_sub(inset + 1),
        inset,
        false,
        true,
        ink,
    ); // top-right
    draw_l(
        &mut img,
        inset,
        h.saturating_sub(inset + 1),
        true,
        false,
        ink,
    ); // bottom-left
    draw_l(
        &mut img,
        w.saturating_sub(inset + 1),
        h.saturating_sub(inset + 1),
        false,
        false,
        ink,
    ); // bottom-right

    // Center cross
    let cx = w / 2;
    let cy = h / 2;
    let cross = arm;
    fill_rect(
        &mut img,
        cx.saturating_sub(cross),
        cy.saturating_sub(thick / 2),
        (cx + cross).min(w),
        (cy + thick / 2 + 1).min(h),
        accent,
    );
    fill_rect(
        &mut img,
        cx.saturating_sub(thick / 2),
        cy.saturating_sub(cross),
        (cx + thick / 2 + 1).min(w),
        (cy + cross).min(h),
        accent,
    );
    img
}

/// Insert a JPEG COM marker after SOI carrying `fakescan-page:N`.
fn inject_page_com(jpeg: &mut Vec<u8>, page_index: u32) -> Result<()> {
    let payload = format!(
        "{}{}",
        std::str::from_utf8(PAGE_MARKER_PREFIX).unwrap(),
        page_index
    );
    inject_com_payload(jpeg, payload.as_bytes())
}

fn inject_side_com(jpeg: &mut Vec<u8>, side: PageSide) -> Result<()> {
    let payload = format!(
        "{}{}",
        std::str::from_utf8(SIDE_MARKER_PREFIX).unwrap(),
        side.as_str()
    );
    inject_com_payload(jpeg, payload.as_bytes())
}

fn inject_com_payload(jpeg: &mut Vec<u8>, payload_bytes: &[u8]) -> Result<()> {
    if jpeg.len() < 2 || jpeg[0] != 0xFF || jpeg[1] != 0xD8 {
        bail!("not a JPEG (missing SOI)");
    }
    let segment_len = 2 + payload_bytes.len(); // length field includes itself
    if segment_len > u16::MAX as usize {
        bail!("COM payload too long");
    }
    let mut segment = Vec::with_capacity(2 + segment_len);
    segment.push(0xFF);
    segment.push(0xFE); // COM
    segment.extend_from_slice(&(segment_len as u16).to_be_bytes());
    segment.extend_from_slice(payload_bytes);
    jpeg.splice(2..2, segment);
    Ok(())
}

fn for_each_com<F>(jpeg: &[u8], mut f: F)
where
    F: FnMut(&[u8]) -> bool,
{
    let mut i = 0;
    while i + 4 < jpeg.len() {
        if jpeg[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = jpeg[i + 1];
        if marker == 0xD8 {
            i += 2;
            continue;
        }
        if marker == 0xD9 || marker == 0xDA {
            break;
        }
        if i + 4 > jpeg.len() {
            break;
        }
        let len = u16::from_be_bytes([jpeg[i + 2], jpeg[i + 3]]) as usize;
        if len < 2 || i + 2 + len > jpeg.len() {
            break;
        }
        if marker == 0xFE {
            let data = &jpeg[i + 4..i + 2 + len];
            if !f(data) {
                return;
            }
        }
        i += 2 + len;
    }
}

/// Collect JPEG marker bytes from segment headers up to (and including) SOS.
///
/// Useful for unit tests: COM=`0xFE`, APP0=`0xE0`, APP1=`0xE1`, SOF0=`0xC0`,
/// SOF2 progressive=`0xC2`. Does not walk entropy-coded scan data.
pub fn jpeg_segment_markers(jpeg: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    if jpeg.len() < 4 || jpeg[0] != 0xFF || jpeg[1] != 0xD8 {
        return out;
    }
    out.push(0xD8); // SOI
    let mut i = 2usize;
    while i + 1 < jpeg.len() {
        if jpeg[i] != 0xFF {
            break;
        }
        let mut j = i + 1;
        while j < jpeg.len() && jpeg[j] == 0xFF {
            j += 1;
        }
        if j >= jpeg.len() {
            break;
        }
        let marker = jpeg[j];
        out.push(marker);
        i = j + 1;
        // Standalone markers (no length): RSTn, SOI, EOI, TEM.
        if matches!(marker, 0xD0..=0xD9 | 0x01) {
            if marker == 0xD9 {
                break;
            }
            continue;
        }
        if i + 2 > jpeg.len() {
            break;
        }
        let len = u16::from_be_bytes([jpeg[i], jpeg[i + 1]]) as usize;
        if len < 2 || i + len > jpeg.len() {
            break;
        }
        i += len;
        // Stop after SOS — remaining bytes are entropy-coded.
        if marker == 0xDA {
            break;
        }
    }
    out
}

/// True when any COM (`0xFE`) segment is present before SOS.
pub fn jpeg_has_com(jpeg: &[u8]) -> bool {
    jpeg_segment_markers(jpeg).contains(&0xFE)
}

/// True when APP1 (`0xE1`, typically EXIF) is present before SOS.
pub fn jpeg_has_app1(jpeg: &[u8]) -> bool {
    jpeg_segment_markers(jpeg).contains(&0xE1)
}

/// True when the frame is baseline SOF0 and not progressive SOF2.
pub fn jpeg_is_baseline(jpeg: &[u8]) -> bool {
    let m = jpeg_segment_markers(jpeg);
    m.contains(&0xC0) && !m.contains(&0xC2)
}

/// SOF0 `(width, height)` in pixels, or `None` if no baseline SOF0.
///
/// RetrieveImage SOF must match Create/Validate `PixelsPerLine` / `NumberOfLines`
/// ([docs/specs/mtom-retrieveimage.md](../../../docs/specs/mtom-retrieveimage.md)).
pub fn jpeg_sof0_dimensions(jpeg: &[u8]) -> Option<(u32, u32)> {
    let mut i = 0usize;
    while i + 4 < jpeg.len() {
        if jpeg[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = jpeg[i + 1];
        if marker == 0xD8 {
            i += 2;
            continue;
        }
        if marker == 0xD9 || marker == 0xDA {
            break;
        }
        if i + 4 > jpeg.len() {
            break;
        }
        let len = u16::from_be_bytes([jpeg[i + 2], jpeg[i + 3]]) as usize;
        if len < 2 || i + 2 + len > jpeg.len() {
            break;
        }
        if marker == 0xC0 {
            let payload = &jpeg[i + 4..i + 2 + len];
            // precision(1) + height(2) + width(2) + nf(1) …
            if payload.len() < 5 {
                return None;
            }
            let height = u16::from_be_bytes([payload[1], payload[2]]) as u32;
            let width = u16::from_be_bytes([payload[3], payload[4]]) as u32;
            return Some((width, height));
        }
        i += 2 + len;
    }
    None
}

/// SOF0 component sampling: `(id, H, V)` triples, or empty if no SOF0.
pub fn jpeg_sof0_components(jpeg: &[u8]) -> Vec<(u8, u8, u8)> {
    let mut i = 0usize;
    while i + 4 < jpeg.len() {
        if jpeg[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = jpeg[i + 1];
        if marker == 0xD8 {
            i += 2;
            continue;
        }
        if marker == 0xD9 || marker == 0xDA {
            break;
        }
        if i + 4 > jpeg.len() {
            break;
        }
        let len = u16::from_be_bytes([jpeg[i + 2], jpeg[i + 3]]) as usize;
        if len < 2 || i + 2 + len > jpeg.len() {
            break;
        }
        if marker == 0xC0 {
            let payload = &jpeg[i + 4..i + 2 + len];
            if payload.len() < 6 {
                return Vec::new();
            }
            let nf = payload[5] as usize;
            let mut comps = Vec::with_capacity(nf);
            let mut off = 6;
            for _ in 0..nf {
                if off + 3 > payload.len() {
                    break;
                }
                let id = payload[off];
                let hv = payload[off + 1];
                comps.push((id, hv >> 4, hv & 0xF));
                off += 3;
            }
            return comps;
        }
        i += 2 + len;
    }
    Vec::new()
}

/// APP0/JFIF `(units, xdensity, ydensity)` when present.
pub fn jpeg_jfif_density(jpeg: &[u8]) -> Option<(u8, u16, u16)> {
    let mut i = 0usize;
    while i + 4 < jpeg.len() {
        if jpeg[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = jpeg[i + 1];
        if marker == 0xD8 {
            i += 2;
            continue;
        }
        if marker == 0xD9 || marker == 0xDA {
            break;
        }
        if i + 4 > jpeg.len() {
            break;
        }
        let len = u16::from_be_bytes([jpeg[i + 2], jpeg[i + 3]]) as usize;
        if len < 2 || i + 2 + len > jpeg.len() {
            break;
        }
        if marker == 0xE0 {
            let payload = &jpeg[i + 4..i + 2 + len];
            if payload.len() >= 14 && payload.starts_with(b"JFIF\0") {
                let units = payload[7];
                let x = u16::from_be_bytes([payload[8], payload[9]]);
                let y = u16::from_be_bytes([payload[10], payload[11]]);
                return Some((units, x, y));
            }
            return None;
        }
        i += 2 + len;
    }
    None
}

/// DRI restart interval, if a DRI segment is present before SOS.
pub fn jpeg_dri(jpeg: &[u8]) -> Option<u16> {
    let mut i = 0usize;
    while i + 4 < jpeg.len() {
        if jpeg[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = jpeg[i + 1];
        if marker == 0xD8 {
            i += 2;
            continue;
        }
        if marker == 0xD9 || marker == 0xDA {
            break;
        }
        if i + 4 > jpeg.len() {
            break;
        }
        let len = u16::from_be_bytes([jpeg[i + 2], jpeg[i + 3]]) as usize;
        if len < 2 || i + 2 + len > jpeg.len() {
            break;
        }
        if marker == 0xDD {
            let payload = &jpeg[i + 4..i + 2 + len];
            if payload.len() >= 2 {
                return Some(u16::from_be_bytes([payload[0], payload[1]]));
            }
            return None;
        }
        i += 2 + len;
    }
    None
}

/// Read the page index from a fakescan JPEG COM marker, if present.
pub fn read_page_marker(jpeg: &[u8]) -> Option<u32> {
    let mut found = None;
    for_each_com(jpeg, |data| {
        if let Some(rest) = data.strip_prefix(PAGE_MARKER_PREFIX) {
            if let Ok(s) = std::str::from_utf8(rest) {
                found = s.parse().ok();
                return false;
            }
        }
        true
    });
    found
}

/// Read the duplex side from a fakescan JPEG COM marker, if present.
pub fn read_side_marker(jpeg: &[u8]) -> Option<PageSide> {
    let mut found = None;
    for_each_com(jpeg, |data| {
        if let Some(rest) = data.strip_prefix(SIDE_MARKER_PREFIX) {
            if let Ok(s) = std::str::from_utf8(rest) {
                found = PageSide::parse(s).ok();
                return false;
            }
        }
        true
    });
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn page_markers_encode_distinct_indices() {
        let p0 = jpeg_page(100, 0).unwrap();
        let p1 = jpeg_page(100, 1).unwrap();
        let p2 = jpeg_page(100, 2).unwrap();
        assert_eq!(read_page_marker(&p0), Some(0));
        assert_eq!(read_page_marker(&p1), Some(1));
        assert_eq!(read_page_marker(&p2), Some(2));
        assert_ne!(p0, p1);
        assert_ne!(p1, p2);
    }

    #[test]
    fn each_pattern_id_produces_readable_com_and_distinct_bytes() {
        let mut digests = HashSet::new();
        for &pattern in PatternId::ALL {
            let bytes =
                jpeg_pattern(pattern, 100, 0).unwrap_or_else(|_| panic!("{}", pattern.as_str()));
            assert_eq!(
                read_page_marker(&bytes),
                Some(0),
                "COM missing for {}",
                pattern.as_str()
            );
            assert!(
                digests.insert(bytes.clone()),
                "pattern {} collided with another pattern's JPEG",
                pattern.as_str()
            );
            // Page index still differs within a pattern.
            let page1 = jpeg_pattern(pattern, 100, 1).unwrap();
            assert_eq!(read_page_marker(&page1), Some(1));
            assert_ne!(bytes, page1);
        }
        assert_eq!(digests.len(), PatternId::ALL.len());
    }

    #[test]
    fn pattern_id_parse_roundtrip() {
        for &p in PatternId::ALL {
            assert_eq!(PatternId::parse(p.as_str()).unwrap(), p);
        }
        assert!(PatternId::parse("nope").is_err());
        assert_eq!(PatternId::parse("default").unwrap(), PatternId::PageId);
        assert_eq!(PatternId::parse("qr").unwrap(), PatternId::Qr);
    }

    #[test]
    fn qr_payload_roundtrips_without_ocr() {
        let text = qr_payload(PatternId::Wedges, 3, Some(PageSide::Back));
        let parsed = parse_qr_payload(&text).expect("parse");
        assert_eq!(parsed.page, 3);
        assert_eq!(parsed.pattern, PatternId::Wedges);
        assert_eq!(parsed.side, Some(PageSide::Back));
        assert!(parse_qr_payload("not-a-fakescan-code").is_none());
    }

    #[test]
    fn qr_on_page_decodes_after_jpeg() {
        // Large `qr` pattern + quality 95 should survive rqrr after JPEG.
        let jpeg = jpeg_pattern(PatternId::Qr, 150, 2).unwrap();
        let img = image::load_from_memory(&jpeg).unwrap().to_luma8();
        let mut prepared = rqrr::PreparedImage::prepare(img);
        let grids = prepared.detect_grids();
        assert!(!grids.is_empty(), "no QR detected in qr pattern JPEG");
        let (_meta, content) = grids[0].decode().expect("qr decode");
        let parsed = parse_qr_payload(&content).expect("payload");
        assert_eq!(parsed.page, 2);
        assert_eq!(parsed.pattern, PatternId::Qr);
        assert_eq!(read_page_marker(&jpeg), Some(2));
    }

    #[test]
    fn corner_qr_on_page_id_decodes() {
        let jpeg = jpeg_pattern(PatternId::PageId, 150, 0).unwrap();
        let img = image::load_from_memory(&jpeg).unwrap().to_luma8();
        let mut prepared = rqrr::PreparedImage::prepare(img);
        let grids = prepared.detect_grids();
        assert!(!grids.is_empty(), "corner QR missing on page_id");
        let (_meta, content) = grids[0].decode().expect("decode");
        let parsed = parse_qr_payload(&content).unwrap();
        assert_eq!(parsed.page, 0);
        assert_eq!(parsed.pattern, PatternId::PageId);
    }

    #[test]
    fn jpeg_page_matches_page_id_pattern() {
        let a = jpeg_page(100, 3).unwrap();
        let b = jpeg_pattern(PatternId::PageId, 100, 3).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn duplex_pair_tags_even_front_odd_back() {
        let front = jpeg_pattern(PatternId::DuplexPair, 100, 0).unwrap();
        let back = jpeg_pattern(PatternId::DuplexPair, 100, 1).unwrap();
        assert_eq!(read_page_marker(&front), Some(0));
        assert_eq!(read_page_marker(&back), Some(1));
        assert_eq!(read_side_marker(&front), Some(PageSide::Front));
        assert_eq!(read_side_marker(&back), Some(PageSide::Back));
        assert_ne!(front, back);
    }

    #[test]
    fn duplex_pair_forced_side_overrides_parity() {
        // Odd sheet index but forced front → still front (blue stack for pass 1).
        let forced =
            jpeg_pattern_with_side(PatternId::DuplexPair, 100, 1, Some(PageSide::Front)).unwrap();
        assert_eq!(read_page_marker(&forced), Some(1));
        assert_eq!(read_side_marker(&forced), Some(PageSide::Front));
        let auto_back = jpeg_pattern(PatternId::DuplexPair, 100, 1).unwrap();
        assert_eq!(read_side_marker(&auto_back), Some(PageSide::Back));
        assert_ne!(forced, auto_back);
    }

    #[test]
    fn page_side_parse_roundtrip() {
        assert_eq!(PageSide::parse("front").unwrap(), PageSide::Front);
        assert_eq!(PageSide::parse("BACK").unwrap(), PageSide::Back);
        assert!(PageSide::parse("sideways").is_err());
    }

    #[test]
    fn bytes_per_line_zero_when_compressed() {
        use crate::effects::ColorMode;
        // Default / ms_strict=false: Brother/wsdscdrv jfif RGB stride.
        assert_eq!(bytes_per_line("jfif", 850, ColorMode::Rgb24, false), 2550);
        assert_eq!(bytes_per_line("jfif", 850, ColorMode::Gray8, false), 2550);
        assert_eq!(bytes_per_line("png", 850, ColorMode::Gray8, false), 850);
        assert_eq!(bytes_per_line("dib", 850, ColorMode::Rgb24, false), 850 * 3);
        assert_eq!(
            bytes_per_line("tiff-single-uncompressed", 850, ColorMode::Gray8, false),
            850
        );
        assert_eq!(bytes_per_line("dib", 900, ColorMode::Bw1, false), 113);
        // ms_strict=true: compressed → 0; uncompressed stride unchanged.
        assert_eq!(bytes_per_line("jfif", 850, ColorMode::Rgb24, true), 0);
        assert_eq!(bytes_per_line("jfif", 850, ColorMode::Gray8, true), 0);
        assert_eq!(bytes_per_line("png", 850, ColorMode::Gray8, true), 0);
        assert_eq!(bytes_per_line("dib", 850, ColorMode::Rgb24, true), 850 * 3);
    }

    #[test]
    fn encode_wsd_formats_have_magic() {
        let jfif = encode_pattern(
            "jfif",
            PatternId::PageId,
            50,
            0,
            None,
            None,
            crate::effects::ColorMode::Rgb24,
            EncodeOpts::WITH_COM,
        )
        .unwrap();
        assert_eq!(&jfif[..2], &[0xFF, 0xD8]);
        assert_eq!(mime_for_format("jfif"), "image/jpeg");

        let dib = encode_pattern(
            "dib",
            PatternId::PageId,
            50,
            0,
            None,
            None,
            crate::effects::ColorMode::Rgb24,
            EncodeOpts::WITH_COM,
        )
        .unwrap();
        assert_eq!(&dib[..2], b"BM");
        assert_eq!(mime_for_format("dib"), "image/bmp");

        let png = encode_pattern(
            "png",
            PatternId::PageId,
            50,
            0,
            None,
            None,
            crate::effects::ColorMode::Rgb24,
            EncodeOpts::WITH_COM,
        )
        .unwrap();
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        assert_eq!(mime_for_format("png"), "image/png");

        let tiff = encode_pattern(
            "tiff-single-uncompressed",
            PatternId::PageId,
            50,
            0,
            None,
            None,
            crate::effects::ColorMode::Rgb24,
            EncodeOpts::WITH_COM,
        )
        .unwrap();
        assert!(
            tiff[..4] == [0x49, 0x49, 0x2A, 0x00] || tiff[..4] == [0x4D, 0x4D, 0x00, 0x2A],
            "tiff magic {:?}",
            &tiff[..4]
        );
        assert_eq!(mime_for_format("tiff-single-uncompressed"), "image/tiff");

        let exif = encode_pattern(
            "exif",
            PatternId::PageId,
            50,
            0,
            None,
            None,
            crate::effects::ColorMode::Rgb24,
            EncodeOpts::WITH_COM,
        )
        .unwrap();
        assert_eq!(&exif[..2], &[0xFF, 0xD8]);
        assert!(
            exif.windows(6).any(|w| w == b"Exif\0\0"),
            "missing EXIF APP1"
        );
        assert_eq!(mime_for_format("exif"), "image/jpeg");

        let pdf = encode_pattern(
            "pdf-a",
            PatternId::PageId,
            50,
            0,
            None,
            None,
            crate::effects::ColorMode::Rgb24,
            EncodeOpts::WITH_COM,
        )
        .unwrap();
        assert!(pdf.starts_with(b"%PDF-1.4"), "pdf header {:?}", &pdf[..8]);
        assert!(pdf.windows(4).any(|w| w == b"%%EOF") || pdf.ends_with(b"%%EOF\n"));
        assert_eq!(mime_for_format("pdf-a"), "application/pdf");

        let g4 = encode_pattern(
            "tiff-single-g4",
            PatternId::PageId,
            50,
            0,
            None,
            None,
            crate::effects::ColorMode::Rgb24,
            EncodeOpts::WITH_COM,
        )
        .unwrap();
        assert_eq!(&g4[..4], &[0x49, 0x49, 0x2A, 0x00]);
        assert_eq!(mime_for_format("tiff-single-g4"), "image/tiff");

        let multi = encode_pattern_pages(
            "tiff-multi-uncompressed",
            PatternId::PageId,
            50,
            &[0, 1],
            None,
            None,
            crate::effects::ColorMode::Rgb24,
            EncodeOpts::WITH_COM,
        )
        .unwrap();
        assert_eq!(&multi[..4], &[0x49, 0x49, 0x2A, 0x00]);

        let xps = encode_pattern(
            "xps",
            PatternId::PageId,
            50,
            0,
            None,
            None,
            crate::effects::ColorMode::Rgb24,
            EncodeOpts::WITH_COM,
        )
        .unwrap();
        assert_eq!(&xps[..2], b"PK", "xps is a zip");
        assert_eq!(mime_for_format("xps"), "application/vnd.ms-xpsdocument");

        let jpeg_tiff = encode_pattern(
            "tiff-single-jpeg-tn2",
            PatternId::PageId,
            50,
            0,
            None,
            None,
            crate::effects::ColorMode::Rgb24,
            EncodeOpts::WITH_COM,
        )
        .unwrap();
        assert_eq!(&jpeg_tiff[..4], &[0x49, 0x49, 0x2A, 0x00]);
        assert!(
            jpeg_tiff.windows(2).any(|w| w == [0xFF, 0xD8]),
            "jpeg-tn2 strip should contain SOI"
        );

        let g3 = encode_pattern(
            "tiff-single-g3mh",
            PatternId::PageId,
            50,
            0,
            None,
            None,
            crate::effects::ColorMode::Rgb24,
            EncodeOpts::WITH_COM,
        )
        .unwrap();
        assert_eq!(&g3[..4], &[0x49, 0x49, 0x2A, 0x00]);

        let jp2 = encode_pattern(
            "jpeg2k",
            PatternId::PageId,
            50,
            0,
            None,
            None,
            crate::effects::ColorMode::Rgb24,
            EncodeOpts::WITH_COM,
        )
        .unwrap();
        assert!(
            jp2.len() > 32 && (jp2[4..8] == *b"jP  " || jp2.starts_with(&[0xFF, 0x4F])),
            "jp2/j2k magic {:?}",
            &jp2[..8.min(jp2.len())]
        );
        assert_eq!(mime_for_format("jpeg2k"), "image/jp2");
    }

    #[test]
    fn encode_opts_can_omit_jpeg_com() {
        let with_com = encode_pattern(
            "jfif",
            PatternId::PageId,
            50,
            3,
            Some(PageSide::Back),
            None,
            crate::effects::ColorMode::Rgb24,
            EncodeOpts::WITH_COM,
        )
        .unwrap();
        assert_eq!(read_page_marker(&with_com), Some(3));
        assert_eq!(read_side_marker(&with_com), Some(PageSide::Back));

        let bare = encode_pattern(
            "jfif",
            PatternId::PageId,
            50,
            3,
            Some(PageSide::Back),
            None,
            crate::effects::ColorMode::Rgb24,
            EncodeOpts::NO_COM,
        )
        .unwrap();
        assert_eq!(&bare[..2], &[0xFF, 0xD8]);
        assert_eq!(read_page_marker(&bare), None);
        assert_eq!(read_side_marker(&bare), None);
        // Still a distinct page (QR / pattern differ by index).
        assert_ne!(
            encode_pattern(
                "jfif",
                PatternId::PageId,
                50,
                0,
                None,
                None,
                crate::effects::ColorMode::Rgb24,
                EncodeOpts::NO_COM,
            )
            .unwrap(),
            bare
        );
    }

    #[test]
    fn encode_opts_for_wia_plain_forces_plain_knobs() {
        assert_eq!(
            encode_opts_for(PatternId::WiaPlain, true),
            EncodeOpts::WIA_PLAIN
        );
        assert_eq!(
            encode_opts_for(PatternId::WiaPlain, false),
            EncodeOpts::WIA_PLAIN
        );
        assert_eq!(
            encode_opts_for(PatternId::Wedges, false),
            EncodeOpts::NO_COM
        );
        assert_eq!(
            encode_opts_for(PatternId::Wedges, true),
            EncodeOpts::WITH_COM
        );
    }

    #[test]
    fn wia_plain_jfif_omits_com_app1_qr_and_is_baseline() {
        let opts = EncodeOpts::WIA_PLAIN;
        let bytes = encode_pattern(
            "jfif",
            PatternId::WiaPlain,
            100,
            0,
            None,
            None,
            crate::effects::ColorMode::Rgb24,
            opts,
        )
        .unwrap();
        assert_eq!(&bytes[..2], &[0xFF, 0xD8]);
        assert!(
            jpeg_is_baseline(&bytes),
            "markers={:?}",
            jpeg_segment_markers(&bytes)
        );
        assert!(
            !jpeg_has_com(&bytes),
            "COM present: markers={:?}",
            jpeg_segment_markers(&bytes)
        );
        assert!(
            !jpeg_has_app1(&bytes),
            "APP1 present: markers={:?}",
            jpeg_segment_markers(&bytes)
        );
        assert_eq!(read_page_marker(&bytes), None);
        assert_eq!(read_side_marker(&bytes), None);
        // JFIF APP0 is expected on baseline scanner-shaped encode.
        assert!(
            jpeg_segment_markers(&bytes).contains(&0xE0),
            "expected APP0/JFIF: {:?}",
            jpeg_segment_markers(&bytes)
        );
        // No QR modules to decode.
        let img = image::load_from_memory(&bytes).unwrap().to_luma8();
        let mut prepared = rqrr::PreparedImage::prepare(img);
        let grids = prepared.detect_grids();
        assert!(grids.is_empty(), "wia_plain must not stamp a QR");
        // Smaller / simpler than QR-stamped wedges at Q95.
        let decorated = encode_pattern(
            "jfif",
            PatternId::Wedges,
            100,
            0,
            None,
            None,
            crate::effects::ColorMode::Rgb24,
            EncodeOpts::NO_COM,
        )
        .unwrap();
        assert!(
            bytes.len() < decorated.len(),
            "plain {} should be smaller than wedges+QR {}",
            bytes.len(),
            decorated.len()
        );
    }

    #[test]
    fn job_page_pixels_caps_300_to_match_encode() {
        let (full_w, full_h) = optical_page_pixels(300);
        assert!(full_w.saturating_mul(full_h) > MAX_PIXELS);
        let (jw, jh) = job_page_pixels(300);
        let (cw, ch) = capped_page_pixels(300);
        assert_eq!((jw, jh), (cw, ch));
        assert_eq!((jw, jh), optical_page_pixels(150));
        // Brother crop @100/@200 stays under the cap — ImageInformation == SOF.
        assert_eq!(job_page_pixels(100), (816, 1144));
        assert_eq!(job_page_pixels(200), (1648, 2289));
        // Platen / WIA extent remains full letter.
        assert_eq!(page_pixels(200), (1700, 2340));
    }

    #[test]
    fn wia_plain_pattern_parse_aliases() {
        assert_eq!(PatternId::parse("wia_plain").unwrap(), PatternId::WiaPlain);
        assert_eq!(PatternId::parse("plain").unwrap(), PatternId::WiaPlain);
        assert_eq!(PatternId::parse("wia-plain").unwrap(), PatternId::WiaPlain);
    }

    #[test]
    fn jfif_encode_matches_brother_structure() {
        // Brother MFC-9330CDW WSD JPEG: SOF0 4:2:0 (Y 2×2), APP0 units=dpi,
        // DRI = MCU columns, component ids 1/2/3. Prior image-crate encode was
        // 4:4:4 + aspect density 1×1 + no DRI (CancelJob after RetrieveImage).
        let dpi = std::env::var("FAKESCAN_DUMP_DPI")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(100u32);
        let bytes = encode_pattern(
            "jfif",
            PatternId::WiaPlain,
            dpi,
            0,
            None,
            None,
            crate::effects::ColorMode::Rgb24,
            EncodeOpts::WIA_PLAIN,
        )
        .unwrap();
        let comps = jpeg_sof0_components(&bytes);
        assert_eq!(
            comps,
            vec![(1, 2, 2), (2, 1, 1), (3, 1, 1)],
            "expected 4:2:0 with 1-based ids, got {comps:?}"
        );
        assert_eq!(
            jpeg_jfif_density(&bytes),
            Some((1, dpi as u16, dpi as u16)),
            "APP0 units=inches + dpi"
        );
        let (w, _) = job_page_pixels(dpi);
        let expect_dri = restart_interval_mcu_row(w);
        assert_eq!(jpeg_dri(&bytes), Some(expect_dri), "DRI = MCU row width");
        assert!(jpeg_is_baseline(&bytes));
        assert!(!jpeg_has_com(&bytes));
        let expect_dims = job_page_pixels(dpi);
        assert_eq!(
            jpeg_sof0_dimensions(&bytes),
            Some(expect_dims),
            "SOF must match ImageInformation optical crop job_page_pixels({dpi})"
        );
        // Brother MFC WIA JPEG: APP0 → DQT → DRI → SOF0 → DHT → SOS (packed tables).
        let markers = jpeg_segment_markers(&bytes);
        assert_eq!(
            markers,
            vec![0xD8, 0xE0, 0xDB, 0xDD, 0xC0, 0xC4, 0xDA],
            "Brother segment order/packing; got {markers:?}"
        );
        assert_eq!(
            markers.iter().filter(|&&m| m == 0xDB).count(),
            1,
            "one packed DQT"
        );
        assert_eq!(
            markers.iter().filter(|&&m| m == 0xC4).count(),
            1,
            "one packed DHT"
        );
        if let Ok(path) = std::env::var("FAKESCAN_DUMP_WIA") {
            std::fs::write(&path, &bytes).expect("dump wia jpeg");
        }
    }

    #[test]
    fn reshape_jpeg_packs_dqt_dht_and_reorders() {
        // jpeg-encoder emits split tables + SOF-before-tables; reshape packs/reorders.
        let w = 32u16;
        let h = 32u16;
        let rgb = vec![0x80u8; (w as usize) * (h as usize) * 3];
        let mut raw = Vec::new();
        {
            let mut enc = jpeg_encoder::Encoder::new(&mut raw, 80);
            enc.set_density(jpeg_encoder::PixelDensity::dpi(100));
            enc.set_sampling_factor(jpeg_encoder::SamplingFactor::F_2_2);
            enc.set_restart_interval(restart_interval_mcu_row(w as u32));
            enc.encode(&rgb, w, h, jpeg_encoder::ColorType::Rgb)
                .unwrap();
        }
        let before = jpeg_segment_markers(&raw);
        let dqt_before = before.iter().filter(|&&m| m == 0xDB).count();
        let dht_before = before.iter().filter(|&&m| m == 0xC4).count();
        eprintln!("reshape before markers={before:?} DQT={dqt_before} DHT={dht_before}");
        assert!(
            dqt_before >= 2
                || dht_before >= 2
                || before != vec![0xD8, 0xE0, 0xDB, 0xDD, 0xC0, 0xC4, 0xDA],
            "precondition: jpeg-encoder still split/unordered; got {before:?}"
        );
        let packed = reshape_jpeg_brother_layout(&raw).unwrap();
        let after = jpeg_segment_markers(&packed);
        eprintln!("reshape after  markers={after:?}");
        assert_eq!(
            after,
            vec![0xD8, 0xE0, 0xDB, 0xDD, 0xC0, 0xC4, 0xDA],
            "after reshape; before was {before:?}"
        );
        assert_eq!(after.iter().filter(|&&m| m == 0xDB).count(), 1);
        assert_eq!(after.iter().filter(|&&m| m == 0xC4).count(), 1);
        // Entropy + EOI preserved (length roughly unchanged).
        assert!(packed.len() >= raw.len().saturating_sub(32));
        assert_eq!(&packed[packed.len() - 2..], &[0xFF, 0xD9]);
    }

    // ── Diagnostic pattern tests ────────────────────────────────────────────

    #[test]
    fn info_and_anac_parse_aliases() {
        // Primary ids
        assert_eq!(PatternId::parse("info").unwrap(), PatternId::Info);
        assert_eq!(PatternId::parse("anac").unwrap(), PatternId::Anac);
        // Alias
        assert_eq!(PatternId::parse("whoami").unwrap(), PatternId::Anac);
        // Roundtrip via as_str
        assert_eq!(
            PatternId::parse(PatternId::Info.as_str()).unwrap(),
            PatternId::Info
        );
        assert_eq!(
            PatternId::parse(PatternId::Anac.as_str()).unwrap(),
            PatternId::Anac
        );
    }

    #[test]
    fn info_and_anac_in_all_slice() {
        assert!(PatternId::ALL.contains(&PatternId::Info));
        assert!(PatternId::ALL.contains(&PatternId::Anac));
    }

    #[test]
    fn render_pattern_rgb_info_non_blank_and_distinct() {
        let (info_img, _) = render_pattern_rgb(PatternId::Info, 50, 0, None).unwrap();
        let (anac_img, _) = render_pattern_rgb(PatternId::Anac, 50, 0, None).unwrap();
        let (page_img, _) = render_pattern_rgb(PatternId::PageId, 50, 0, None).unwrap();

        // Must be valid non-empty images.
        assert!(info_img.width() > 0 && info_img.height() > 0);
        assert!(anac_img.width() > 0 && anac_img.height() > 0);

        // Must differ from each other and from an unrelated pattern.
        assert_ne!(
            info_img.as_raw(),
            anac_img.as_raw(),
            "info vs anac should differ"
        );
        assert_ne!(
            info_img.as_raw(),
            page_img.as_raw(),
            "info should differ from page_id"
        );
        assert_ne!(
            anac_img.as_raw(),
            page_img.as_raw(),
            "anac should differ from page_id"
        );
    }

    #[test]
    fn render_pattern_rgb_info_with_ctx_non_blank() {
        let ctx = PageContext {
            profile_id: "test-profile".into(),
            device_uuid: "a1b2c3d4-0000-0000-0000-000000000000".into(),
            pattern_id: "info".into(),
            dpi: 50,
            format: "jfif".into(),
            job_id: 7,
            peer: Some("192.168.1.42:12345".into()),
            timestamp_secs: 1_760_000_000,
        };
        let (img, _) = render_pattern_rgb_ctx(
            PatternId::Info,
            50,
            0,
            None,
            EncodeOpts::default(),
            Some(&ctx),
        )
        .unwrap();
        assert!(img.width() > 0 && img.height() > 0);

        // Without context the image should differ (contains placeholder text).
        let (img_no_ctx, _) = render_pattern_rgb(PatternId::Info, 50, 0, None).unwrap();
        assert_ne!(
            img.as_raw(),
            img_no_ctx.as_raw(),
            "ctx vs no-ctx info should differ"
        );
    }

    #[test]
    fn render_pattern_rgb_anac_with_ctx_shows_peer() {
        let ctx = PageContext {
            profile_id: "test-profile".into(),
            device_uuid: "00000000-0000-0000-0000-000000000001".into(),
            pattern_id: "anac".into(),
            dpi: 50,
            format: "jfif".into(),
            job_id: 3,
            peer: Some("10.0.0.1:9999".into()),
            timestamp_secs: 1_760_000_001,
        };
        let (img, _) = render_pattern_rgb_ctx(
            PatternId::Anac,
            50,
            0,
            None,
            EncodeOpts::default(),
            Some(&ctx),
        )
        .unwrap();
        assert!(img.width() > 0 && img.height() > 0);

        // Different peer → different pixels.
        let ctx2 = PageContext {
            peer: Some("10.0.0.2:9999".into()),
            ..ctx.clone()
        };
        let (img2, _) = render_pattern_rgb_ctx(
            PatternId::Anac,
            50,
            0,
            None,
            EncodeOpts::default(),
            Some(&ctx2),
        )
        .unwrap();
        assert_ne!(
            img.as_raw(),
            img2.as_raw(),
            "different peer should produce different pixels"
        );
    }

    #[test]
    fn qr_contains_client_when_ctx_has_peer() {
        use rqrr::PreparedImage;

        let ctx = PageContext {
            profile_id: "test-profile".into(),
            device_uuid: "00000000-0000-0000-0000-000000000002".into(),
            pattern_id: "info".into(),
            dpi: 150,
            format: "jfif".into(),
            job_id: 1,
            peer: Some("192.168.99.1:55000".into()),
            timestamp_secs: 1_760_000_002,
        };
        let jpeg = encode_pattern_pages_ctx(
            "jfif",
            PatternId::Info,
            150,
            &[0],
            None,
            None,
            crate::effects::ColorMode::Rgb24,
            EncodeOpts::WITH_COM,
            Some(&ctx),
        )
        .unwrap();

        let img = image::load_from_memory(&jpeg).unwrap().to_luma8();
        let mut prepared = PreparedImage::prepare(img);
        let grids = prepared.detect_grids();
        assert!(!grids.is_empty(), "info pattern must stamp a QR");
        let (_meta, content) = grids[0].decode().expect("qr decode");
        assert!(
            content.contains("client=192.168.99.1:55000"),
            "QR payload must contain client= when peer is set; got: {content}"
        );
    }

    #[test]
    fn qr_no_client_when_no_peer() {
        use rqrr::PreparedImage;

        let jpeg = encode_pattern_pages_ctx(
            "jfif",
            PatternId::Info,
            150,
            &[0],
            None,
            None,
            crate::effects::ColorMode::Rgb24,
            EncodeOpts::WITH_COM,
            None, // no ctx → no client=
        )
        .unwrap();
        let img = image::load_from_memory(&jpeg).unwrap().to_luma8();
        let mut prepared = PreparedImage::prepare(img);
        let grids = prepared.detect_grids();
        assert!(
            !grids.is_empty(),
            "info pattern must stamp a QR even without ctx"
        );
        let (_meta, content) = grids[0].decode().expect("qr decode");
        assert!(
            !content.contains("client="),
            "QR payload must NOT contain client= when peer absent; got: {content}"
        );
    }
}
