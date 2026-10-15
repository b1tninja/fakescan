//! Layout v3 of the `diag` page: a parametric, size-independent generator.
//!
//! Nothing here is a stored fixture. A page (raster, layout JSON, vector primitives, QR payload) is a
//! pure function of a [`PageSpec`]: sheet size (paper name or width x height in mm, orientation),
//! margin, bleed, dpi, page / copy / total, duplex side, job tag. The same parameters always give the
//! same bytes; the QR payload carries the parameters so a decoder can regenerate the expected page.
//!
//! Architecture: [`build_layout`] places *blocks* (colour bars, ramps, patch grids, geometry, text,
//! rulers, ...) in rows inside the printable frame. Block boxes scale with the sheet (one factor `s`),
//! rows that cannot fit at the smallest factor are dropped by priority and recorded with
//! `drawn: false`. Fine detail (line periods, text point sizes, ruler pitch) stays physical. Every block
//! emits (a) [`Prim`] drawing primitives in millimetres, (b) layout elements with expected values and
//! ground-truth samples. [`rasterize`] paints the primitives, so the JSON and the pixels cannot drift
//! (tests sample every element at several sheet sizes and dpis).
//!
//! Coordinate system: millimetres, origin top-left of the sheet (the raster), x right, y down.
//! Pixel `i` covers `[i / k, (i + 1) / k)` mm with `k = dpi / 25.4`. Colours are 8-bit sRGB code
//! values, no ICC conversion happens here.

use crate::diag_fid as fid;
use crate::image_gen::{glyph_rows, PageSide};
use image::{Rgb, RgbImage};
use serde_json::{json, Value};

pub const LAYOUT_VERSION: u32 = 3;
/// Revision inside layout 3. Revision 1 adds dual-unit rulers, OpenCV-compatible ArUco markers, calibration
/// boards and corner brackets; revision 0 (the first v3 pages) carried the private 4x4 corner markers.
pub const LAYOUT_REVISION: u32 = 1;
pub const COLOUR_SPACE: &str = "sRGB";
pub const SCHEMA: &str = "fakescan.diag.layout/3";
pub const FRAME_RGB: [u8; 3] = [0, 90, 200];
pub const OUTLINE_RGB: [u8; 3] = [120, 120, 120];
const INK: [u8; 3] = [0, 0, 0];
const PAPER: [u8; 3] = [255, 255, 255];

pub const MIN_SCALE: f64 = 0.55;
pub const MAX_SCALE: f64 = 2.0;
const GAP_MIN: f64 = 1.5;
const GAP_MAX: f64 = 7.0;

/// EBU 100/0/75/0 bars: white 100 %, then 75 % yellow, cyan, green, magenta, red, blue, black.
pub const EBU_BARS: [[u8; 3]; 8] = [
    [255, 255, 255],
    [191, 191, 0],
    [0, 191, 191],
    [0, 191, 0],
    [191, 0, 191],
    [191, 0, 0],
    [0, 0, 191],
    [0, 0, 0],
];
/// SMPTE-style reverse-blue castellation strip.
pub const SMPTE_STRIP: [[u8; 3]; 7] = [
    [0, 0, 191],
    [19, 19, 19],
    [191, 0, 191],
    [19, 19, 19],
    [0, 191, 191],
    [19, 19, 19],
    [191, 191, 191],
];
/// ColorChecker Classic 24 (commonly published sRGB, row-major 6 x 4).
pub const CC24: [(&str, [u8; 3]); 24] = [
    ("dark_skin", [0x73, 0x52, 0x44]),
    ("light_skin", [0xC2, 0x96, 0x82]),
    ("blue_sky", [0x62, 0x7A, 0x9D]),
    ("foliage", [0x57, 0x6C, 0x43]),
    ("blue_flower", [0x85, 0x80, 0xB1]),
    ("bluish_green", [0x67, 0xBD, 0xAA]),
    ("orange", [0xD6, 0x7E, 0x2C]),
    ("purplish_blue", [0x50, 0x5B, 0xA6]),
    ("moderate_red", [0xC1, 0x5A, 0x63]),
    ("purple", [0x5E, 0x3C, 0x6C]),
    ("yellow_green", [0x9D, 0xBC, 0x40]),
    ("orange_yellow", [0xE0, 0xA3, 0x2E]),
    ("blue", [0x38, 0x3D, 0x96]),
    ("green", [0x46, 0x94, 0x49]),
    ("red", [0xAF, 0x36, 0x3C]),
    ("yellow", [0xE7, 0xC7, 0x1F]),
    ("magenta", [0xBB, 0x56, 0x95]),
    ("cyan", [0x08, 0x85, 0xA1]),
    ("white", [0xF3, 0xF3, 0xF3]),
    ("neutral_8", [0xC8, 0xC8, 0xC8]),
    ("neutral_6_5", [0xA0, 0xA0, 0xA0]),
    ("neutral_5", [0x7A, 0x7A, 0x7A]),
    ("neutral_3_5", [0x55, 0x55, 0x55]),
    ("black", [0x34, 0x34, 0x34]),
];
/// sRGB renderings of the full-tint CMYK process colours (declared values, not a profile conversion).
pub const CMYK_SOLIDS: [(&str, [u8; 3]); 4] = [
    ("cyan", [0, 174, 239]),
    ("magenta", [236, 0, 140]),
    ("yellow", [255, 242, 0]),
    ("black", [35, 31, 32]),
];
pub const RICH_BLACK: [u8; 3] = [10, 10, 10];
pub const FLAT_TONES_PCT: [u32; 5] = [10, 25, 50, 75, 90];
pub const NEAR_BLACK: [u8; 8] = [1, 2, 3, 4, 6, 8, 12, 16];
pub const NEAR_WHITE: [u8; 8] = [254, 253, 252, 251, 249, 247, 243, 239];
pub const RES_PERIODS_MM: [f64; 8] = [2.0, 1.0, 0.75, 0.5, 0.4, 0.3, 0.25, 0.2];
pub const LINE_PT: [f64; 7] = [0.25, 0.35, 0.5, 0.7, 1.0, 1.4, 2.0];
pub const HALFTONE_PCT: [u32; 5] = [10, 25, 50, 75, 90];
pub const HALFTONE_PITCH_MM: f64 = 0.4;
pub const ZONE_EDGE_CYC_PER_MM: f64 = 4.0;
pub const STAR_CYCLES: u32 = 36;
pub const ORDER_SLOTS: u32 = 16;
/// Orientation glyphs, per corner TL, TR, BL, BR. The back side mirrors each glyph and rotates the
/// assignment one corner step.
pub const GLYPHS_FRONT: [char; 4] = ['F', 'R', 'G', 'J'];
pub const GLYPHS_BACK: [char; 4] = ['J', 'G', 'R', 'F'];

const PAPERS: [(&str, f64, f64); 19] = [
    ("A0", 841.0, 1189.0),
    ("A1", 594.0, 841.0),
    ("A2", 420.0, 594.0),
    ("A3", 297.0, 420.0),
    ("A4", 210.0, 297.0),
    ("A5", 148.0, 210.0),
    ("A6", 105.0, 148.0),
    ("B4", 250.0, 353.0),
    ("B5", 176.0, 250.0),
    ("JIS-B4", 257.0, 364.0),
    ("JIS-B5", 182.0, 257.0),
    ("LETTER", 215.9, 279.4),
    ("LEGAL", 215.9, 355.6),
    ("TABLOID", 279.4, 431.8),
    ("LEDGER", 431.8, 279.4),
    ("EXECUTIVE", 184.2, 266.7),
    ("STATEMENT", 139.7, 215.9),
    ("FOLIO", 215.9, 330.2),
    ("A4-EXTRA", 235.0, 322.0),
];
/// Media outlines (portrait w x h mm) drawn dashed when they fit inside the sheet.
const OUTLINES: [(&str, f64, f64); 7] = [
    ("A6", 105.0, 148.0),
    ("A5", 148.0, 210.0),
    ("STATEMENT", 139.7, 215.9),
    ("A4", 210.0, 297.0),
    ("LETTER", 215.9, 279.4),
    ("LEGAL", 215.9, 355.6),
    ("A3", 297.0, 420.0),
];

/// Paper size in mm (portrait) by name, or `WxH` / `W x H` in mm.
pub fn paper_size_mm(name: &str) -> Option<(f64, f64)> {
    let n = name.trim().to_ascii_uppercase().replace(['_', ' '], "-");
    let n = match n.as_str() {
        "ISO-B4" => "B4".to_string(),
        "ISO-B5" => "B5".to_string(),
        "US-LETTER" => "LETTER".to_string(),
        "US-LEGAL" => "LEGAL".to_string(),
        "11X17" => "TABLOID".to_string(),
        _ => n,
    };
    if let Some(p) = PAPERS.iter().find(|p| p.0 == n) {
        return Some((p.1, p.2));
    }
    let lower = name.trim().to_ascii_lowercase().replace(' ', "");
    let (a, b) = lower.split_once('x')?;
    let (w, h) = (a.parse::<f64>().ok()?, b.parse::<f64>().ok()?);
    (w > 0.0 && h > 0.0).then_some((w, h))
}

pub fn paper_names() -> Vec<&'static str> {
    PAPERS.iter().map(|p| p.0).collect()
}

// ------------------------------------------------------------------------------------------------
// Page specification
// ------------------------------------------------------------------------------------------------

/// Which ruler scales are drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rulers {
    Metric,
    Inch,
    Both,
}

impl Rulers {
    pub fn as_str(self) -> &'static str {
        match self {
            Rulers::Metric => "metric",
            Rulers::Inch => "inch",
            Rulers::Both => "both",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "metric" | "mm" => Some(Rulers::Metric),
            "inch" | "in" | "imperial" => Some(Rulers::Inch),
            "both" | "" => Some(Rulers::Both),
            _ => None,
        }
    }
    pub fn metric(self) -> bool {
        self != Rulers::Inch
    }
    pub fn inch(self) -> bool {
        self != Rulers::Metric
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PageSpec {
    /// Paper name as given (or `custom`); informational, the pixel size below is authoritative.
    pub paper: String,
    pub width_px: u32,
    pub height_px: u32,
    pub dpi: u32,
    pub margin_mm: f64,
    pub bleed_mm: f64,
    pub page: u32,
    pub copy: u32,
    /// Total pages in the job (0 = unknown).
    pub total: u32,
    pub side: Option<PageSide>,
    pub job: String,
    /// Height of the tester banner stamped over the top by the caller, in pixels (0 = none).
    pub banner_px: u32,
    /// `rgb` or `gray` (gray replicates luma into all three channels).
    pub gray: bool,
    /// Ruler scales drawn: metric, inch or both (default).
    pub rulers: Rulers,
    /// Extra QR payload suffix (e.g. `client=ip`), appended after `|`.
    pub extra: Option<String>,
}

impl PageSpec {
    /// A sheet of `w_mm` x `h_mm` at `dpi` (pixel size = round(mm * dpi / 25.4)).
    pub fn from_mm(paper: &str, w_mm: f64, h_mm: f64, dpi: u32) -> Self {
        let k = dpi as f64 / 25.4;
        PageSpec::from_px(
            paper,
            (w_mm * k).round().max(1.0) as u32,
            (h_mm * k).round().max(1.0) as u32,
            dpi,
        )
    }

    pub fn from_px(paper: &str, w: u32, h: u32, dpi: u32) -> Self {
        PageSpec {
            paper: paper.to_string(),
            width_px: w.max(1),
            height_px: h.max(1),
            dpi: dpi.max(1),
            margin_mm: 4.0,
            bleed_mm: 0.0,
            page: 0,
            copy: 0,
            total: 0,
            side: None,
            job: String::new(),
            banner_px: 0,
            gray: false,
            rulers: Rulers::Both,
            extra: None,
        }
    }

    /// By paper name (or `WxH` mm) and orientation (`landscape` swaps to wide).
    pub fn from_paper(name: &str, landscape: bool, dpi: u32) -> Option<Self> {
        let (mut w, mut h) = paper_size_mm(name)?;
        if (landscape && h > w)
            || (!landscape && w > h && !name.to_ascii_uppercase().contains("LEDGER"))
        {
            std::mem::swap(&mut w, &mut h);
        }
        Some(PageSpec::from_mm(
            &name.trim().to_ascii_uppercase(),
            w,
            h,
            dpi,
        ))
    }

    pub fn k(&self) -> f64 {
        self.dpi as f64 / 25.4
    }
    pub fn width_mm(&self) -> f64 {
        self.width_px as f64 / self.k()
    }
    pub fn height_mm(&self) -> f64 {
        self.height_px as f64 / self.k()
    }

    /// Build from a JSON object (the Python binding). Unknown keys are ignored.
    pub fn from_json(v: &Value) -> anyhow::Result<Self> {
        let dpi = v.get("dpi").and_then(Value::as_u64).unwrap_or(100) as u32;
        let paper = v.get("paper").and_then(Value::as_str).unwrap_or("A4");
        let landscape = v
            .get("orientation")
            .and_then(Value::as_str)
            .map(|o| o.eq_ignore_ascii_case("landscape"))
            .unwrap_or(false);
        let mut s = if let (Some(w), Some(h)) = (
            v.get("width_px").and_then(Value::as_u64),
            v.get("height_px").and_then(Value::as_u64),
        ) {
            PageSpec::from_px(paper, w as u32, h as u32, dpi)
        } else if let (Some(w), Some(h)) = (
            v.get("width_mm").and_then(Value::as_f64),
            v.get("height_mm").and_then(Value::as_f64),
        ) {
            let (w, h) = if landscape == (w > h) { (w, h) } else { (h, w) };
            PageSpec::from_mm("custom", w, h, dpi)
        } else {
            PageSpec::from_paper(paper, landscape, dpi)
                .ok_or_else(|| anyhow::anyhow!("unknown paper {paper:?}"))?
        };
        let f = |k: &str, d: f64| v.get(k).and_then(Value::as_f64).unwrap_or(d);
        let u = |k: &str| v.get(k).and_then(Value::as_u64).unwrap_or(0) as u32;
        s.margin_mm = f("margin_mm", 4.0).max(0.0);
        s.bleed_mm = f("bleed_mm", 0.0).max(0.0);
        s.page = u("page");
        s.copy = u("copy");
        s.total = u("total");
        s.banner_px = u("banner_px");
        s.side = v
            .get("side")
            .and_then(Value::as_str)
            .and_then(|x| PageSide::parse(x).ok());
        s.job = sanitize_job(v.get("job").and_then(Value::as_str).unwrap_or(""));
        s.gray = v
            .get("colour")
            .or_else(|| v.get("color"))
            .and_then(Value::as_str)
            .map(|c| c.eq_ignore_ascii_case("gray") || c.eq_ignore_ascii_case("grey"))
            .unwrap_or(false);
        s.rulers = v
            .get("rulers")
            .and_then(Value::as_str)
            .and_then(Rulers::parse)
            .unwrap_or(Rulers::Both);
        s.extra = v.get("extra").and_then(Value::as_str).map(str::to_string);
        Ok(s)
    }

    /// QR payload: `fakescan|v1|page=0|pat=diag|side=front|lay=3|paper=A4|px=816x1144|dpi=100|m=4|b=0|copy=0|of=1|job=x`.
    pub fn qr_payload(&self) -> String {
        let side = self.side.map(|s| s.as_str()).unwrap_or("none");
        let mut p = format!(
            "{}|page={}|pat=diag|side={side}|lay={LAYOUT_VERSION}|paper={}|px={}x{}|dpi={}|m={}|b={}|copy={}|of={}|bn={}|g={}|rev={}|rl={}|ar={}|id={}",
            crate::image_gen::QR_SCHEME,
            self.page,
            self.paper.replace('|', "-"),
            self.width_px,
            self.height_px,
            self.dpi,
            fmt_num(self.margin_mm),
            fmt_num(self.bleed_mm),
            self.copy,
            self.total,
            self.banner_px,
            self.gray as u8,
            LAYOUT_REVISION,
            self.rulers.as_str(),
            "4x4_50",
            "0-19",
        );
        if !self.job.is_empty() {
            p.push_str(&format!("|job={}", self.job));
        }
        if let Some(e) = &self.extra {
            p.push('|');
            p.push_str(e);
        }
        p
    }

    /// Regenerate the spec from a v3 QR payload (inverse of [`PageSpec::qr_payload`]).
    pub fn from_qr_payload(text: &str) -> Option<Self> {
        let text = text.trim();
        if !text.starts_with(crate::image_gen::QR_SCHEME) {
            return None;
        }
        let mut s = PageSpec::from_px("custom", 1, 1, 100);
        let mut lay = 0u32;
        for part in text.split('|').skip(2) {
            let (k, v) = part.split_once('=')?;
            match k {
                "page" => s.page = v.parse().ok()?,
                "side" => s.side = PageSide::parse(v).ok(),
                "lay" => lay = v.parse().ok()?,
                "paper" => s.paper = v.to_string(),
                "px" => {
                    let (a, b) = v.split_once('x')?;
                    s.width_px = a.parse().ok()?;
                    s.height_px = b.parse().ok()?;
                }
                "dpi" => s.dpi = v.parse().ok()?,
                "m" => s.margin_mm = v.parse().ok()?,
                "b" => s.bleed_mm = v.parse().ok()?,
                "copy" => s.copy = v.parse().ok()?,
                "of" => s.total = v.parse().ok()?,
                "bn" => s.banner_px = v.parse().ok()?,
                "g" => s.gray = v == "1",
                // revision-1 keys; payloads without them are revision 0 and still parse
                "rl" => s.rulers = Rulers::parse(v).unwrap_or(Rulers::Both),
                "job" => s.job = v.to_string(),
                _ => {}
            }
        }
        (lay == LAYOUT_VERSION).then_some(s)
    }

    pub fn json(&self) -> Value {
        json!({
            "paper": self.paper, "width_px": self.width_px, "height_px": self.height_px, "dpi": self.dpi,
            "width_mm": self.width_mm(), "height_mm": self.height_mm(),
            "orientation": if self.width_px > self.height_px { "landscape" } else { "portrait" },
            "margin_mm": self.margin_mm, "bleed_mm": self.bleed_mm,
            "page": self.page, "copy": self.copy, "total": self.total,
            "side": self.side.map(|s| s.as_str()), "job": self.job,
            "banner_px": self.banner_px, "colour": if self.gray { "gray" } else { "rgb" },
            "rulers": self.rulers.as_str(),
        })
    }
}

fn fmt_num(v: f64) -> String {
    let s = format!("{v:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

pub fn sanitize_job(s: &str) -> String {
    s.chars()
        .filter_map(|c| {
            let c = c.to_ascii_uppercase();
            (c.is_ascii_alphanumeric() || c == '-' || c == '_').then_some(c)
        })
        .take(16)
        .collect()
}

/// Pixel height of the tester banner `image_gen::stamp_banner` draws for a `w` x `h` raster.
pub fn banner_height_px(w: u32, h: u32) -> u32 {
    if w < 80 || h < 40 {
        return 0;
    }
    let scale = (w.min(h) / 180).clamp(2, 6);
    7 * scale + 4 * scale
}

// ------------------------------------------------------------------------------------------------
// Primitives and rasteriser
// ------------------------------------------------------------------------------------------------

type C3 = [u8; 3];
type R4 = [f64; 4];

#[derive(Clone, Debug)]
pub enum Prim {
    Rect {
        r: R4,
        c: C3,
    },
    /// Non-overlapping rects painted as one union (no seams between touching modules); exported as
    /// one `rect` op each.
    Rects {
        rs: Vec<R4>,
        c: C3,
    },
    Poly {
        pts: Vec<[f64; 2]>,
        c: C3,
    },
    Line {
        a: [f64; 2],
        b: [f64; 2],
        w: f64,
        c: C3,
        dash: Option<[f64; 2]>,
    },
    Circle {
        c: [f64; 2],
        r: f64,
        fill: Option<C3>,
        stroke: Option<(C3, f64)>,
    },
    Text {
        x: f64,
        y: f64,
        text: String,
        pt: f64,
        c: C3,
        mirror: bool,
    },
    Ramp {
        r: R4,
        c0: C3,
        c1: C3,
        horizontal: bool,
    },
    Wheel {
        c: [f64; 2],
        r: f64,
    },
    Star {
        c: [f64; 2],
        r: f64,
        cycles: u32,
    },
    Zone {
        r: R4,
        f_edge: f64,
    },
    Halftone {
        r: R4,
        cov: f64,
        pitch: f64,
    },
    /// 50 % duty bars, ink first. `vertical`: lines run vertically (period along x).
    Grating {
        r: R4,
        period: f64,
        vertical: bool,
        c: C3,
    },
    Ticks {
        x: f64,
        y: f64,
        horizontal: bool,
        pitch: f64,
        count: u32,
        tiers: Vec<(u32, f64)>,
        w: f64,
        down: bool,
        c: C3,
    },
    Qr {
        r: R4,
        payload: String,
        module_px: u32,
    },
}

impl Prim {
    pub fn vector(&self) -> bool {
        !matches!(
            self,
            Prim::Ramp { .. }
                | Prim::Wheel { .. }
                | Prim::Star { .. }
                | Prim::Zone { .. }
                | Prim::Halftone { .. }
                | Prim::Qr { .. }
        )
    }

    /// JSON ops for this primitive (a `Rects` union expands into one `rect` op per member).
    fn json_all(&self, id: &str) -> Vec<Value> {
        match self {
            Prim::Rects { rs, c } => rs
                .iter()
                .map(|r| Prim::Rect { r: *r, c: *c }.json(id))
                .collect(),
            _ => vec![self.json(id)],
        }
    }

    fn json(&self, id: &str) -> Value {
        let mut v = match self {
            Prim::Rect { r, c } => json!({"op": "rect", "rect_mm": r, "rgb": c}),
            Prim::Rects { rs, c } => json!({"op": "rect", "rect_mm": rs.first(), "rgb": c}),
            Prim::Poly { pts, c } => json!({"op": "polygon", "points_mm": pts, "rgb": c}),
            Prim::Line { a, b, w, c, dash } => {
                json!({"op": "line", "from_mm": a, "to_mm": b, "width_mm": w, "rgb": c, "dash_mm": dash})
            }
            Prim::Circle { c, r, fill, stroke } => {
                json!({"op": "circle", "centre_mm": c, "radius_mm": r,
                "fill": fill, "stroke": stroke.map(|(col, w)| json!({"rgb": col, "width_mm": w}))})
            }
            Prim::Text {
                x,
                y,
                text,
                pt,
                c,
                mirror,
            } => json!({"op": "text", "x_mm": x, "y_mm": y, "text": text,
                "pt": pt, "rgb": c, "mirror": mirror, "font": "5x7 pixel font, y is the cap top, cell = 0.72*pt/7 em-mm"}),
            Prim::Ramp {
                r,
                c0,
                c1,
                horizontal,
            } => {
                json!({"op": "ramp", "rect_mm": r, "from_rgb": c0, "to_rgb": c1, "horizontal": horizontal})
            }
            Prim::Wheel { c, r } => json!({"op": "hue_wheel", "centre_mm": c, "radius_mm": r}),
            Prim::Star { c, r, cycles } => {
                json!({"op": "siemens_star", "centre_mm": c, "radius_mm": r, "cycles": cycles})
            }
            Prim::Zone { r, f_edge } => {
                json!({"op": "zone_plate", "rect_mm": r, "edge_cyc_per_mm": f_edge})
            }
            Prim::Halftone { r, cov, pitch } => {
                json!({"op": "halftone", "rect_mm": r, "coverage": cov, "pitch_mm": pitch, "screen_deg": 45})
            }
            Prim::Grating {
                r,
                period,
                vertical,
                c,
            } => {
                json!({"op": "grating", "rect_mm": r, "period_mm": period, "vertical_lines": vertical, "rgb": c})
            }
            Prim::Ticks {
                x,
                y,
                horizontal,
                pitch,
                count,
                tiers,
                w,
                down,
                c,
            } => json!({"op": "ticks", "origin_mm": [x, y],
                "horizontal": horizontal, "pitch_mm": pitch, "count": count, "tiers": tiers, "width_mm": w, "down": down, "rgb": c}),
            Prim::Qr {
                r,
                payload,
                module_px,
            } => json!({"op": "qr", "rect_mm": r, "payload": payload, "module_px": module_px}),
        };
        v["id"] = json!(id);
        v["vector"] = json!(self.vector());
        v
    }
}

pub fn ramp_value(c0: C3, c1: C3, t: f64) -> C3 {
    let t = t.clamp(0.0, 1.0);
    let f = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * t).round() as u8;
    [f(c0[0], c1[0]), f(c0[1], c1[1]), f(c0[2], c1[2])]
}

fn hsv_to_rgb(h_deg: f64, s: f64, v: f64) -> C3 {
    let h = h_deg.rem_euclid(360.0) / 60.0;
    let i = h.floor();
    let f = h - i;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    let (r, g, b) = match i as u32 % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    [
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8,
    ]
}

/// Hue colour at the wheel point (angle degrees, radius fraction): H = angle, S = fraction, V = 1.
pub fn wheel_rgb(angle_deg: f64, frac: f64) -> C3 {
    hsv_to_rgb(angle_deg, frac.clamp(0.0, 1.0), 1.0)
}

pub fn text_unit_mm(pt: f64) -> f64 {
    0.72 * pt * 25.4 / 72.0 / 7.0
}

/// (width, cap height) mm of `n` characters at `pt`.
pub fn text_size_mm(n: usize, pt: f64) -> (f64, f64) {
    let u = text_unit_mm(pt);
    ((n.max(1) * 6 - 1) as f64 * u, 7.0 * u)
}

/// Ten rows per glyph: 2 above the cap line, 7 body rows, 1 below (accents, cedilla).
pub fn glyph_ext(ch: char) -> [u8; 10] {
    let up = if ch == 'ß' {
        ch
    } else {
        ch.to_uppercase().next().unwrap_or(ch)
    };
    let (base, above, below): ([u8; 7], [u8; 2], u8) = match up {
        'Ä' => (glyph_rows('A'), [0b01010, 0], 0),
        'Ö' => (glyph_rows('O'), [0b01010, 0], 0),
        'Ü' => (glyph_rows('U'), [0b01010, 0], 0),
        'É' => (glyph_rows('E'), [0b00010, 0b00100], 0),
        'È' => (glyph_rows('E'), [0b01000, 0b00100], 0),
        'Ñ' => (glyph_rows('N'), [0b01101, 0b10110], 0),
        'Ç' => (glyph_rows('C'), [0, 0], 0b00100),
        'Å' => (glyph_rows('A'), [0b00100, 0b01010], 0),
        'Ø' => (
            [
                0b01110, 0b10011, 0b10101, 0b10101, 0b10101, 0b11001, 0b01110,
            ],
            [0, 0],
            0,
        ),
        'J' => (
            [
                0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100,
            ],
            [0, 0],
            0,
        ),
        'ß' => (
            [
                0b01100, 0b10010, 0b10100, 0b10100, 0b10010, 0b10001, 0b10110,
            ],
            [0, 0],
            0,
        ),
        other => (glyph_rows(other), [0, 0], 0),
    };
    let mut o = [0u8; 10];
    o[0] = above[0];
    o[1] = above[1];
    o[2..9].copy_from_slice(&base);
    o[9] = below;
    o
}

/// Ink cells of a text run as rects (mm), honouring `mirror`.
pub fn text_cells_mm(x: f64, y: f64, text: &str, pt: f64, mirror: bool) -> Vec<R4> {
    let u = text_unit_mm(pt);
    let n = text.chars().count();
    let total = (n.max(1) * 6 - 1) as f64;
    let mut out = vec![];
    for (i, ch) in text.chars().enumerate() {
        for (row, bits) in glyph_ext(ch).iter().enumerate() {
            for col in 0..5usize {
                if bits & (1 << (4 - col)) != 0 {
                    let c = (i * 6 + col) as f64;
                    let cx = if mirror {
                        x + (total - 1.0 - c) * u
                    } else {
                        x + c * u
                    };
                    let cy = y + (row as f64 - 2.0) * u;
                    out.push([cx, cy, cx + u, cy + u]);
                }
            }
        }
    }
    out
}

/// Analytic ink fraction of the 5x7 body (excluding accents) of `text`.
pub fn text_ink_fraction(text: &str) -> f64 {
    let mut ink = 0usize;
    for ch in text.chars() {
        for r in &glyph_ext(ch)[2..9] {
            ink += (r & 0x1f).count_ones() as usize;
        }
    }
    ink as f64 / ((text.chars().count().max(1) * 6 - 1) * 7) as f64
}

struct Canvas<'a> {
    img: &'a mut RgbImage,
    k: f64,
}

impl Canvas<'_> {
    fn blend(&mut self, x: u32, y: u32, c: C3, a: f64) {
        if a <= 0.0 || x >= self.img.width() || y >= self.img.height() {
            return;
        }
        if a >= 0.999_999 {
            self.img.put_pixel(x, y, Rgb(c));
            return;
        }
        let o = self.img.get_pixel(x, y).0;
        let m = |i: usize| (o[i] as f64 * (1.0 - a) + c[i] as f64 * a).round() as u8;
        self.img.put_pixel(x, y, Rgb([m(0), m(1), m(2)]));
    }

    fn bbox_px(&self, r: R4) -> (u32, u32, u32, u32) {
        let (w, h) = (self.img.width() as f64, self.img.height() as f64);
        let x0 = (r[0] * self.k).floor().clamp(0.0, w) as u32;
        let y0 = (r[1] * self.k).floor().clamp(0.0, h) as u32;
        let x1 = (r[2] * self.k).ceil().clamp(0.0, w) as u32;
        let y1 = (r[3] * self.k).ceil().clamp(0.0, h) as u32;
        (x0, y0, x1, y1)
    }

    /// Union of non-overlapping rects with exact area coverage, composited once.
    fn rects(&mut self, rects: &[R4], c: C3) {
        if rects.is_empty() {
            return;
        }
        let mut u = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
        for r in rects {
            u = [
                u[0].min(r[0]),
                u[1].min(r[1]),
                u[2].max(r[2]),
                u[3].max(r[3]),
            ];
        }
        let (bx0, by0, bx1, by1) = self.bbox_px(u);
        if bx1 <= bx0 || by1 <= by0 {
            return;
        }
        let bw = (bx1 - bx0) as usize;
        let mut cov = vec![0f64; bw * (by1 - by0) as usize];
        let k = self.k;
        for r in rects {
            let (x0, y0, x1, y1) = self.bbox_px(*r);
            for py in y0.max(by0)..y1.min(by1) {
                let oy = ((py as f64 + 1.0) / k).min(r[3]) - (py as f64 / k).max(r[1]);
                if oy <= 0.0 {
                    continue;
                }
                let fy = oy * k;
                for px in x0.max(bx0)..x1.min(bx1) {
                    let ox = ((px as f64 + 1.0) / k).min(r[2]) - (px as f64 / k).max(r[0]);
                    if ox > 0.0 {
                        cov[(py - by0) as usize * bw + (px - bx0) as usize] += ox * k * fy;
                    }
                }
            }
        }
        for py in by0..by1 {
            for px in bx0..bx1 {
                let a = cov[(py - by0) as usize * bw + (px - bx0) as usize].min(1.0);
                self.blend(px, py, c, a);
            }
        }
    }

    fn poly(&mut self, pts: &[[f64; 2]], c: C3) {
        let mut u = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
        for p in pts {
            u = [
                u[0].min(p[0]),
                u[1].min(p[1]),
                u[2].max(p[0]),
                u[3].max(p[1]),
            ];
        }
        let (x0, y0, x1, y1) = self.bbox_px(u);
        const SS: u32 = 4;
        for py in y0..y1 {
            for px in x0..x1 {
                let mut hit = 0;
                for sy in 0..SS {
                    for sx in 0..SS {
                        let x = (px as f64 + (sx as f64 + 0.5) / SS as f64) / self.k;
                        let y = (py as f64 + (sy as f64 + 0.5) / SS as f64) / self.k;
                        if point_in_poly(pts, x, y) {
                            hit += 1;
                        }
                    }
                }
                self.blend(px, py, c, hit as f64 / (SS * SS) as f64);
            }
        }
    }

    fn circle(&mut self, c: [f64; 2], r: f64, fill: Option<C3>, stroke: Option<(C3, f64)>) {
        let ext = r + stroke.map(|s| s.1).unwrap_or(0.0) + 1.0 / self.k;
        let (x0, y0, x1, y1) = self.bbox_px([c[0] - ext, c[1] - ext, c[0] + ext, c[1] + ext]);
        for py in y0..y1 {
            for px in x0..x1 {
                let d = (((px as f64 + 0.5) / self.k - c[0]).powi(2)
                    + ((py as f64 + 0.5) / self.k - c[1]).powi(2))
                .sqrt()
                    * self.k;
                if let Some(f) = fill {
                    self.blend(px, py, f, (r * self.k - d + 0.5).clamp(0.0, 1.0));
                }
                if let Some((sc, sw)) = stroke {
                    let half = sw * self.k / 2.0;
                    let a = (half - (d - r * self.k).abs() + 0.5).clamp(0.0, 1.0);
                    self.blend(px, py, sc, a);
                }
            }
        }
    }

    fn line(&mut self, a: [f64; 2], b: [f64; 2], w: f64, c: C3, dash: Option<[f64; 2]>) {
        let len = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        if len <= 0.0 {
            return;
        }
        let (dx, dy) = ((b[0] - a[0]) / len, (b[1] - a[1]) / len);
        let segs: Vec<(f64, f64)> = match dash {
            None => vec![(0.0, len)],
            Some([on, off]) => {
                let mut v = vec![];
                let mut t = 0.0;
                while t < len {
                    v.push((t, (t + on).min(len)));
                    t += on + off;
                }
                v
            }
        };
        let axis = dx.abs() > 0.999_999 || dy.abs() > 0.999_999;
        if axis {
            let rects: Vec<R4> = segs
                .iter()
                .map(|&(s, e)| {
                    let (p, q) = (
                        [a[0] + dx * s, a[1] + dy * s],
                        [a[0] + dx * e, a[1] + dy * e],
                    );
                    [
                        p[0].min(q[0]) - dy.abs() * w / 2.0,
                        p[1].min(q[1]) - dx.abs() * w / 2.0,
                        p[0].max(q[0]) + dy.abs() * w / 2.0,
                        p[1].max(q[1]) + dx.abs() * w / 2.0,
                    ]
                })
                .collect();
            self.rects(&rects, c);
        } else {
            for (s, e) in segs {
                let (nx, ny) = (-dy * w / 2.0, dx * w / 2.0);
                let (p, q) = (
                    [a[0] + dx * s, a[1] + dy * s],
                    [a[0] + dx * e, a[1] + dy * e],
                );
                self.poly(
                    &[
                        [p[0] + nx, p[1] + ny],
                        [q[0] + nx, q[1] + ny],
                        [q[0] - nx, q[1] - ny],
                        [p[0] - nx, p[1] - ny],
                    ],
                    c,
                );
            }
        }
    }

    fn per_pixel(&mut self, r: R4, mut f: impl FnMut(f64, f64) -> Option<(C3, f64)>) {
        let (x0, y0, x1, y1) = self.bbox_px(r);
        for py in y0..y1 {
            for px in x0..x1 {
                let (x, y) = ((px as f64 + 0.5) / self.k, (py as f64 + 0.5) / self.k);
                if x < r[0] || x >= r[2] || y < r[1] || y >= r[3] {
                    continue;
                }
                if let Some((c, a)) = f(x, y) {
                    self.blend(px, py, c, a);
                }
            }
        }
    }

    fn paint(&mut self, p: &Prim) {
        match p {
            Prim::Rect { r, c } => self.rects(&[*r], *c),
            Prim::Rects { rs, c } => self.rects(rs, *c),
            Prim::Poly { pts, c } => self.poly(pts, *c),
            Prim::Line { a, b, w, c, dash } => self.line(*a, *b, *w, *c, *dash),
            Prim::Circle { c, r, fill, stroke } => self.circle(*c, *r, *fill, *stroke),
            Prim::Text {
                x,
                y,
                text,
                pt,
                c,
                mirror,
            } => {
                let cells = text_cells_mm(*x, *y, text, *pt, *mirror);
                self.rects(&cells, *c);
            }
            Prim::Ramp {
                r,
                c0,
                c1,
                horizontal,
            } => {
                let (c0, c1, hz, r) = (*c0, *c1, *horizontal, *r);
                self.per_pixel(r, |x, y| {
                    let t = if hz {
                        (x - r[0]) / (r[2] - r[0])
                    } else {
                        (y - r[1]) / (r[3] - r[1])
                    };
                    Some((ramp_value(c0, c1, t), 1.0))
                });
            }
            Prim::Wheel { c, r } => {
                let (c, rad) = (*c, *r);
                let k = self.k;
                self.per_pixel(
                    [
                        c[0] - rad - 1.0,
                        c[1] - rad - 1.0,
                        c[0] + rad + 1.0,
                        c[1] + rad + 1.0,
                    ],
                    |x, y| {
                        let (dx, dy) = (x - c[0], y - c[1]);
                        let d = (dx * dx + dy * dy).sqrt();
                        let a = ((rad - d) * k + 0.5).clamp(0.0, 1.0);
                        (a > 0.0).then(|| (wheel_rgb(dy.atan2(dx).to_degrees(), d / rad), a))
                    },
                );
            }
            Prim::Star { c, r, cycles } => {
                let (c, rad, n) = (*c, *r, *cycles as f64);
                let k = self.k;
                self.per_pixel(
                    [
                        c[0] - rad - 1.0,
                        c[1] - rad - 1.0,
                        c[0] + rad + 1.0,
                        c[1] + rad + 1.0,
                    ],
                    |x, y| {
                        let mut hit = 0;
                        for sy in 0..4 {
                            for sx in 0..4 {
                                let (px, py) = (
                                    x + (sx as f64 - 1.5) / (4.0 * k),
                                    y + (sy as f64 - 1.5) / (4.0 * k),
                                );
                                let (dx, dy) = (px - c[0], py - c[1]);
                                if (dx * dx + dy * dy).sqrt() <= rad && star_ink(dy.atan2(dx), n) {
                                    hit += 1;
                                }
                            }
                        }
                        (hit > 0).then(|| (INK, hit as f64 / 16.0))
                    },
                );
            }
            Prim::Zone { r, f_edge } => {
                let (r, fe) = (*r, *f_edge);
                let half = (r[2] - r[0]) / 2.0;
                let (cx, cy) = ((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0);
                self.per_pixel(r, |x, y| {
                    let g = zone_value(((x - cx).powi(2) + (y - cy).powi(2)).sqrt(), half, fe);
                    Some(([g, g, g], 1.0))
                });
            }
            Prim::Halftone { r, cov, pitch } => {
                let (r, cov, pitch) = (*r, *cov, *pitch);
                let k = self.k;
                self.per_pixel(r, |x, y| {
                    let mut hit = 0;
                    for sy in 0..4 {
                        for sx in 0..4 {
                            let (px, py) = (
                                x + (sx as f64 - 1.5) / (4.0 * k),
                                y + (sy as f64 - 1.5) / (4.0 * k),
                            );
                            if halftone_ink(px, py, cov, pitch) {
                                hit += 1;
                            }
                        }
                    }
                    Some((INK, hit as f64 / 16.0))
                });
            }
            Prim::Grating {
                r,
                period,
                vertical,
                c,
            } => {
                let mut rects = vec![];
                let (a0, a1) = if *vertical {
                    (r[0], r[2])
                } else {
                    (r[1], r[3])
                };
                let mut s = a0;
                while s < a1 - 1e-9 {
                    let e = (s + period / 2.0).min(a1);
                    rects.push(if *vertical {
                        [s, r[1], e, r[3]]
                    } else {
                        [r[0], s, r[2], e]
                    });
                    s += period;
                }
                self.rects(&rects, *c);
            }
            Prim::Ticks {
                x,
                y,
                horizontal,
                pitch,
                count,
                tiers,
                w,
                down,
                c,
            } => {
                let mut rects = vec![];
                for i in 0..=*count {
                    let len = tiers
                        .iter()
                        .rev()
                        .find(|(every, _)| i % every == 0)
                        .map(|t| t.1)
                        .unwrap_or(0.0);
                    if len <= 0.0 {
                        continue;
                    }
                    let p = i as f64 * pitch;
                    let rect = if *horizontal {
                        let (y0, y1) = if *down {
                            (*y, *y + len)
                        } else {
                            (*y - len, *y)
                        };
                        [x + p - w / 2.0, y0, x + p + w / 2.0, y1]
                    } else {
                        let (x0, x1) = if *down {
                            (*x, *x + len)
                        } else {
                            (*x - len, *x)
                        };
                        [x0, y + p - w / 2.0, x1, y + p + w / 2.0]
                    };
                    rects.push(rect);
                }
                self.rects(&rects, *c);
            }
            Prim::Qr {
                r,
                payload,
                module_px,
            } => {
                if let Ok(code) = qrcode::QrCode::with_error_correction_level(
                    payload.as_bytes(),
                    qrcode::EcLevel::M,
                ) {
                    let n = code.width() as u32;
                    let colors = code.to_colors();
                    let (x0, y0) = (
                        (r[0] * self.k).round() as u32,
                        (r[1] * self.k).round() as u32,
                    );
                    let tot = (n + 8) * module_px;
                    for y in 0..tot {
                        for x in 0..tot {
                            let (mx, my) = (x / module_px, y / module_px);
                            let dark = mx >= 4
                                && my >= 4
                                && mx < n + 4
                                && my < n + 4
                                && colors[((my - 4) * n + (mx - 4)) as usize]
                                    == qrcode::Color::Dark;
                            if x0 + x < self.img.width() && y0 + y < self.img.height() {
                                self.img.put_pixel(
                                    x0 + x,
                                    y0 + y,
                                    Rgb(if dark { INK } else { PAPER }),
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

fn point_in_poly(pts: &[[f64; 2]], x: f64, y: f64) -> bool {
    let mut inside = false;
    let mut j = pts.len() - 1;
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[j]);
        if (a[1] > y) != (b[1] > y) && x < (b[0] - a[0]) * (y - a[1]) / (b[1] - a[1]) + a[0] {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Siemens-star ink at angle `a` (radians, atan2 image coordinates): even wedges are ink.
pub fn star_ink(a: f64, cycles: f64) -> bool {
    let w = std::f64::consts::PI / cycles;
    ((a.rem_euclid(2.0 * std::f64::consts::PI) / w).floor() as i64) % 2 == 0
}

/// Zone plate grey level (255 at the centre) at radius `r` mm for half-width `half` mm and an
/// edge frequency of `f_edge` cycles/mm at r = half.
pub fn zone_value(r: f64, half: f64, f_edge: f64) -> u8 {
    let n = f_edge * half;
    let phase = std::f64::consts::PI * n * (r / half).powi(2);
    (255.0 * (0.5 + 0.5 * phase.cos())).round() as u8
}

/// 45 degree dot screen: ink where the point lies inside a dot of area fraction `cov` (inverted
/// dots, offset half a cell, above 50 %).
pub fn halftone_ink(x: f64, y: f64, cov: f64, pitch: f64) -> bool {
    let (u, v) = (
        (x + y) / std::f64::consts::SQRT_2 / pitch,
        (y - x) / std::f64::consts::SQRT_2 / pitch,
    );
    let (cov, off) = if cov <= 0.5 {
        (cov, 0.0)
    } else {
        (1.0 - cov, 0.5)
    };
    let (fu, fv) = (
        (u + off).rem_euclid(1.0) - 0.5,
        (v + off).rem_euclid(1.0) - 0.5,
    );
    let inside = (fu * fu + fv * fv).sqrt() <= (cov / std::f64::consts::PI).sqrt();
    if cov == 0.0 {
        return false;
    }
    if off == 0.0 {
        inside
    } else {
        !inside
    }
}

/// Paint all primitives of a layout, in order, on a white sheet.
pub fn rasterize(layout: &Layout) -> RgbImage {
    let spec = &layout.spec;
    let mut img = RgbImage::from_pixel(spec.width_px, spec.height_px, Rgb(PAPER));
    {
        let mut cv = Canvas {
            img: &mut img,
            k: spec.k(),
        };
        for (_, p) in &layout.prims {
            cv.paint(p);
        }
    }
    if spec.gray {
        for px in img.pixels_mut() {
            let l = ((299 * px.0[0] as u32 + 587 * px.0[1] as u32 + 114 * px.0[2] as u32 + 500)
                / 1000) as u8;
            px.0 = [l, l, l];
        }
    }
    img
}

// ------------------------------------------------------------------------------------------------
// Layout model
// ------------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct El {
    pub id: String,
    pub kind: &'static str,
    pub group: &'static str,
    pub rect: R4,
    pub drawn: bool,
    pub scale: f64,
    pub expected: Value,
    pub samples: Vec<Value>,
    pub reason: Option<String>,
}

impl El {
    fn json(&self, gray: bool) -> Value {
        let samples: Vec<Value> = self
            .samples
            .iter()
            .map(|s| {
                if !gray {
                    return s.clone();
                }
                let mut s = s.clone();
                let rgb: Vec<u64> = s["rgb"]
                    .as_array()
                    .map(|a| a.iter().filter_map(Value::as_u64).collect())
                    .unwrap_or_default();
                if rgb.len() == 3 {
                    let l = ((299 * rgb[0] + 587 * rgb[1] + 114 * rgb[2] + 500) / 1000) as u8;
                    s["rgb"] = json!([l, l, l]);
                    s["tol"] = json!(s["tol"].as_u64().unwrap_or(4) + 2);
                }
                s
            })
            .collect();
        let mut v = json!({
            "id": self.id, "kind": self.kind, "group": self.group,
            "rect_mm": self.rect, "size_mm": [self.rect[2] - self.rect[0], self.rect[3] - self.rect[1]],
            "since_layout": 3, "drawn": self.drawn, "scale": self.scale,
            "expected": self.expected, "samples": samples,
        });
        if let Some(r) = &self.reason {
            v["reason"] = json!(r);
        }
        v
    }
}

#[derive(Clone, Debug)]
pub struct Layout {
    pub spec: PageSpec,
    pub els: Vec<El>,
    pub prims: Vec<(String, Prim)>,
    pub qr_payload: String,
    pub scale: f64,
    pub frame: R4,
    pub content: R4,
    pub notes: Vec<String>,
}

impl Layout {
    pub fn json(&self) -> Value {
        let s = &self.spec;
        let k = s.k();
        json!({
            "schema": SCHEMA,
            "layout_version": LAYOUT_VERSION,
            "layout_revision": LAYOUT_REVISION,
            "colour_space": COLOUR_SPACE,
            "colour_note": "8-bit sRGB code values; no ICC conversion happens in the generator",
            "spec": s.json(),
            "origin": "top-left of the sheet (= the raster), x right, y down, millimetres; pixel i covers [i/k, (i+1)/k)",
            "box_mm": [s.width_mm(), s.height_mm()],
            "raster_px": [s.width_px, s.height_px], "raster_mm": [s.width_mm(), s.height_mm()],
            "dpi_effective": s.dpi, "px_per_mm": k,
            "banner_reserve_mm": s.banner_px as f64 / k,
            "frame_mm": self.frame, "content_mm": self.content, "scale": self.scale,
            "bleed_box_mm": [-s.bleed_mm, -s.bleed_mm, s.width_mm() + s.bleed_mm, s.height_mm() + s.bleed_mm],
            "qr_payload": self.qr_payload,
            "qr_payload_layout_field": format!("lay={LAYOUT_VERSION}"),
            "notes": self.notes,
            "elements": self.els.iter().map(|e| e.json(s.gray)).collect::<Vec<_>>(),
            "primitives": self.prims.iter().flat_map(|(id, p)| p.json_all(id)).collect::<Vec<_>>(),
        })
    }
}

// ------------------------------------------------------------------------------------------------
// Builder and blocks
// ------------------------------------------------------------------------------------------------

struct B {
    k: f64,
    w: f64,
    h: f64,
    gray: bool,
    prims: Vec<(String, Prim)>,
    els: Vec<El>,
    spec: PageSpec,
}

type DrawFn = fn(&mut B, R4, f64);

struct Blk {
    id: &'static str,
    w: f64,
    h: f64,
    fill: bool,
    scale: bool,
    draw: DrawFn,
    optional: bool,
}

struct Row {
    name: &'static str,
    prio: u32,
    blocks: Vec<Blk>,
}

impl B {
    fn add(&mut self, id: &str, p: Prim) {
        self.prims.push((id.to_string(), p));
    }
    fn rect(&mut self, id: &str, r: R4, c: C3) {
        self.add(id, Prim::Rect { r, c });
    }
    /// 4 thin rects outlining `r` (line drawn inside).
    fn outline(&mut self, id: &str, r: R4, t: f64, c: C3) {
        for q in [
            [r[0], r[1], r[2], r[1] + t],
            [r[0], r[3] - t, r[2], r[3]],
            [r[0], r[1], r[0] + t, r[3]],
            [r[2] - t, r[1], r[2], r[3]],
        ] {
            self.rect(id, q, c);
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn text(&mut self, id: &str, x: f64, y: f64, text: &str, pt: f64, c: C3, mirror: bool) {
        self.add(
            id,
            Prim::Text {
                x,
                y,
                text: text.to_string(),
                pt,
                c,
                mirror,
            },
        );
    }
    fn el(
        &mut self,
        id: &str,
        kind: &'static str,
        group: &'static str,
        rect: R4,
        sc: f64,
        expected: Value,
    ) -> usize {
        self.els.push(El {
            id: id.to_string(),
            kind,
            group,
            rect,
            drawn: true,
            scale: sc,
            expected,
            samples: vec![],
            reason: None,
        });
        self.els.len() - 1
    }
    fn sample(&mut self, e: usize, x: f64, y: f64, rgb: C3, tol: u8) {
        self.els[e]
            .samples
            .push(json!({"x_mm": x, "y_mm": y, "rgb": rgb, "tol": tol}));
    }
    /// Pixel-centre x (mm) of the pixel containing `x`.
    fn snap(&self, x: f64) -> f64 {
        ((x * self.k).floor() + 0.5) / self.k
    }
    fn ink_tol(&self, w_mm: f64) -> u8 {
        if w_mm * self.k >= 2.0 {
            60
        } else {
            200
        }
    }
}

// ---- colour bars ----------------------------------------------------------------------------
fn blk_smpte(b: &mut B, r: R4, sc: f64) {
    let h = r[3] - r[1];
    let (h1, h2) = (h * 0.62, h * 0.30);
    let w = r[2] - r[0];
    let bars = [r[0], r[1], r[2], r[1] + h1];
    let strip = [r[0], r[3] - h2, r[2], r[3]];
    let e = b.el("ebu_bars", "colour_bars", "colour", bars, sc, json!({
        "standard": "EBU 100/0/75/0", "rgb": EBU_BARS, "bar_width_mm": w / 8.0,
        "meaning": "channel order, saturation clipping, black level; white = 255, 75 % bars = 191"}));
    for (i, c) in EBU_BARS.iter().enumerate() {
        let x0 = r[0] + w * i as f64 / 8.0;
        b.rect("ebu_bars", [x0, bars[1], x0 + w / 8.0 + 0.001, bars[3]], *c);
        b.sample(e, x0 + w / 16.0, (bars[1] + bars[3]) / 2.0, *c, 4);
    }
    let e2 = b.el(
        "smpte_castellation",
        "colour_bars",
        "colour",
        strip,
        sc,
        json!({
        "standard": "SMPTE reverse-blue strip", "rgb": SMPTE_STRIP, "cells": 7}),
    );
    for (i, c) in SMPTE_STRIP.iter().enumerate() {
        let x0 = r[0] + w * i as f64 / 7.0;
        b.rect(
            "smpte_castellation",
            [x0, strip[1], x0 + w / 7.0 + 0.001, strip[3]],
            *c,
        );
        b.sample(e2, x0 + w / 14.0, (strip[1] + strip[3]) / 2.0, *c, 4);
    }
    b.outline("ebu_bars_outline", bars, 0.25, INK);
    b.outline("smpte_outline", strip, 0.25, INK);
}

// ---- ramps ----------------------------------------------------------------------------------
fn ramps(
    b: &mut B,
    r: R4,
    sc: f64,
    items: &[(&str, C3, C3)],
    group: &'static str,
    kind_note: &str,
) {
    let n = items.len() as f64;
    let gap = 0.8;
    let rh = ((r[3] - r[1]) - gap * (n - 1.0)) / n;
    for (i, (id, c0, c1)) in items.iter().enumerate() {
        let y0 = r[1] + i as f64 * (rh + gap);
        let rr = [r[0], y0, r[2], y0 + rh];
        let e = b.el(
            id,
            "ramp",
            group,
            rr,
            sc,
            json!({"from_rgb": c0, "to_rgb": c1, "levels": 256, "note": kind_note,
            "value": "round(from + (to - from) * t), t = (pixel centre x - x0) / width"}),
        );
        b.add(
            id,
            Prim::Ramp {
                r: rr,
                c0: *c0,
                c1: *c1,
                horizontal: true,
            },
        );
        for t in [0.1, 0.5, 0.9] {
            let x = b.snap(rr[0] + t * (rr[2] - rr[0]));
            let tt = (x - rr[0]) / (rr[2] - rr[0]);
            b.sample(e, x, (rr[1] + rr[3]) / 2.0, ramp_value(*c0, *c1, tt), 3);
        }
    }
}
fn blk_ramps_rgbn(b: &mut B, r: R4, sc: f64) {
    ramps(
        b,
        r,
        sc,
        &[
            ("ramp_r", INK, [255, 0, 0]),
            ("ramp_g", INK, [0, 255, 0]),
            ("ramp_b", INK, [0, 0, 255]),
            ("ramp_n", INK, [255, 255, 255]),
        ],
        "colour",
        "additive primaries and neutral, black to full",
    );
}
fn blk_ramps_cmyk(b: &mut B, r: R4, sc: f64) {
    ramps(
        b,
        r,
        sc,
        &[
            ("ramp_c", PAPER, CMYK_SOLIDS[0].1),
            ("ramp_m", PAPER, CMYK_SOLIDS[1].1),
            ("ramp_y", PAPER, CMYK_SOLIDS[2].1),
            ("ramp_k", PAPER, CMYK_SOLIDS[3].1),
        ],
        "colour",
        "CMYK-ish separation ramps rendered as sRGB, paper white to the declared full-tint value",
    );
}

// ---- steps / clip / rich black ----------------------------------------------------------------
fn blk_steps(b: &mut B, r: R4, sc: f64) {
    let (cells, gaps) = (8 + 8 + 2 + 2, 3.0);
    let gw = 1.5;
    let cw = ((r[2] - r[0]) - gw * gaps) / cells as f64;
    let (y0, y1) = (r[1], r[3]);
    let mut x = r[0];
    let group = |b: &mut B,
                 id: &str,
                 kind: &'static str,
                 vals: &[C3],
                 names: &[&str],
                 x: &mut f64,
                 outline: bool| {
        let rr = [*x, y0, *x + cw * vals.len() as f64, y1];
        let e = b.el(
            id,
            kind,
            "tone",
            rr,
            sc,
            json!({"rgb": vals, "names": names}),
        );
        for (i, c) in vals.iter().enumerate() {
            let cell = [*x + cw * i as f64, y0, *x + cw * (i + 1) as f64, y1];
            b.rect(id, cell, *c);
            b.sample(e, (cell[0] + cell[2]) / 2.0, (y0 + y1) / 2.0, *c, 2);
        }
        if outline {
            b.outline(&format!("{id}_outline"), rr, 0.25, INK);
        }
        *x = rr[2] + gw;
    };
    let nb: Vec<C3> = NEAR_BLACK.iter().map(|&v| [v, v, v]).collect();
    let nw: Vec<C3> = NEAR_WHITE.iter().map(|&v| [v, v, v]).collect();
    group(b, "near_black", "patches", &nb, &[], &mut x, false);
    group(b, "near_white", "patches", &nw, &[], &mut x, true);
    group(
        b,
        "clip",
        "patches",
        &[INK, PAPER],
        &["clip_black", "clip_white"],
        &mut x,
        true,
    );
    group(
        b,
        "rich_black",
        "patches",
        &[RICH_BLACK, CMYK_SOLIDS[3].1],
        &["rich_black", "k100"],
        &mut x,
        false,
    );
}

// ---- flat fields --------------------------------------------------------------------------------
fn blk_flat(b: &mut B, r: R4, sc: f64) {
    let gw = 1.5;
    let w = ((r[2] - r[0]) - gw * 4.0) / 5.0;
    for (i, pct) in FLAT_TONES_PCT.iter().enumerate() {
        let v = (255.0 * (1.0 - *pct as f64 / 100.0)).round() as u8;
        let rr = [
            r[0] + i as f64 * (w + gw),
            r[1],
            r[0] + i as f64 * (w + gw) + w,
            r[3],
        ];
        let id = format!("flat_{pct}");
        let e = b.el(&id, "flat_field", "uniformity", rr, sc, json!({"ink_pct": pct, "rgb": [v, v, v],
            "meaning": "banding / streaking / mottle: row and column profiles of the interior must be flat"}));
        b.rect(&id, rr, [v, v, v]);
        for (fx, fy) in [
            (0.5, 0.5),
            (0.2, 0.25),
            (0.8, 0.25),
            (0.2, 0.75),
            (0.8, 0.75),
        ] {
            b.sample(
                e,
                rr[0] + fx * (rr[2] - rr[0]),
                rr[1] + fy * (rr[3] - rr[1]),
                [v, v, v],
                2,
            );
        }
        b.outline(&format!("{id}_outline"), rr, 0.2, [160, 160, 160]);
    }
}

// ---- ColorChecker -----------------------------------------------------------------------------
fn blk_cc24(b: &mut B, r: R4, sc: f64) {
    let (p, g) = (7.5 * sc, 0.8 * sc);
    let e = b.el("colorchecker24", "patch_grid", "colour", r, sc, json!({
        "columns": 6, "rows": 4, "order": "row-major, patch 1 top-left", "patch_mm": p, "gap_mm": g,
        "patches": CC24.iter().map(|(n, c)| json!({"name": n, "rgb": c})).collect::<Vec<_>>(),
        "source": "commonly published sRGB values for the ColorChecker Classic; the generator declares them, no spectral claim"}));
    for (i, (_, c)) in CC24.iter().enumerate() {
        let (col, row) = ((i % 6) as f64, (i / 6) as f64);
        let rr = [
            r[0] + col * (p + g),
            r[1] + row * (p + g),
            r[0] + col * (p + g) + p,
            r[1] + row * (p + g) + p,
        ];
        b.rect("colorchecker24", rr, *c);
        b.sample(e, (rr[0] + rr[2]) / 2.0, (rr[1] + rr[3]) / 2.0, *c, 2);
    }
}

// ---- wheel / star / slants --------------------------------------------------------------------
fn blk_wheel(b: &mut B, r: R4, sc: f64) {
    let rad = (r[2] - r[0]).min(r[3] - r[1]) / 2.0;
    let c = [(r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0];
    let e = b.el("hue_wheel", "hue_wheel", "colour", [c[0] - rad, c[1] - rad, c[0] + rad, c[1] + rad], sc, json!({
        "centre_mm": c, "radius_mm": rad, "hue": "angle from +x clockwise on screen, V = 1, S = r / R"}));
    b.add("hue_wheel", Prim::Wheel { c, r: rad });
    for deg in [0.0f64, 60.0, 120.0, 180.0, 240.0, 300.0] {
        let (x, y) = (
            c[0] + 0.7 * rad * deg.to_radians().cos(),
            c[1] + 0.7 * rad * deg.to_radians().sin(),
        );
        b.sample(e, x, y, wheel_rgb(deg, 0.7), 24);
    }
}
fn blk_star(b: &mut B, r: R4, sc: f64) {
    let rad = (r[2] - r[0]).min(r[3] - r[1]) / 2.0;
    let c = [(r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0];
    let e = b.el(
        "siemens_star",
        "siemens_star",
        "resolution",
        [c[0] - rad, c[1] - rad, c[0] + rad, c[1] + rad],
        sc,
        json!({
        "centre_mm": c, "radius_mm": rad, "cycles": STAR_CYCLES, "ink_first_wedge_at_angle_deg": 0,
        "cutoff": "radius where wedges blur to grey: lp/mm = cycles / (pi * r)"}),
    );
    b.add(
        "siemens_star",
        Prim::Star {
            c,
            r: rad,
            cycles: STAR_CYCLES,
        },
    );
    let w = std::f64::consts::PI / STAR_CYCLES as f64;
    for i in 0..4 {
        let a = (i as f64 + 0.5) * w;
        let ink = star_ink(a, STAR_CYCLES as f64);
        let (x, y) = (c[0] + 0.85 * rad * a.cos(), c[1] + 0.85 * rad * a.sin());
        b.sample(e, x, y, if ink { INK } else { PAPER }, 90);
    }
}
fn blk_slants(b: &mut B, r: R4, sc: f64) {
    let side = ((r[2] - r[0]).min((r[3] - r[1]) / 2.0)) * 0.78;
    let cell_h = (r[3] - r[1]) / 2.0;
    for (i, (id, deg)) in [("slant_edge_a", 5.0f64), ("slant_edge_b", -5.0)]
        .iter()
        .enumerate()
    {
        let c = [(r[0] + r[2]) / 2.0, r[1] + cell_h * (i as f64 + 0.5)];
        let (s, co) = deg.to_radians().sin_cos();
        let hs = side / 2.0;
        let pts: Vec<[f64; 2]> = [(-hs, -hs), (hs, -hs), (hs, hs), (-hs, hs)]
            .iter()
            .map(|(x, y)| [c[0] + x * co - y * s, c[1] + x * s + y * co])
            .collect();
        let e = b.el(id, "slant_edge", "resolution", [c[0] - hs, c[1] - hs, c[0] + hs, c[1] + hs], sc, json!({
            "side_mm": side, "angle_deg": deg, "dark_rgb": [40, 40, 40], "note": "edge angle and 10-90 % rise (ISO 12233 style)", "corners_mm": pts}));
        b.add(
            id,
            Prim::Poly {
                pts,
                c: [40, 40, 40],
            },
        );
        b.sample(e, c[0], c[1], [40, 40, 40], 4);
    }
}

// ---- resolution / grid / ladders ----------------------------------------------------------------
fn blk_res(b: &mut B, r: R4, _sc: f64) {
    let p = 4.0;
    for (row, vertical) in [(0usize, true), (1usize, false)] {
        let id = if vertical {
            "res_lines_v"
        } else {
            "res_lines_h"
        };
        let y0 = r[1] + row as f64 * (p + 0.8);
        let rr = [r[0], y0, r[0] + p * 8.0, y0 + p];
        let e = b.el(id, "line_pairs", "resolution", rr, 1.0, json!({
            "periods_mm": RES_PERIODS_MM, "patch_mm": p, "duty": 0.5, "ink_first": true,
            "orientation": if vertical { "vertical lines (period along x)" } else { "horizontal lines (period along y)" }}));
        for (i, per) in RES_PERIODS_MM.iter().enumerate() {
            let pr = [r[0] + i as f64 * p, y0, r[0] + (i + 1) as f64 * p, y0 + p];
            b.add(
                id,
                Prim::Grating {
                    r: pr,
                    period: *per,
                    vertical,
                    c: INK,
                },
            );
            if *per >= 1.0 {
                let (x, y) = if vertical {
                    (pr[0] + per / 4.0, (pr[1] + pr[3]) / 2.0)
                } else {
                    ((pr[0] + pr[2]) / 2.0, pr[1] + per / 4.0)
                };
                b.sample(e, x, y, INK, 90);
            }
        }
    }
}
fn blk_grid(b: &mut B, r: R4, _sc: f64) {
    let (w, h) = (r[2] - r[0], r[3] - r[1]);
    let e = b.el(
        "fine_grid",
        "grid",
        "geometry",
        r,
        1.0,
        json!({"pitch_mm": 1.0, "major_every": 5, "minor_mm": 0.15, "major_mm": 0.3}),
    );
    let _ = e;
    for i in 0..=(w.floor() as u32) {
        let x = r[0] + i as f64;
        let t = if i % 5 == 0 { 0.3 } else { 0.15 };
        b.add(
            "fine_grid",
            Prim::Line {
                a: [x, r[1]],
                b: [x, r[1] + h.floor()],
                w: t,
                c: INK,
                dash: None,
            },
        );
    }
    for j in 0..=(h.floor() as u32) {
        let y = r[1] + j as f64;
        let t = if j % 5 == 0 { 0.3 } else { 0.15 };
        b.add(
            "fine_grid",
            Prim::Line {
                a: [r[0], y],
                b: [r[0] + w.floor(), y],
                w: t,
                c: INK,
                dash: None,
            },
        );
    }
}
fn blk_ladder(b: &mut B, r: R4, _sc: f64, negative: bool) {
    let pitch = (r[3] - r[1]) / LINE_PT.len() as f64;
    let id = if negative {
        "line_ladder_neg"
    } else {
        "line_ladder_pos"
    };
    let widths: Vec<f64> = LINE_PT.iter().map(|p| p * 25.4 / 72.0).collect();
    let e = b.el(id, "line_ladder", "resolution", r, 1.0, json!({
        "widths_pt": LINE_PT, "widths_mm": widths, "polarity": if negative { "reverse (paper lines on ink)" } else { "positive (ink lines on paper)" },
        "pitch_mm": pitch, "length_mm": r[2] - r[0]}));
    if negative {
        b.rect(id, r, INK);
    }
    for (i, w) in widths.iter().enumerate() {
        let yc = r[1] + pitch * (i as f64 + 0.5);
        let rr = [r[0] + 1.0, yc - w / 2.0, r[2] - 1.0, yc + w / 2.0];
        b.rect(id, rr, if negative { PAPER } else { INK });
        let t = b.ink_tol(*w);
        let thick = *w * b.k >= 2.0;
        if negative {
            b.sample(
                e,
                (r[0] + r[2]) / 2.0,
                yc,
                PAPER,
                if thick { 60 } else { 250 },
            );
        } else {
            // a sub-pixel line may straddle a pixel boundary, so only wide lines are checked tightly
            b.sample(e, (r[0] + r[2]) / 2.0, yc, INK, if thick { t } else { 250 });
        }
    }
}
fn blk_ladder_pos(b: &mut B, r: R4, sc: f64) {
    blk_ladder(b, r, sc, false)
}
fn blk_ladder_neg(b: &mut B, r: R4, sc: f64) {
    blk_ladder(b, r, sc, true)
}
fn blk_halftone(b: &mut B, r: R4, sc: f64) {
    let cw = (r[2] - r[0]) / 5.0;
    let hh = (r[3] - r[1]) / 2.0 - 0.3;
    let e = b.el("halftone_ladder", "halftone", "tone", r, sc, json!({
        "coverage_pct": HALFTONE_PCT, "pitch_mm": HALFTONE_PITCH_MM, "screen_deg": 45,
        "top_row": "45 degree dot screen at the stated area coverage", "bottom_row": "solid grey with the same mean (255 * (1 - coverage))",
        "meaning": "mean of the screen cell must equal the solid cell after any resampling; moire shows as a mean or pattern shift"}));
    for (i, pct) in HALFTONE_PCT.iter().enumerate() {
        let x0 = r[0] + i as f64 * cw;
        b.add(
            "halftone_ladder",
            Prim::Halftone {
                r: [x0, r[1], x0 + cw, r[1] + hh],
                cov: *pct as f64 / 100.0,
                pitch: HALFTONE_PITCH_MM,
            },
        );
        let v = (255.0 * (1.0 - *pct as f64 / 100.0)).round() as u8;
        b.rect("halftone_ladder", [x0, r[3] - hh, x0 + cw, r[3]], [v, v, v]);
        b.sample(e, x0 + cw / 2.0, r[3] - hh / 2.0, [v, v, v], 2);
    }
}

// ---- zone plate / geometry / text on colour -------------------------------------------------------
fn blk_zone(b: &mut B, r: R4, sc: f64) {
    let side = (r[2] - r[0]).min(r[3] - r[1]);
    let rr = [r[0], r[1], r[0] + side, r[1] + side];
    let e = b.el("zone_plate", "zone_plate", "resolution", rr, sc, json!({
        "edge_cyc_per_mm": ZONE_EDGE_CYC_PER_MM, "half_width_mm": side / 2.0, "value": "255 * (0.5 + 0.5 * cos(pi * N * (r / half)^2)), N = f_edge * half",
        "meaning": "resampling kernel, aliasing and moire: the ring pattern must keep its radial frequency law"}));
    b.add(
        "zone_plate",
        Prim::Zone {
            r: rr,
            f_edge: ZONE_EDGE_CYC_PER_MM,
        },
    );
    let c = [(rr[0] + rr[2]) / 2.0, (rr[1] + rr[3]) / 2.0];
    b.sample(
        e,
        b.snap(c[0]),
        (((c[1] * b.k).floor()) + 0.5) / b.k,
        [zone_value(0.0, side / 2.0, ZONE_EDGE_CYC_PER_MM); 3],
        12,
    );
}
fn blk_geometry(b: &mut B, r: R4, sc: f64) {
    let u = sc;
    let y = (r[1] + r[3]) / 2.0;
    let radii: Vec<f64> = [2.0, 4.0, 6.0, 8.0].iter().map(|v| v * u).collect();
    let c1 = [r[0] + 8.0 * u, y];
    b.el("concentric_circles", "circles", "geometry", [c1[0] - 8.0 * u, y - 8.0 * u, c1[0] + 8.0 * u, y + 8.0 * u], sc,
        json!({"centre_mm": c1, "radii_mm": radii, "line_mm": 0.3, "meaning": "ellipse fit gives x/y anisotropy and shear"}));
    for rad in &radii {
        b.add(
            "concentric_circles",
            Prim::Circle {
                c: c1,
                r: *rad,
                fill: None,
                stroke: Some((INK, 0.3)),
            },
        );
    }
    let sq = [c1[0] + 12.0 * u, y - 8.0 * u, c1[0] + 28.0 * u, y + 8.0 * u];
    let c2 = [(sq[0] + sq[2]) / 2.0, y];
    b.el("circle_in_square", "square_circle", "geometry", sq, sc, json!({"square_mm": 16.0 * u, "circle_radius_mm": 8.0 * u, "diagonals": true, "line_mm": 0.3}));
    b.outline("circle_in_square", sq, 0.3, INK);
    b.add(
        "circle_in_square",
        Prim::Circle {
            c: c2,
            r: 8.0 * u - 0.15,
            fill: None,
            stroke: Some((INK, 0.3)),
        },
    );
    b.add(
        "circle_in_square",
        Prim::Line {
            a: [sq[0], sq[1]],
            b: [sq[2], sq[3]],
            w: 0.3,
            c: INK,
            dash: None,
        },
    );
    b.add(
        "circle_in_square",
        Prim::Line {
            a: [sq[2], sq[1]],
            b: [sq[0], sq[3]],
            w: 0.3,
            c: INK,
            dash: None,
        },
    );
    let hr = 7.5 * u;
    let c3 = [sq[2] + 4.0 * u + hr, y];
    let verts: Vec<[f64; 2]> = (0..6)
        .map(|i| {
            let a = (60.0 * i as f64).to_radians();
            [c3[0] + hr * a.cos(), c3[1] + hr * a.sin()]
        })
        .collect();
    b.el(
        "hexagon",
        "hexagon",
        "geometry",
        [c3[0] - hr, c3[1] - hr, c3[0] + hr, c3[1] + hr],
        sc,
        json!({"centre_mm": c3, "circumradius_mm": hr, "vertices_mm": verts, "line_mm": 0.3}),
    );
    for i in 0..6 {
        b.add(
            "hexagon",
            Prim::Line {
                a: verts[i],
                b: verts[(i + 1) % 6],
                w: 0.3,
                c: INK,
                dash: None,
            },
        );
    }
}
fn blk_toc(b: &mut B, r: R4, _sc: f64) {
    let rows: [(&str, C3, C3); 4] = [
        ("yellow", [255, 255, 0], INK),
        ("cyan", [0, 255, 255], INK),
        ("red", [255, 0, 0], PAPER),
        ("blue", [0, 0, 255], PAPER),
    ];
    let rh = (r[3] - r[1]) / 4.0;
    let e = b.el("text_on_colour", "text_on_colour", "text", r, 1.0, json!({
        "text": "LEGIBLE 0123 ABC", "pt": 6, "rows": rows.iter().map(|(n, bg, fg)| json!({"name": n, "bg": bg, "fg": fg})).collect::<Vec<_>>()}));
    for (i, (_, bg, fg)) in rows.iter().enumerate() {
        let rr = [r[0], r[1] + i as f64 * rh, r[2], r[1] + (i + 1) as f64 * rh];
        b.rect("text_on_colour", rr, *bg);
        let (_, th) = text_size_mm(16, 6.0);
        b.text(
            "text_on_colour",
            rr[0] + 1.0,
            rr[1] + (rh - th) / 2.0,
            "LEGIBLE 0123 ABC",
            6.0,
            *fg,
            false,
        );
        b.sample(e, rr[2] - 0.4, rr[1] + rh / 2.0, *bg, 2);
    }
}

// ---- QR ---------------------------------------------------------------------------------------------
fn blk_qr(b: &mut B, r: R4, sc: f64) {
    let payload = b.spec.qr_payload();
    let Ok(code) =
        qrcode::QrCode::with_error_correction_level(payload.as_bytes(), qrcode::EcLevel::M)
    else {
        let e = b.el("qr", "qr", "metadata", r, sc, json!({"payload": payload}));
        b.els[e].drawn = false;
        b.els[e].reason = Some("payload too long".into());
        return;
    };
    let n = code.width() as u32 + 8;
    let target = ((r[2] - r[0]).min(r[3] - r[1]) * b.k).floor() as u32;
    let mp = (target / n).max(1);
    let size = (mp * n) as f64 / b.k;
    let (x0, y0) = ((r[0] * b.k).round() / b.k, (r[1] * b.k).round() / b.k);
    let rr = [x0, y0, x0 + size, y0 + size];
    b.el("qr", "qr", "metadata", rr, sc, json!({
        "payload": payload, "module_px": mp, "modules": n - 8, "quiet_modules": 4, "ec": "M",
        "meaning": "page / copy / side / job / sheet size / dpi / layout version; regenerate the page from it"}));
    b.add(
        "qr",
        Prim::Qr {
            r: rr,
            payload,
            module_px: mp,
        },
    );
}

// ---- text ----------------------------------------------------------------------------------------------
fn blk_text_ladder(b: &mut B, r: R4, _sc: f64) {
    let lines: [(f64, &str); 4] = [
        (12.0, "12PT SCAN TEST 0123"),
        (10.0, "10PT SCAN TEST 0123 ABC"),
        (8.0, "8PT SCAN TEST 0123 ABCXYZ"),
        (6.0, "6PT SCAN TEST 0123 ABCXYZ 4567"),
    ];
    let mut y = r[1];
    for (pt, text) in lines {
        let (w, h) = text_size_mm(text.chars().count(), pt);
        let id = format!("text_{}pt", pt as u32);
        let e = b.el(&id, "text", "text", [r[0], y, r[0] + w, y + h], 1.0, json!({
            "pt": pt, "text": text, "ink_fraction": text_ink_fraction(text), "cap_height_mm": h, "cell_mm": text_unit_mm(pt)}));
        let _ = e;
        b.text(&id, r[0], y, text, pt, INK, false);
        y += h + 1.3;
    }
}
fn blk_pangram(b: &mut B, r: R4, _sc: f64) {
    let pangram = "THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG";
    let latin = "ÄÖÜ ÉÑ ÇÅØ ß 0123456789";
    let rev = "REVERSE TEXT 6PT  ABC xyz";
    let (_, h) = text_size_mm(1, 6.0);
    let mut y = r[1];
    let (w1, _) = text_size_mm(pangram.chars().count(), 6.0);
    b.el(
        "pangram",
        "text",
        "text",
        [r[0], y, r[0] + w1, y + h],
        1.0,
        json!({"pt": 6, "text": pangram, "ink_fraction": text_ink_fraction(pangram)}),
    );
    b.text("pangram", r[0], y, pangram, 6.0, INK, false);
    y += h + 1.6;
    let (w2, _) = text_size_mm(latin.chars().count(), 6.0);
    b.el("latin_sample", "text", "text", [r[0], y - 2.0 * text_unit_mm(6.0), r[0] + w2, y + h + text_unit_mm(6.0)], 1.0, json!({
        "pt": 6, "text": latin, "note": "Latin-1 accents composed on the 5x7 pixel font (no TrueType font is embedded)"}));
    b.text("latin_sample", r[0], y, latin, 6.0, INK, false);
    y += h + 2.4;
    let (w3, _) = text_size_mm(rev.chars().count(), 6.0);
    let bar = [r[0], y - 0.8, r[0] + w3 + 1.6, y + h + 0.8];
    let e = b.el(
        "reverse_text",
        "text_on_colour",
        "text",
        bar,
        1.0,
        json!({"pt": 6, "text": rev, "fg": PAPER, "bg": INK}),
    );
    b.rect("reverse_text", bar, INK);
    b.text("reverse_text", r[0] + 0.8, y, rev, 6.0, PAPER, false);
    b.sample(e, bar[2] - 0.3, (bar[1] + bar[3]) / 2.0, INK, 4);
}

// ---- rulers ---------------------------------------------------------------------------------------------
// ---- fiducial geometry constants ----------------------------------------------------------------------
/// Width of the left / right fiducial columns (brackets, vertical ruler, edge-midpoint marker).
pub const STRIP_COL_MM: f64 = 15.0;
/// Thickness of a vertical ruler strip inside its column.
pub const VRULER_W_MM: f64 = 13.0;
/// Height of the horizontal ruler strip.
pub const HRULER_H_MM: f64 = 11.0;
/// Module size of the left / right edge-midpoint ArUco markers.
pub const SIDE_MARKER_MODULE_MM: f64 = 1.4;
/// The fiducial columns are drawn when the content box is at least this big (mm).
pub const STRIP_MIN_CONTENT_MM: [f64; 2] = [120.0, 150.0];
pub const METRIC_TIERS: [(u32, f64); 3] = [(1, 1.5), (5, 2.5), (10, 3.5)];
pub const INCH_TIERS: [(u32, f64); 5] = [(1, 1.2), (2, 1.8), (4, 2.4), (8, 3.0), (16, 3.6)];
pub const TICK_W_MM: f64 = 0.3;
pub const RULER_LABEL_PT: f64 = 5.0;
/// ArUco ids: corners clockwise from the top-left, then edge midpoints clockwise from the top.
pub const ARUCO_ID_TL: usize = 0;
pub const ARUCO_ID_TR: usize = 1;
pub const ARUCO_ID_BR: usize = 2;
pub const ARUCO_ID_BL: usize = 3;
pub const ARUCO_ID_TM: usize = 4;
pub const ARUCO_ID_MR: usize = 5;
pub const ARUCO_ID_BM: usize = 6;
pub const ARUCO_ID_ML: usize = 7;
/// ChArUco board: first marker id.
pub const CHARUCO_FIRST_ID: usize = 8;
/// 9 x 6 squares = 8 x 5 inner corners: an even x odd pattern has no 180 degree ambiguity for OpenCV.
pub const CHESS_SQUARES: [usize; 2] = [9, 6];
pub const CHARUCO_SQUARES: [usize; 2] = [6, 4];
/// Asymmetric circle grid: points per row, rows.
pub const CIRCLES_GRID: [usize; 2] = [6, 5];
/// Board scale at layout scale 1: chessboard square, ChArUco square, circle row pitch (mm).
pub const CHESS_SQ_MM: f64 = 3.5;
pub const CHARUCO_SQ_MM: f64 = 5.5;
pub const CHARUCO_MARKER_RATIO: f64 = 0.7;
pub const CIRCLES_SP_MM: f64 = 3.0;
pub const CIRCLES_DIAMETER_RATIO: f64 = 0.55;

fn dropped_el(b: &mut B, id: &str, kind: &'static str, group: &'static str, reason: &str) {
    let e = b.el(id, kind, group, [0.0, 0.0, 0.0, 0.0], 1.0, json!({}));
    b.els[e].drawn = false;
    b.els[e].reason = Some(reason.to_string());
}

/// Draw ArUco marker `aid` of `DICT_4X4_50` into the square `r` as element `el`.
fn draw_aruco(b: &mut B, el: &str, aid: usize, r: R4, pos: &str) {
    let side = r[2] - r[0];
    let cell = side / 6.0;
    let e = b.el(
        el,
        "marker",
        "fiducial",
        r,
        1.0,
        json!({
        "dictionary": fid::ARUCO_DICT_NAME, "marker_id": aid, "position": pos,
        "modules": 6, "payload_bits": 4, "module_mm": cell, "cell_mm": cell, "side_mm": side,
        "quiet_zone_mm": cell, "quiet_modules": 1, "min_module_mm": fid::MIN_MODULE_MM,
        "corners_mm": fid::corners_of(r),
        "corner_order": "TL, TR, BR, BL of the marker in its canonical (upright) orientation, the order cv2.aruco returns",
        "cells": fid::aruco_rows(aid), "cells_note": "6x6 modules including the black border, 1 = black",
        "payload_hex": format!("0x{:04X}", fid::ARUCO_4X4_50[aid]),
        "opencv": "cv2.aruco.getPredefinedDictionary(cv2.aruco.DICT_4X4_50)",
        "orientation": "the code is not rotation symmetric: the detected corner order gives the page rotation"}),
    );
    b.add(
        el,
        Prim::Rects {
            rs: fid::aruco_rects(aid, r),
            c: INK,
        },
    );
    let cells = fid::aruco_cells(aid);
    if cell * b.k >= 3.0 {
        for (ri, row) in cells.iter().enumerate() {
            for (ci, &black) in row.iter().enumerate() {
                b.sample(
                    e,
                    r[0] + (ci as f64 + 0.5) * cell,
                    r[1] + (ri as f64 + 0.5) * cell,
                    if black { INK } else { PAPER },
                    // paper modules may sit under a dashed grey media outline (value 120)
                    if black { 90 } else { 140 },
                );
            }
        }
    } else {
        b.sample(e, r[0] + cell * 0.5, r[1] + cell * 0.5, INK, 190);
    }
}

/// Edge-midpoint marker in a band: returns `(end of the left flow region, start of the right region)`.
#[allow(clippy::too_many_arguments)]
fn band_mid_marker(
    b: &mut B,
    r: R4,
    yc: f64,
    g: &ClusterGeom,
    x0: f64,
    x1: f64,
    el: &str,
    aid: usize,
    pos: &str,
) -> (f64, f64) {
    let q = g.m / 6.0;
    let cx = (r[0] + r[2]) / 2.0;
    if x1 - x0 >= g.m + 2.0 * q + 8.0 {
        draw_aruco(
            b,
            el,
            aid,
            [
                cx - g.m / 2.0,
                yc - g.m / 2.0,
                cx + g.m / 2.0,
                yc + g.m / 2.0,
            ],
            pos,
        );
        (cx - g.m / 2.0 - q - 0.5, cx + g.m / 2.0 + q + 0.5)
    } else {
        dropped_el(b, el, "marker", "fiducial", "no room in the band");
        (x1, x1)
    }
}

// ---- rulers ------------------------------------------------------------------------------------------------

/// One ruler scale. `a0` = zero line position along the axis, `edge` = cross coordinate of the edge the
/// ticks start from, `d` = +1 / -1 (ticks point towards +cross / -cross).
struct RulerSpec {
    id: &'static str,
    inch: bool,
    horizontal: bool,
    a0: f64,
    edge: f64,
    d: f64,
    len: f64,
    /// strip rect (both scales share it)
    strip: R4,
    /// along-axis window left free for a marker (absolute coordinate)
    mask: Option<[f64; 2]>,
}

fn ruler_r4(horizontal: bool, a0: f64, a1: f64, c0: f64, c1: f64) -> R4 {
    let (a0, a1) = (a0.min(a1), a0.max(a1));
    let (c0, c1) = (c0.min(c1), c0.max(c1));
    if horizontal {
        [a0, c0, a1, c1]
    } else {
        [c0, a0, c1, a1]
    }
}

/// Draw one ruler scale (ticks, labels, element with the per-unit tick table).
fn draw_ruler(b: &mut B, sp: &RulerSpec) {
    let (pitch, tiers): (f64, &[(u32, f64)]) = if sp.inch {
        (25.4 / 16.0, &INCH_TIERS)
    } else {
        (1.0, &METRIC_TIERS)
    };
    let count = (sp.len / pitch + 1e-9).floor() as u32;
    let table = fid::ruler_ticks(pitch, count, tiers);
    let masked = |a: f64| {
        sp.mask
            .is_some_and(|m| a > m[0] - TICK_W_MM && a < m[1] + TICK_W_MM)
    };
    let lab_every = if sp.inch { 25.4 } else { 10.0 };
    let max_len = tiers.last().map(|t| t.1).unwrap_or(0.0);
    let id = sp.id;
    // ticks
    if sp.horizontal && sp.mask.is_none() {
        b.add(
            id,
            Prim::Ticks {
                x: sp.a0,
                y: sp.edge,
                horizontal: true,
                pitch,
                count,
                tiers: tiers.to_vec(),
                w: TICK_W_MM,
                down: sp.d > 0.0,
                c: INK,
            },
        );
    } else {
        let rs: Vec<R4> = table
            .iter()
            .filter(|(_, pos, _)| !masked(sp.a0 + pos))
            .map(|(_, pos, len)| {
                ruler_r4(
                    sp.horizontal,
                    sp.a0 + pos - TICK_W_MM / 2.0,
                    sp.a0 + pos + TICK_W_MM / 2.0,
                    sp.edge,
                    sp.edge + sp.d * len,
                )
            })
            .collect();
        b.add(id, Prim::Rects { rs, c: INK });
    }
    // labels
    let mut labels: Vec<Value> = vec![];
    let mut i = 0u32;
    loop {
        let pos = i as f64 * lab_every;
        if pos > sp.len + 1e-9 {
            break;
        }
        let txt = i.to_string();
        let (tw, th) = text_size_mm(txt.len(), RULER_LABEL_PT);
        let a = sp.a0 + pos;
        let (x, y) = if sp.horizontal {
            let ctop = if sp.d > 0.0 {
                sp.edge + max_len + 0.45
            } else {
                sp.edge - max_len - 0.45 - th
            };
            (a + 0.4, ctop)
        } else {
            let cleft = if sp.d > 0.0 {
                sp.edge + max_len + 0.45
            } else {
                sp.edge - max_len - 0.45 - tw
            };
            // just below the tick: the zero line (horizontal) must not strike through the "0"
            (cleft, a + 0.35)
        };
        let fits_axis = if sp.horizontal {
            pos + 0.4 + tw <= sp.len
        } else {
            pos + 0.35 + th <= sp.len + 0.5
        };
        let in_mask = sp.mask.is_some_and(|m| {
            let (lo, hi) = if sp.horizontal {
                (x, x + tw)
            } else {
                (y, y + th)
            };
            hi > m[0] && lo < m[1]
        });
        if fits_axis && !in_mask {
            b.text(id, x, y, &txt, RULER_LABEL_PT, INK, false);
            labels.push(json!({"text": txt, "x_mm": x, "y_mm": y, "value": i}));
        }
        i += 1;
    }
    let keep: Vec<&(u32, f64, f64)> = table.iter().filter(|t| !masked(sp.a0 + t.1)).collect();
    let ticks = json!({
        "index": keep.iter().map(|t| t.0).collect::<Vec<_>>(),
        "pos_mm": keep.iter().map(|t| t.1).collect::<Vec<_>>(),
        "length_mm": keep.iter().map(|t| t.2).collect::<Vec<_>>(),
    });
    let (origin, dir) = if sp.horizontal {
        ([sp.a0, sp.edge], "+x")
    } else {
        ([sp.edge, sp.a0], "+y")
    };
    let rect = ruler_r4(
        sp.horizontal,
        sp.a0,
        sp.a0 + sp.len,
        sp.edge,
        sp.edge + sp.d * max_len,
    );
    let ticks_point = match (sp.horizontal, sp.d > 0.0) {
        (true, true) => "down",
        (true, false) => "up",
        (false, true) => "right",
        (false, false) => "left",
    };
    let e = b.el(
        id,
        "ruler",
        "scale",
        rect,
        1.0,
        json!({
        "unit": if sp.inch { "in" } else { "mm" }, "mm_per_unit": if sp.inch { 25.4 } else { 1.0 },
        "axis": if sp.horizontal { "x" } else { "y" }, "direction": dir,
        "pitch_mm": pitch, "units_per_tick": if sp.inch { 1.0 / 16.0 } else { 1.0 }, "count": count,
        "length_mm": count as f64 * pitch, "span_mm": sp.len,
        "tiers": tiers.iter().map(|t| json!([t.0, t.1])).collect::<Vec<_>>(),
        "tier_names": if sp.inch { json!(["1/16 in", "1/8 in", "1/4 in", "1/2 in", "1 in"]) } else { json!(["1 mm", "5 mm", "10 mm"]) },
        "origin_mm": origin, "zero_line_mm": sp.a0, "tick_edge_mm": sp.edge, "ticks_point": ticks_point,
        "tick_width_mm": TICK_W_MM, "strip_rect_mm": sp.strip,
        "labels_every_mm": lab_every, "label_pt": RULER_LABEL_PT, "labels": labels,
        "masked_mm": sp.mask.map(|m| vec![m]).unwrap_or_default(),
        "ticks": ticks,
        "shared_zero_line": "the metric and the inch scale of one strip start at the same zero line; the two scales sit on opposite edges of the strip"}),
    );
    // samples at the major ticks (inside the tick, away from the strip lines)
    let major = if sp.inch { 16 } else { 10 };
    for (idx, pos, _) in &keep {
        if idx % major == 0 {
            let a = sp.a0 + pos;
            let c = sp.edge + sp.d * 0.8;
            if sp.horizontal {
                b.sample(e, a, c, INK, 190);
            } else {
                b.sample(e, c, a, INK, 190);
            }
        }
    }
}

/// Strip lines of one ruler strip: baseline(s), zero line, end line.
#[allow(clippy::too_many_arguments)]
fn ruler_lines(
    b: &mut B,
    id: &str,
    horizontal: bool,
    a0: f64,
    len: f64,
    c_lo: f64,
    c_hi: f64,
    inch: bool,
    metric: bool,
) {
    let t = 0.2;
    let mut rs: Vec<R4> = vec![];
    if metric {
        rs.push(ruler_r4(
            horizontal,
            a0,
            a0 + len,
            c_lo - t / 2.0,
            c_lo + t / 2.0,
        ));
    }
    if inch {
        rs.push(ruler_r4(
            horizontal,
            a0,
            a0 + len,
            c_hi - t / 2.0,
            c_hi + t / 2.0,
        ));
    }
    rs.push(ruler_r4(horizontal, a0 - 0.15, a0 + 0.15, c_lo, c_hi));
    rs.push(ruler_r4(
        horizontal,
        a0 + len - 0.15,
        a0 + len + 0.15,
        c_lo,
        c_hi,
    ));
    b.add(id, Prim::Rects { rs, c: INK });
}

fn blk_rulers(b: &mut B, r: R4, _sc: f64) {
    let cw = r[2] - r[0];
    let len = (((cw - 6.0) / 10.0).floor() * 10.0).max(10.0);
    let x0 = r[0] + (cw - len) / 2.0;
    // measure bar
    let bar = [x0, r[1] + 1.8, x0 + len, r[1] + 3.0];
    let label = format!("MEASURE HERE: THIS BAR IS EXACTLY {} MM", len as u32);
    let e = b.el(
        "measure_bar",
        "measure_bar",
        "scale",
        bar,
        1.0,
        json!({
        "length_mm": len, "x0_mm": x0, "x1_mm": x0 + len, "end_ticks_mm": 3.0, "label": label}),
    );
    b.rect("measure_bar", bar, INK);
    b.rect(
        "measure_bar",
        [bar[0] - 0.15, r[1], bar[0] + 0.15, r[1] + 4.8],
        INK,
    );
    b.rect(
        "measure_bar",
        [bar[2] - 0.15, r[1], bar[2] + 0.15, r[1] + 4.8],
        INK,
    );
    b.sample(
        e,
        (bar[0] + bar[2]) / 2.0,
        (bar[1] + bar[3]) / 2.0,
        INK,
        120,
    );
    b.text("measure_bar", x0, r[1] + 5.4, &label, 6.0, INK, false);
    // the dual-sided strip: metric ticks hang from the top edge, inch ticks rise from the bottom edge
    let rulers = b.spec.rulers;
    let yt = r[1] + 9.6;
    let yb = yt + HRULER_H_MM;
    let strip = [x0, yt, x0 + len, yb];
    ruler_lines(
        b,
        if rulers.metric() {
            "ruler_metric"
        } else {
            "ruler_inch"
        },
        true,
        x0,
        len,
        yt,
        yb,
        rulers.inch(),
        rulers.metric(),
    );
    for (id, inch, edge, d) in [
        ("ruler_metric", false, yt, 1.0),
        ("ruler_inch", true, yb, -1.0),
    ] {
        if (inch && !rulers.inch()) || (!inch && !rulers.metric()) {
            dropped_el(
                b,
                id,
                "ruler",
                "scale",
                "rulers parameter excludes this unit",
            );
            continue;
        }
        draw_ruler(
            b,
            &RulerSpec {
                id,
                inch,
                horizontal: true,
                a0: x0,
                edge,
                d,
                len,
                strip,
                mask: None,
            },
        );
    }
}

// ---- fiducial columns (vertical rulers, brackets, edge-midpoint markers) --------------------------------
#[allow(clippy::too_many_arguments)]
fn draw_bracket(b: &mut B, id: &str, ex: f64, ey: f64, sx: f64, sy: f64, idx: usize, corner: &str) {
    let br = fid::bracket(ex, ey, sx, sy, idx);
    let mut bb = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for q in &br.black {
        bb = [
            bb[0].min(q[0]),
            bb[1].min(q[1]),
            bb[2].max(q[2]),
            bb[3].max(q[3]),
        ];
    }
    let e = b.el(
        id,
        "corner_bracket",
        "fiducial",
        bb,
        1.0,
        json!({
        "corner": corner, "elbow_mm": [ex, ey], "arm_mm": fid::BRACKET_ARM_MM, "thickness_mm": fid::BRACKET_THICK_MM,
        "black_rects_mm": br.black, "white_mark_rect_mm": br.white_mark, "black_mark_rect_mm": br.black_mark,
        "mark_offset_mm": 4.0 + 1.2 * idx as f64,
        "meaning": "black L with a white square in the horizontal arm (its distance from the elbow differs per corner) and a black square in the inner corner: unambiguous orientation and an elbow point for sub-pixel corner location"}),
    );
    b.add(
        id,
        Prim::Rects {
            rs: br.black.clone(),
            c: INK,
        },
    );
    b.rect(id, br.white_mark, PAPER);
    b.rect(id, br.black_mark, INK);
    let mid = |q: R4| ((q[0] + q[2]) / 2.0, (q[1] + q[3]) / 2.0);
    b.sample(
        e,
        ex + sx * 1.2,
        ey + sy * fid::BRACKET_THICK_MM / 2.0,
        INK,
        120,
    );
    let (wx, wy) = mid(br.white_mark);
    b.sample(e, wx, wy, PAPER, 140);
    let (kx, ky) = mid(br.black_mark);
    b.sample(e, kx, ky, INK, 150);
}

/// Draw the left / right fiducial columns: corner L-brackets, a vertical ruler each and the
/// edge-midpoint ArUco markers. When the content box is too small everything is recorded as not drawn.
fn draw_strips(b: &mut B, content: R4, active: bool) {
    let names = ["tl", "tr", "bl", "br"];
    let ids = [
        ("ruler_v_metric", "ruler_v_inch", "marker_ml"),
        ("ruler_vr_metric", "ruler_vr_inch", "marker_mr"),
    ];
    if !active {
        let why = "content box too small for the fiducial columns";
        for n in names {
            dropped_el(
                b,
                &format!("bracket_{n}"),
                "corner_bracket",
                "fiducial",
                why,
            );
        }
        for (m, i, mk) in ids {
            dropped_el(b, m, "ruler", "scale", why);
            dropped_el(b, i, "ruler", "scale", why);
            dropped_el(b, mk, "marker", "fiducial", why);
        }
        return;
    }
    let ym = (content[1] + content[3]) / 2.0;
    let corners = [
        (content[0] + 0.5, content[1] + 0.5, 1.0, 1.0),
        (content[2] - 0.5, content[1] + 0.5, -1.0, 1.0),
        (content[0] + 0.5, content[3] - 0.5, 1.0, -1.0),
        (content[2] - 0.5, content[3] - 0.5, -1.0, -1.0),
    ];
    let cnames = ["top_left", "top_right", "bottom_left", "bottom_right"];
    for (i, (ex, ey, sx, sy)) in corners.into_iter().enumerate() {
        draw_bracket(
            b,
            &format!("bracket_{}", names[i]),
            ex,
            ey,
            sx,
            sy,
            i,
            cnames[i],
        );
    }
    let rulers = b.spec.rulers;
    let side_m = 6.0 * SIDE_MARKER_MODULE_MM;
    let half_win = side_m / 2.0 + SIDE_MARKER_MODULE_MM;
    let y0 = content[1] + 12.0;
    let len = (((content[3] - 12.0 - y0) / 10.0).floor() * 10.0).max(0.0);
    for (ci, (mid, iid, mk)) in ids.into_iter().enumerate() {
        let col_x = if ci == 0 {
            content[0]
        } else {
            content[2] - STRIP_COL_MM
        };
        let xs = col_x + (STRIP_COL_MM - VRULER_W_MM) / 2.0;
        let xr = xs + VRULER_W_MM;
        let strip = [xs, y0, xr, y0 + len];
        let mask = Some([ym - half_win, ym + half_win]);
        if len >= 40.0 {
            ruler_lines(
                b,
                if rulers.metric() { mid } else { iid },
                false,
                y0,
                len,
                xs,
                xr,
                rulers.inch(),
                rulers.metric(),
            );
            for (id, inch, edge, d) in [(mid, false, xs, 1.0), (iid, true, xr, -1.0)] {
                if (inch && !rulers.inch()) || (!inch && !rulers.metric()) {
                    dropped_el(
                        b,
                        id,
                        "ruler",
                        "scale",
                        "rulers parameter excludes this unit",
                    );
                    continue;
                }
                draw_ruler(
                    b,
                    &RulerSpec {
                        id,
                        inch,
                        horizontal: false,
                        a0: y0,
                        edge,
                        d,
                        len,
                        strip,
                        mask,
                    },
                );
            }
        } else {
            dropped_el(
                b,
                mid,
                "ruler",
                "scale",
                "column too short for a vertical ruler",
            );
            dropped_el(
                b,
                iid,
                "ruler",
                "scale",
                "column too short for a vertical ruler",
            );
        }
        let cx = col_x + STRIP_COL_MM / 2.0;
        draw_aruco(
            b,
            mk,
            if ci == 0 { ARUCO_ID_ML } else { ARUCO_ID_MR },
            [
                cx - side_m / 2.0,
                ym - side_m / 2.0,
                cx + side_m / 2.0,
                ym + side_m / 2.0,
            ],
            if ci == 0 { "left_mid" } else { "right_mid" },
        );
    }
}

// ---- calibration boards ------------------------------------------------------------------------------------
fn board_dropped(b: &mut B, id: &str, rect: R4, why: &str) {
    let e = b.el(id, "board", "calibration", rect, 1.0, json!({}));
    b.els[e].drawn = false;
    b.els[e].reason = Some(why.to_string());
}

fn blk_chess(b: &mut B, r: R4, sc: f64) {
    let sq = CHESS_SQ_MM * sc;
    if sq < fid::MIN_SQUARE_MM {
        return board_dropped(b, "chessboard", r, "square below the minimum size");
    }
    let [cols, rows] = CHESS_SQUARES;
    let (pw, ph) = (cols as f64 * sq, rows as f64 * sq);
    let (bw, bh) = (r[2] - r[0], r[3] - r[1]);
    let (x0, y0) = (r[0] + (bw - pw) / 2.0, r[1] + (bh - ph) / 2.0);
    let (black, inner) = fid::chessboard(x0, y0, cols, rows, sq);
    let e = b.el(
        "chessboard",
        "chessboard",
        "calibration",
        [x0, y0, x0 + pw, y0 + ph],
        sc,
        json!({
        "squares": [cols, rows], "inner_corners": [cols - 1, rows - 1], "square_mm": sq, "origin_mm": [x0, y0],
        "first_square": "black (top-left)", "quiet_mm": sq, "min_square_mm": fid::MIN_SQUARE_MM,
        "inner_corner_positions_mm": inner,
        "opencv": format!("cv2.findChessboardCorners(img, ({}, {}))", cols - 1, rows - 1)}),
    );
    b.add("chessboard", Prim::Rects { rs: black, c: INK });
    if sq * b.k >= 2.5 {
        for rr in 0..rows {
            for cc in 0..cols {
                let (want, tol) = if (rr + cc) % 2 == 0 {
                    (INK, 90)
                } else {
                    (PAPER, 140)
                };
                b.sample(
                    e,
                    x0 + (cc as f64 + 0.5) * sq,
                    y0 + (rr as f64 + 0.5) * sq,
                    want,
                    tol,
                );
            }
        }
    }
}

fn blk_charuco(b: &mut B, r: R4, sc: f64) {
    let sq = CHARUCO_SQ_MM * sc;
    let ml = sq * CHARUCO_MARKER_RATIO;
    if sq < fid::MIN_SQUARE_MM || ml / 6.0 < fid::MIN_MODULE_MM * 0.7 {
        return board_dropped(
            b,
            "charuco_board",
            r,
            "square or marker module below the minimum size",
        );
    }
    let [cols, rows] = CHARUCO_SQUARES;
    let (pw, ph) = (cols as f64 * sq, rows as f64 * sq);
    let (bw, bh) = (r[2] - r[0], r[3] - r[1]);
    let (x0, y0) = (r[0] + (bw - pw) / 2.0, r[1] + (bh - ph) / 2.0);
    let cb = fid::charuco(x0, y0, cols, rows, sq, ml, CHARUCO_FIRST_ID);
    let ids: Vec<usize> = cb.markers.iter().map(|m| m.0).collect();
    let id_first = ids[0];
    let id_last = *ids.last().unwrap();
    let e = b.el(
        "charuco_board",
        "charuco",
        "calibration",
        [x0, y0, x0 + pw, y0 + ph],
        sc,
        json!({
        "squares": [cols, rows], "square_mm": sq, "marker_mm": ml, "marker_module_mm": ml / 6.0,
        "dictionary": fid::ARUCO_DICT_NAME, "marker_ids": ids, "legacy_pattern": false,
        "first_square": "black (top-left); markers in the squares with odd row + col, ids row-major",
        "origin_mm": [x0, y0], "quiet_mm": sq, "chessboard_corners_mm": cb.corners,
        "corner_count": cb.corners.len(),
        "opencv": format!(
            "cv2.aruco.CharucoBoard(({cols}, {rows}), square_mm, marker_mm, DICT_4X4_50, np.arange({id_first}, {}))",
            id_last + 1),
        "markers": cb.markers.iter().map(|(id, q)| json!({"id": id, "rect_mm": q, "corners_mm": fid::corners_of(*q)})).collect::<Vec<_>>()}),
    );
    b.add(
        "charuco_board",
        Prim::Rects {
            rs: cb.black_squares.clone(),
            c: INK,
        },
    );
    if sq * b.k >= 2.5 {
        for sqr in &cb.black_squares {
            b.sample(e, (sqr[0] + sqr[2]) / 2.0, (sqr[1] + sqr[3]) / 2.0, INK, 90);
        }
    }
    for (aid, q) in &cb.markers {
        draw_aruco(b, &format!("charuco_marker_{aid}"), *aid, *q, "charuco");
    }
}

fn blk_circles(b: &mut B, r: R4, sc: f64) {
    let sp = CIRCLES_SP_MM * sc;
    if sp < fid::MIN_SQUARE_MM * 0.9 {
        return board_dropped(b, "circle_grid", r, "spacing below the minimum size");
    }
    let [per_row, rows] = CIRCLES_GRID;
    let (pw, ph) = ((2 * (per_row - 1) + 1) as f64 * sp, (rows - 1) as f64 * sp);
    let (bw, bh) = (r[2] - r[0], r[3] - r[1]);
    let (x0, y0) = (r[0] + (bw - pw) / 2.0, r[1] + (bh - ph) / 2.0);
    let pts = fid::circle_grid(x0, y0, per_row, rows, sp);
    let dia = sp * CIRCLES_DIAMETER_RATIO;
    let e = b.el(
        "circle_grid",
        "circle_grid",
        "calibration",
        [
            x0 - dia / 2.0,
            y0 - dia / 2.0,
            x0 + pw + dia / 2.0,
            y0 + ph + dia / 2.0,
        ],
        sc,
        json!({
        "pattern": "asymmetric", "per_row": per_row, "rows": rows, "points": pts.len(),
        "row_pitch_mm": sp, "in_row_pitch_mm": 2.0 * sp, "circle_diameter_mm": dia, "origin_mm": [x0, y0],
        "centres_mm": pts, "layout": "x = (2 j + i % 2) * row_pitch, y = i * row_pitch (OpenCV asymmetric grid, row-major)",
        "quiet_mm": 1.5 * sp,
        "opencv": format!("cv2.findCirclesGrid(img, ({per_row}, {rows}), flags=cv2.CALIB_CB_ASYMMETRIC_GRID)")}),
    );
    for p in &pts {
        b.add(
            "circle_grid",
            Prim::Circle {
                c: *p,
                r: dia / 2.0,
                fill: Some(INK),
                stroke: None,
            },
        );
        if dia * b.k >= 3.0 {
            b.sample(e, p[0], p[1], INK, 120);
        }
    }
}

/// Geometry of one corner cluster `[glyph][marker][registration cross]` at scale `u`.
struct ClusterGeom {
    gw: f64,
    g1: f64,
    m: f64,
    g2: f64,
    arm: f64,
    width: f64,
}

fn cluster_geom(u: f64) -> ClusterGeom {
    let gw = 5.0 * 1.25 * u;
    let m = (8.0 * u).max(6.0 * fid::MIN_MODULE_MM);
    let q = m / 6.0;
    let g1 = (1.5 * u).max(q);
    let g2 = (1.0 * u).max(q);
    let arm = 3.8 * u * 0.7;
    ClusterGeom {
        gw,
        g1,
        m,
        g2,
        arm,
        width: gw + g1 + m + g2 + 2.0 * arm,
    }
}

// ---- corner bands ---------------------------------------------------------------------------------------
fn draw_cross(b: &mut B, id: &str, c: [f64; 2], u: f64) {
    b.add(
        id,
        Prim::Circle {
            c,
            r: 3.0 * u,
            fill: None,
            stroke: Some((INK, 0.3)),
        },
    );
    b.add(
        id,
        Prim::Line {
            a: [c[0] - 3.8 * u, c[1]],
            b: [c[0] + 3.8 * u, c[1]],
            w: 0.3,
            c: INK,
            dash: None,
        },
    );
    b.add(
        id,
        Prim::Line {
            a: [c[0], c[1] - 3.8 * u],
            b: [c[0], c[1] + 3.8 * u],
            w: 0.3,
            c: INK,
            dash: None,
        },
    );
}

fn glyph_for(side: Option<PageSide>, corner: usize) -> (char, bool) {
    if side == Some(PageSide::Back) {
        (GLYPHS_BACK[corner], true)
    } else {
        (GLYPHS_FRONT[corner], false)
    }
}

/// Glyph bitmap rows (5 chars each, '1' = ink), mirror applied.
pub fn glyph_bitmap(ch: char, mirror: bool) -> Vec<String> {
    glyph_rows(ch)
        .iter()
        .map(|bits| {
            let row: String = (0..5)
                .map(|c| if bits & (1 << (4 - c)) != 0 { '1' } else { '0' })
                .collect();
            if mirror {
                row.chars().rev().collect()
            } else {
                row
            }
        })
        .collect()
}

fn corner_cluster(b: &mut B, r: R4, sc: f64, top: bool) {
    // r is the whole band. Left cluster [glyph][marker][cross], right cluster mirrored.
    let u = sc;
    let gu = 1.25 * u;
    let g = cluster_geom(u);
    let (gw, m) = (g.gw, g.m);
    let yc = (r[1] + r[3]) / 2.0;
    let side = b.spec.side;
    for (i, left) in [true, false].into_iter().enumerate() {
        let corner = if top { i } else { 2 + i };
        let (ch, mirror) = glyph_for(side, corner);
        let gid = ["orient_tl", "orient_tr", "orient_bl", "orient_br"][corner];
        let mid = ["marker_tl", "marker_tr", "marker_bl", "marker_br"][corner];
        let aid = [ARUCO_ID_TL, ARUCO_ID_TR, ARUCO_ID_BL, ARUCO_ID_BR][corner];
        let cid = ["reg_tl", "reg_tr", "reg_bl", "reg_br"][corner];
        let (gx, mx, cx) = if left {
            (r[0], r[0] + gw + g.g1, r[0] + gw + g.g1 + m + g.g2 + g.arm)
        } else {
            (
                r[2] - gw,
                r[2] - gw - g.g1 - m,
                r[2] - gw - g.g1 - m - g.g2 - g.arm,
            )
        };
        let gy = yc - 3.5 * gu;
        let corner_name = ["top_left", "top_right", "bottom_left", "bottom_right"][corner];
        let rect = [gx, gy, gx + gw, gy + 7.0 * gu];
        b.el(gid, "orientation_glyph", "orientation", rect, sc, json!({
            "glyph": ch.to_string(), "mirrored": mirror, "corner": corner_name,
            "cell_mm": gu, "bitmap": glyph_bitmap(ch, mirror), "side": side.map(|s| s.as_str()),
            "meaning": "asymmetric letter per corner: rotation, mirror and flip show as a different glyph in a different corner; the back side mirrors the glyphs and rotates the assignment"}));
        b.text(
            gid,
            gx,
            gy,
            &ch.to_string(),
            7.0 * gu / (0.72 * 25.4 / 72.0),
            INK,
            mirror,
        );
        let mrect = [mx, yc - m / 2.0, mx + m, yc + m / 2.0];
        draw_aruco(b, mid, aid, mrect, corner_name);
        draw_cross(b, cid, [cx, yc], u * 0.7);
        let (ax, ay) = (cx, yc);
        b.el(cid, "registration_cross", "fiducial", [ax - 3.8 * u * 0.7, ay - 3.8 * u * 0.7, ax + 3.8 * u * 0.7, ay + 3.8 * u * 0.7], sc, json!({
            "centre_mm": [ax, ay], "ring_radius_mm": 3.0 * u * 0.7, "arm_mm": 3.8 * u * 0.7, "line_mm": 0.3, "use": "affine fit: scale x/y, rotation, shear, offset"}));
    }
}

#[allow(unused_assignments)]
fn blk_band_top(b: &mut B, r: R4, sc: f64) {
    corner_cluster(b, r, sc, true);
    let u = sc;
    let g = cluster_geom(u);
    let cluster = g.width + 3.0 * u;
    let (x0, x1) = (r[0] + cluster, r[2] - cluster);
    let yc = (r[1] + r[3]) / 2.0;
    let page = b.spec.page;
    let (mut lim, rx0) = band_mid_marker(b, r, yc, &g, x0, x1, "marker_tm", ARUCO_ID_TM, "top_mid");
    let mut jumped = lim >= x1;
    let mut x = x0;
    // flow cursor: items fill the left region, then continue right of the midpoint marker
    macro_rules! fit {
        ($need:expr) => {{
            let n: f64 = $need;
            if x + n <= lim {
                true
            } else if !jumped {
                jumped = true;
                x = rx0;
                lim = x1;
                x + n <= lim
            } else {
                false
            }
        }};
    }
    // big page number
    let digits = (page + 1).to_string();
    let gu = 1.15 * u;
    let dw = (digits.len() * 6) as f64 * gu - gu;
    if fit!(dw) {
        let rect = [x, yc - 3.5 * gu, x + dw, yc + 3.5 * gu];
        let digit_bitmaps: Vec<Value> = "0123456789"
            .chars()
            .map(|d| json!({"char": d.to_string(), "rows": glyph_bitmap(d, false)}))
            .collect();
        b.el(
            "page_number",
            "page_number",
            "order",
            rect,
            sc,
            json!({"text": digits, "value": page + 1, "cell_mm": gu,
            "origin_mm": [x, yc - 3.5 * gu], "char_pitch_cells": 6, "digit_bitmaps": digit_bitmaps,
            "meaning": "1-based page number in big glyphs"}),
        );
        b.text(
            "page_number",
            x,
            yc - 3.5 * gu,
            &digits,
            7.0 * gu / (0.72 * 25.4 / 72.0),
            INK,
            false,
        );
        x += dw + 2.0 * u;
    }
    // order bars
    let (pitch, bw, bh) = (1.4 * u, 0.9 * u, 6.0 * u);
    let need = ORDER_SLOTS as f64 * pitch;
    if fit!(need) {
        let filled = (page + 1).min(ORDER_SLOTS);
        let rect = [x, yc - bh / 2.0, x + need, yc + bh / 2.0];
        let slots: Vec<f64> = (0..ORDER_SLOTS)
            .map(|i| x + i as f64 * pitch + bw / 2.0)
            .collect();
        let e = b.el("order_bars", "order_bars", "order", rect, sc, json!({
            "slots": ORDER_SLOTS, "filled": filled, "slot_centres_x_mm": slots, "bar_mm": [bw, bh], "ink": INK, "empty_rgb": [215, 215, 215],
            "meaning": "page index + 1 filled bars from the left (capped at 16), the numbered-bars cue of the old page_id pattern"}));
        for i in 0..ORDER_SLOTS {
            let rr = [
                x + i as f64 * pitch,
                yc - bh / 2.0,
                x + i as f64 * pitch + bw,
                yc + bh / 2.0,
            ];
            let c = if i < filled { INK } else { [215, 215, 215] };
            b.rect("order_bars", rr, c);
            b.sample(
                e,
                (rr[0] + rr[2]) / 2.0,
                yc,
                c,
                if bw * b.k >= 1.5 { 40 } else { 120 },
            );
        }
        x += need + 2.0 * u;
    }
    // side marker
    let sm_w = 5.5 * u;
    let back = b.spec.side == Some(PageSide::Back);
    let label = match b.spec.side {
        Some(PageSide::Front) => "FRONT",
        Some(PageSide::Back) => "BACK",
        None => "-",
    };
    let (lw, lh) = text_size_mm(label.len(), 6.0);
    if fit!(sm_w + 1.0 * u + lw) {
        let sq = [x, yc - 3.5 * u, x + sm_w, yc + 3.5 * u];
        let half = if back {
            [sq[0] + sm_w / 2.0, sq[1], sq[2], sq[3]]
        } else {
            [sq[0], sq[1], sq[0] + sm_w / 2.0, sq[3]]
        };
        let e = b.el("side_marker", "side_marker", "order", [sq[0], sq[1], x + sm_w + 1.0 * u + lw, sq[3]], sc, json!({
            "side": b.spec.side.map(|s| s.as_str()), "square_mm": sq, "ink_half": if back { "right" } else { "left" }, "label": label,
            "meaning": "duplex face: front inks the left half, back the right half; the back also mirrors the corner glyphs"}));
        b.outline("side_marker", sq, 0.25, INK);
        b.rect("side_marker", half, INK);
        let (ix, px_) = if back {
            (sq[0] + sm_w * 0.25, sq[0] + sm_w * 0.75)
        } else {
            (sq[0] + sm_w * 0.75, sq[0] + sm_w * 0.25)
        };
        b.sample(e, px_, yc, INK, 60);
        b.sample(e, ix, yc, PAPER, 60);
        b.text(
            "side_marker",
            x + sm_w + 1.0 * u,
            yc - lh / 2.0,
            label,
            6.0,
            INK,
            false,
        );
        x += sm_w + 1.0 * u + lw + 2.0 * u;
    }
    // stamp
    let l1 = if b.spec.total > 0 {
        format!("PAGE {} OF {}", page + 1, b.spec.total)
    } else {
        format!("PAGE {}", page + 1)
    };
    let job = if b.spec.job.is_empty() {
        "-".to_string()
    } else {
        b.spec.job.clone()
    };
    let l2 = format!("COPY {} JOB {}", b.spec.copy, job);
    let (w1, h1) = text_size_mm(l1.len(), 6.0);
    let (w2, _) = text_size_mm(l2.len(), 6.0);
    let sw = w1.max(w2);
    if fit!(sw) {
        b.el("page_stamp", "page_stamp", "order", [x, yc - h1 - 0.6, x + sw, yc + h1 + 0.6], sc, json!({
            "lines": [l1, l2], "page": page, "copy": b.spec.copy, "total": b.spec.total, "job": b.spec.job, "pt": 6}));
        b.text("page_stamp", x, yc - h1 - 0.4, &l1, 6.0, INK, false);
        b.text("page_stamp", x, yc + 0.4, &l2, 6.0, INK, false);
    } else {
        let e = b.el(
            "page_stamp",
            "page_stamp",
            "order",
            [x, yc, x, yc],
            sc,
            json!({"lines": [l1, l2]}),
        );
        b.els[e].drawn = false;
        b.els[e].reason = Some("no room beside the order elements".into());
    }
}

fn blk_band_bottom(b: &mut B, r: R4, sc: f64) {
    corner_cluster(b, r, sc, false);
    let u = sc;
    let g = cluster_geom(u);
    let cluster = g.width + 3.0 * u;
    let (x0, x1) = (r[0] + cluster, r[2] - cluster);
    let yc = (r[1] + r[3]) / 2.0;
    let s = &b.spec;
    let l1 = format!(
        "{} {}X{} MM {} DPI",
        s.paper,
        fmt_num((s.width_mm() * 10.0).round() / 10.0),
        fmt_num((s.height_mm() * 10.0).round() / 10.0),
        s.dpi
    );
    let l2 = format!(
        "DIAG LAYOUT 3 MARGIN {} BLEED {}",
        fmt_num(s.margin_mm),
        fmt_num(s.bleed_mm)
    );
    let (w1, h1) = text_size_mm(l1.len(), 6.0);
    let (w2, _) = text_size_mm(l2.len(), 6.0);
    let (lim, rx0) = band_mid_marker(b, r, yc, &g, x0, x1, "marker_bm", ARUCO_ID_BM, "bottom_mid");
    let wmax = w1.max(w2);
    let lx = if x0 + wmax <= lim {
        Some(x0)
    } else if rx0 + wmax <= x1 {
        Some(rx0)
    } else {
        None
    };
    if let Some(x0) = lx {
        b.el(
            "legend",
            "legend",
            "text",
            [x0, yc - h1 - 0.6, x0 + w1.max(w2), yc + h1 + 0.6],
            sc,
            json!({"lines": [l1, l2]}),
        );
        b.text("legend", x0, yc - h1 - 0.4, &l1, 6.0, INK, false);
        b.text("legend", x0, yc + 0.4, &l2, 6.0, INK, false);
    }
}

// ------------------------------------------------------------------------------------------------
// Row table and packer
// ------------------------------------------------------------------------------------------------

fn blk(id: &'static str, w: f64, h: f64, fill: bool, scale: bool, draw: DrawFn) -> Blk {
    Blk {
        id,
        w,
        h,
        fill,
        scale,
        draw,
        optional: false,
    }
}

fn opt(mut b: Blk) -> Blk {
    b.optional = true;
    b
}

fn rows() -> Vec<Row> {
    vec![
        Row {
            name: "band_top",
            prio: 0,
            blocks: vec![blk("band_top", 80.0, 11.0, true, true, blk_band_top)],
        },
        Row {
            name: "bars",
            prio: 3,
            blocks: vec![blk("bars", 80.0, 14.0, true, true, blk_smpte)],
        },
        Row {
            name: "ramps",
            prio: 5,
            blocks: vec![
                blk("ramps_rgbn", 40.0, 17.0, true, true, blk_ramps_rgbn),
                blk("ramps_cmyk", 40.0, 17.0, true, true, blk_ramps_cmyk),
            ],
        },
        Row {
            name: "steps",
            prio: 4,
            blocks: vec![blk("steps", 80.0, 8.0, true, true, blk_steps)],
        },
        Row {
            name: "flat",
            prio: 6,
            blocks: vec![blk("flat", 80.0, 9.0, true, true, blk_flat)],
        },
        Row {
            name: "colour_geometry",
            prio: 2,
            blocks: vec![
                blk("cc24", 49.0, 32.4, false, true, blk_cc24),
                blk("wheel", 32.0, 32.0, false, true, blk_wheel),
                blk("star", 32.0, 32.0, false, true, blk_star),
                blk("slants", 17.0, 33.0, false, true, blk_slants),
            ],
        },
        Row {
            name: "resolution",
            prio: 7,
            blocks: vec![
                blk("res", 32.0, 8.8, false, false, blk_res),
                opt(blk("grid", 20.0, 8.8, false, false, blk_grid)),
                blk("ladder_pos", 30.0, 9.8, false, false, blk_ladder_pos),
                opt(blk("ladder_neg", 30.0, 9.8, false, false, blk_ladder_neg)),
                opt(blk("halftone", 30.0, 9.8, false, true, blk_halftone)),
            ],
        },
        Row {
            name: "zone_qr",
            prio: 0,
            blocks: vec![
                blk("zone", 32.0, 32.0, false, true, blk_zone),
                blk("geometry", 56.0, 17.0, false, true, blk_geometry),
                blk("qr", 34.0, 34.0, false, true, blk_qr),
            ],
        },
        Row {
            name: "text",
            prio: 8,
            blocks: vec![
                blk("text_ladder", 60.0, 14.0, false, false, blk_text_ladder),
                opt(blk("pangram", 58.0, 14.0, false, false, blk_pangram)),
                opt(blk("toc", 24.0, 13.6, false, false, blk_toc)),
            ],
        },
        Row {
            name: "boards",
            prio: 6,
            blocks: vec![
                blk("chess", 38.5, 28.0, false, true, blk_chess),
                blk("charuco", 44.0, 33.0, false, true, blk_charuco),
                blk("circles", 42.0, 21.0, false, true, blk_circles),
            ],
        },
        Row {
            name: "rulers",
            prio: 1,
            blocks: vec![blk("rulers", 60.0, 21.0, true, false, blk_rulers)],
        },
        Row {
            name: "band_bottom",
            prio: 0,
            blocks: vec![blk("band_bottom", 80.0, 11.0, true, true, blk_band_bottom)],
        },
    ]
}

/// Row size at scale `s` with the blocks in `dropped` removed.
fn row_dims(row: &Row, s: f64, dropped: &[bool]) -> (f64, f64) {
    let (mut w, mut h, mut n) = (0.0, 0.0f64, 0usize);
    for (i, b) in row.blocks.iter().enumerate() {
        if dropped[i] {
            continue;
        }
        let sc = if b.scale { s } else { 1.0 };
        w += b.w * sc;
        h = h.max(b.h * sc);
        n += 1;
    }
    (w + GAP_MIN * (n as f64 - 1.0).max(0.0), h)
}

/// Drop optional blocks (right to left) until the row is at most `cw` wide. `None` if impossible.
fn fit_row(row: &Row, s: f64, cw: f64, allow_opt: bool) -> Option<(Vec<bool>, f64, f64)> {
    let mut dropped = vec![false; row.blocks.len()];
    loop {
        let (w, h) = row_dims(row, s, &dropped);
        if w <= cw {
            return Some((dropped, w, h));
        }
        if !allow_opt {
            return None;
        }
        let victim = (0..row.blocks.len())
            .rev()
            .find(|&i| row.blocks[i].optional && !dropped[i])?;
        dropped[victim] = true;
    }
}

/// Build the layout (primitives + elements) for a page spec. Pure function of `spec`.
pub fn build_layout(spec: &PageSpec) -> Layout {
    let k = spec.k();
    let (w, h) = (spec.width_mm(), spec.height_mm());
    let mut b = B {
        k,
        w,
        h,
        gray: spec.gray,
        prims: vec![],
        els: vec![],
        spec: spec.clone(),
    };
    let mut notes: Vec<String> = vec![];
    let banner_mm = spec.banner_px as f64 / k;
    let m = spec.margin_mm.min(w / 4.0).min(h / 4.0);
    let frame_line = 0.4f64.max(1.5 / k);
    let top = if spec.banner_px > 0 {
        m.max(banner_mm + 0.8)
    } else {
        m
    };
    let frame = [m, top, w - m, h - m];

    // ---- sheet-relative marks (drawn first so content paints over them) -------------------------
    // dashed media outlines
    let landscape = w > h;
    let mut outline_els: Vec<usize> = vec![];
    for (name, pw, ph) in OUTLINES {
        let (ow, oh) = if landscape { (ph, pw) } else { (pw, ph) };
        let fits = ow < w - 0.5 && oh < h - 0.5;
        let rect = [0.0, 0.0, ow, oh];
        let e = b.el(&format!("outline_{}", name.to_ascii_lowercase()), "media_outline", "print_marks", rect, 1.0, json!({
            "media": name, "size_mm": [ow, oh], "anchor": "top-left of the sheet", "style": "dashed 4 mm on 2 mm off, 0.2 mm, grey 120",
            "fits": fits, "note": "drawn only when the medium is smaller than the sheet in both directions"}));
        b.els[e].drawn = fits;
        if !fits {
            b.els[e].reason = Some("medium does not fit inside the sheet".into());
            continue;
        }
        let id = format!("outline_{}", name.to_ascii_lowercase());
        let pts = [
            [0.1, 0.1],
            [ow - 0.1, 0.1],
            [ow - 0.1, oh - 0.1],
            [0.1, oh - 0.1],
        ];
        for i in 0..4 {
            b.add(
                &id,
                Prim::Line {
                    a: pts[i],
                    b: pts[(i + 1) % 4],
                    w: 0.2,
                    c: OUTLINE_RGB,
                    dash: Some([4.0, 2.0]),
                },
            );
        }
        outline_els.push(e);
    }

    // ---- frame, border, ticks ---------------------------------------------------------------------
    let fe = b.el("printable_frame", "frame", "print_marks", frame, 1.0, json!({
        "meaning": "printable area: frame at the margin from the sheet edge (top edge sits below the tester banner when there is one)",
        "margin_mm": m, "line_mm": frame_line, "rgb": FRAME_RGB,
        "inset_mm": {"left": frame[0], "right": w - frame[2], "bottom": h - frame[3], "top": frame[1]},
        "edges": [{"side": "left", "pos_mm": frame[0] + frame_line / 2.0}, {"side": "right", "pos_mm": frame[2] - frame_line / 2.0},
                  {"side": "top", "pos_mm": frame[1] + frame_line / 2.0}, {"side": "bottom", "pos_mm": frame[3] - frame_line / 2.0}]}));
    b.outline("printable_frame", frame, frame_line, FRAME_RGB);
    let mid_x = (frame[0] + frame[2]) / 2.0;
    let mid_y = (frame[1] + frame[3]) / 2.0;
    b.sample(fe, mid_x, frame[1] + frame_line / 2.0, FRAME_RGB, 140);
    b.sample(fe, mid_x, frame[3] - frame_line / 2.0, FRAME_RGB, 140);
    b.sample(fe, frame[0] + frame_line / 2.0, mid_y, FRAME_RGB, 140);
    b.sample(fe, frame[2] - frame_line / 2.0, mid_y, FRAME_RGB, 140);
    let px_mm = 1.0 / k;
    let be = b.el("page_border", "frame", "print_marks", [0.0, 0.0, w, h], 1.0, json!({
        "meaning": "physical sheet edge: 1 px black hairline at the very edge of the raster", "rgb": INK, "inset_px": 0,
        "top_edge_note": "the top row lies under the tester banner when one is stamped"}));
    b.outline("page_border", [0.0, 0.0, w, h], px_mm, INK);
    b.sample(be, mid_x, h - px_mm / 2.0, INK, 30);
    b.sample(be, px_mm / 2.0, mid_y, INK, 30);
    b.sample(be, w - px_mm / 2.0, mid_y, INK, 30);
    let (o, a, t) = (0.5, 8.0f64.min(m * 1.8 + 4.0), 0.3);
    let ce = b.el(
        "corner_ticks",
        "marks",
        "print_marks",
        [o, o, w - o, h - o],
        1.0,
        json!({"arm_mm": a, "line_mm": t, "inset_mm": o}),
    );
    let _ = ce;
    for r in [
        [o, o, o + a, o + t],
        [o, o, o + t, o + a],
        [w - o - a, o, w - o, o + t],
        [w - o - t, o, w - o, o + a],
        [o, h - o - t, o + a, h - o],
        [o, h - o - a, o + t, h - o],
        [w - o - a, h - o - t, w - o, h - o],
        [w - o - t, h - o - a, w - o, h - o],
    ] {
        b.rect("corner_ticks", r, INK);
    }
    for (id, r) in [
        ("edge_tick_left", [o, mid_y - 3.0, o + 4.0, mid_y + 3.0]),
        (
            "edge_tick_right",
            [w - o - 4.0, mid_y - 3.0, w - o, mid_y + 3.0],
        ),
        (
            "edge_tick_bottom",
            [mid_x - 3.0, h - o - 4.0, mid_x + 3.0, h - o],
        ),
    ] {
        let (cx, cy) = ((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0);
        let horiz = r[2] - r[0] > r[3] - r[1];
        b.el(id, "marks", "print_marks", r, 1.0, json!({"centre_mm": [cx, cy], "note": "midpoint tick: centring of the scan / print on the sheet", "horizontal": horiz}));
        if horiz {
            b.rect(id, [r[0], cy - t / 2.0, r[2], cy + t / 2.0], INK);
        } else {
            b.rect(id, [cx - t / 2.0, r[1], cx + t / 2.0, r[3]], INK);
        }
    }
    if spec.bleed_mm > 0.0 {
        let bl = (2.0 + spec.bleed_mm).min((m - 0.8).max(0.4));
        let e = b.el("bleed_probes", "bleed", "print_marks", [0.0, 0.0, w, h], 1.0, json!({
            "bleed_mm": spec.bleed_mm, "bars_mm": bl,
            "meaning": "solid bars flush with the left, right and bottom sheet edges: borderless / bleed output keeps them to the edge, margin-clipping devices shorten them",
            "bleed_box_mm": [-spec.bleed_mm, -spec.bleed_mm, w + spec.bleed_mm, h + spec.bleed_mm]}));
        for r in [
            [0.0, mid_y - 10.0, bl, mid_y + 10.0],
            [w - bl, mid_y - 10.0, w, mid_y + 10.0],
            [mid_x - 10.0, h - bl, mid_x + 10.0, h],
        ] {
            b.rect("bleed_probes", r, INK);
            b.sample(e, (r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0, INK, 4);
        }
    } else {
        notes.push("bleed_mm = 0: no bleed probes drawn; set bleed_mm > 0 to add flush edge bars and the bleed box".to_string());
    }

    // ---- block flow ------------------------------------------------------------------------------------
    let pad = frame_line + 1.0;
    let content = [
        frame[0] + pad,
        frame[1] + pad,
        frame[2] - pad,
        frame[3] - pad,
    ];
    let strips = content[2] - content[0] >= STRIP_MIN_CONTENT_MM[0]
        && content[3] - content[1] >= STRIP_MIN_CONTENT_MM[1];
    // the rows live between the fiducial columns
    let inner = if strips {
        [
            content[0] + STRIP_COL_MM + 1.0,
            content[1],
            content[2] - STRIP_COL_MM - 1.0,
            content[3],
        ]
    } else {
        content
    };
    draw_strips(&mut b, content, strips);
    let (cw, ch) = (inner[2] - inner[0], inner[3] - inner[1]);
    let all = rows();
    let mut keep: Vec<bool> = vec![true; all.len()];
    let mut s = MIN_SCALE;
    let mut overflow = false;
    if cw > 20.0 && ch > 20.0 {
        loop {
            let live: Vec<&Row> = all
                .iter()
                .zip(&keep)
                .filter(|(_, k)| **k)
                .map(|(r, _)| r)
                .collect();
            let mut found = None;
            'passes: for allow_opt in [false, true] {
                let mut cand = MAX_SCALE;
                while cand >= MIN_SCALE - 1e-9 {
                    let fits: Vec<Option<(Vec<bool>, f64, f64)>> = live
                        .iter()
                        .map(|r| fit_row(r, cand, cw, allow_opt))
                        .collect();
                    if fits.iter().all(Option::is_some) {
                        let total_h: f64 = fits.iter().flatten().map(|d| d.2).sum::<f64>()
                            + GAP_MIN * (live.len() as f64 - 1.0);
                        if total_h <= ch {
                            found = Some(cand);
                            break 'passes;
                        }
                    }
                    cand -= 0.01;
                }
            }
            if let Some(f) = found {
                s = f;
                break;
            }
            // drop the least important live row
            let victim = all
                .iter()
                .enumerate()
                .filter(|(i, r)| keep[*i] && r.prio > 0)
                .max_by_key(|(i, r)| (r.prio, *i))
                .map(|(i, _)| i);
            match victim {
                Some(i) => keep[i] = false,
                None => {
                    overflow = true;
                    s = MIN_SCALE;
                    break;
                }
            }
        }
    } else {
        overflow = true;
        keep = vec![false; all.len()];
    }
    if overflow {
        notes.push(
            "sheet too small for the mandatory rows at the minimum scale; layout truncated"
                .to_string(),
        );
    }
    for (row, k_) in all.iter().zip(&keep) {
        if !*k_ {
            for blkd in &row.blocks {
                let e = b.el(
                    blkd.id,
                    "block",
                    "dropped",
                    [0.0, 0.0, 0.0, 0.0],
                    s,
                    json!({"row": row.name, "priority": row.prio}),
                );
                b.els[e].drawn = false;
                b.els[e].reason =
                    Some("dropped: does not fit this sheet at the minimum scale".into());
            }
        }
    }
    let live: Vec<&Row> = all
        .iter()
        .zip(&keep)
        .filter(|(_, k)| **k)
        .map(|(r, _)| r)
        .collect();
    if !live.is_empty() && !overflow {
        let allow_opt = live.iter().any(|r| fit_row(r, s, cw, false).is_none());
        let fits: Vec<(Vec<bool>, f64, f64)> = live
            .iter()
            .filter_map(|r| fit_row(r, s, cw, allow_opt))
            .collect();
        let sum_h: f64 = fits.iter().map(|d| d.2).sum();
        let n = live.len();
        let gap = if n > 1 {
            ((ch - sum_h) / (n as f64 - 1.0)).clamp(GAP_MIN, GAP_MAX)
        } else {
            0.0
        };
        let used = sum_h + gap * (n as f64 - 1.0);
        let spare = (ch - used).max(0.0);
        // top band at the top, bottom band flush at the bottom, middle rows centred between them
        let mut y = inner[1];
        for (i, (row, d)) in live.iter().zip(&fits).enumerate() {
            if i == 1 {
                y += spare / 2.0;
            }
            if row.name == "band_bottom" {
                y = inner[3] - d.2;
            }
            for (bi, drop) in d.0.iter().enumerate() {
                if *drop {
                    let e = b.el(
                        row.blocks[bi].id,
                        "block",
                        "dropped",
                        [0.0, 0.0, 0.0, 0.0],
                        s,
                        json!({"row": row.name, "optional": true}),
                    );
                    b.els[e].drawn = false;
                    b.els[e].reason =
                        Some("optional block dropped: row too wide for this sheet".into());
                }
            }
            place_row(&mut b, row, s, [inner[0], y, inner[2], y + d.2], &d.0);
            y += d.2 + gap;
        }
    } else if !live.is_empty() {
        notes.push("rows not placed".to_string());
    }

    // ---- outline samples away from content -------------------------------------------------------------
    let rects: Vec<R4> = b
        .els
        .iter()
        .filter(|e| e.drawn && e.group != "print_marks")
        .map(|e| e.rect)
        .chain([frame])
        .collect();
    for e in outline_els {
        let r = b.els[e].rect;
        let mut got = 0;
        let mut t = 2.0;
        // walk the right edge and bottom edge in the first dash of each 6 mm period
        let mut cands: Vec<[f64; 2]> = vec![];
        while t < r[3] - 2.0 {
            cands.push([r[2] - 0.1, 0.1 + t]);
            t += 6.0;
        }
        t = 2.0;
        while t < r[2] - 2.0 {
            cands.push([r[2] - 0.1 - t, r[3] - 0.1]);
            t += 6.0;
        }
        for c in cands {
            let near = rects.iter().any(|q| {
                c[0] > q[0] - 2.0
                    && c[0] < q[2] + 2.0
                    && c[1] > q[1] - 2.0
                    && c[1] < q[3] + 2.0
                    && !(q[0] <= 0.0 && q[1] <= 0.0)
            });
            let touches_other_outline = false;
            if !near && !touches_other_outline && got < 3 {
                b.sample(e, c[0], c[1], OUTLINE_RGB, 200);
                got += 1;
            }
        }
    }

    // ---- assemble ----------------------------------------------------------------------------------------
    let _ = &b.gray;
    let _ = (b.w, b.h);
    Layout {
        spec: spec.clone(),
        els: b.els,
        prims: b.prims,
        qr_payload: spec.qr_payload(),
        scale: s,
        frame,
        content,
        notes,
    }
}

fn place_row(b: &mut B, row: &Row, s: f64, r: R4, dropped: &[bool]) {
    let blocks: Vec<&Blk> = row
        .blocks
        .iter()
        .enumerate()
        .filter(|(i, _)| !dropped[*i])
        .map(|(_, x)| x)
        .collect();
    let n = blocks.len();
    let avail = r[2] - r[0];
    let sc_of = |x: &Blk| if x.scale { s } else { 1.0 };
    let fixed: f64 = blocks
        .iter()
        .filter(|x| !x.fill)
        .map(|x| x.w * sc_of(x))
        .sum();
    let nfill = blocks.iter().filter(|x| x.fill).count();
    let fill_min: f64 = blocks
        .iter()
        .filter(|x| x.fill)
        .map(|x| x.w * sc_of(x))
        .sum();
    let (gap, fill_w, x_start) = if nfill > 0 {
        let rest = avail - fixed - GAP_MIN * (n as f64 - 1.0);
        (
            GAP_MIN,
            (rest / nfill as f64).max(fill_min / nfill as f64),
            r[0],
        )
    } else if n > 1 {
        ((avail - fixed) / (n as f64 - 1.0), 0.0, r[0])
    } else {
        (0.0, 0.0, r[0] + (avail - fixed) / 2.0)
    };
    let mut x = x_start;
    for blkd in blocks {
        let sc = sc_of(blkd);
        let bw = if blkd.fill { fill_w } else { blkd.w * sc };
        let bh = blkd.h * sc;
        let rect = [
            x,
            r[1],
            x + bw,
            r[1] + if blkd.fill { r[3] - r[1] } else { bh },
        ];
        (blkd.draw)(b, rect, s);
        x += bw + gap;
    }
}

/// Layout + raster for a spec.
pub fn render(spec: &PageSpec) -> (RgbImage, Layout) {
    let layout = build_layout(spec);
    (rasterize(&layout), layout)
}

/// PNG with sRGB + gAMA declared and a pHYs chunk for the physical size (pixels per metre).
pub fn png_bytes(img: &RgbImage, dpi: u32) -> anyhow::Result<Vec<u8>> {
    let base = crate::diag_v2::png_bytes(img)?;
    // insert pHYs right after the IHDR chunk (33 bytes), before gAMA / sRGB
    let ppm = (dpi as f64 / 0.0254).round() as u32;
    let mut data = ppm.to_be_bytes().to_vec();
    data.extend(ppm.to_be_bytes());
    data.push(1);
    let mut body = b"pHYs".to_vec();
    body.extend(&data);
    let mut out = base[..33].to_vec();
    out.extend((data.len() as u32).to_be_bytes());
    out.extend(&body);
    out.extend(crate::diag_v2::crc32(&body).to_be_bytes());
    out.extend(&base[33..]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(paper: &str, landscape: bool, dpi: u32) -> PageSpec {
        PageSpec::from_paper(paper, landscape, dpi).unwrap()
    }

    /// Every drawn element's samples match the raster within tolerance.
    fn check_samples(sp: &PageSpec) -> usize {
        let (img, lay) = render(sp);
        let k = sp.k();
        let mut n = 0;
        for e in lay.json()["elements"].as_array().unwrap() {
            if !e["drawn"].as_bool().unwrap() {
                continue;
            }
            for s in e["samples"].as_array().unwrap() {
                let (x, y) = (s["x_mm"].as_f64().unwrap(), s["y_mm"].as_f64().unwrap());
                let (px, py) = ((x * k).floor() as u32, (y * k).floor() as u32);
                assert!(
                    px < img.width() && py < img.height(),
                    "{} sample off sheet ({x},{y}) on {:?}",
                    e["id"],
                    sp.paper
                );
                let got = img.get_pixel(px, py).0;
                let want: Vec<i64> = s["rgb"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_i64().unwrap())
                    .collect();
                let tol = s["tol"].as_i64().unwrap();
                for c in 0..3 {
                    assert!(
                        (got[c] as i64 - want[c]).abs() <= tol,
                        "{} @({x:.2},{y:.2}) mm {:?} dpi {}: got {got:?} want {want:?} tol {tol}",
                        e["id"],
                        sp.paper,
                        sp.dpi
                    );
                }
                n += 1;
            }
        }
        n
    }

    #[test]
    fn samples_match_pixels_at_several_sheet_sizes_and_dpis() {
        for (paper, land, dpi) in [
            ("A4", false, 100),
            ("A4", false, 150),
            ("A4", false, 200),
            ("LETTER", false, 100),
            ("A5", false, 150),
            ("A3", false, 100),
            ("LEGAL", false, 100),
            ("A4", true, 100),
            ("TABLOID", false, 75),
            ("A6", false, 150),
            ("210x297", false, 100),
            ("B5", false, 100),
        ] {
            let n = check_samples(&spec(paper, land, dpi));
            assert!(n > 40, "{paper} only {n} samples");
        }
        // custom mm and a banner-reserved scanner-like sheet
        let mut sp = PageSpec::from_px("custom", 816, 1144, 100);
        sp.banner_px = banner_height_px(816, 1144);
        sp.side = Some(PageSide::Back);
        sp.page = 5;
        sp.total = 9;
        sp.job = sanitize_job("job-7");
        sp.bleed_mm = 3.0;
        assert!(check_samples(&sp) > 100);
        let mut g = spec("A4", false, 100);
        g.gray = true;
        assert!(check_samples(&g) > 100);
    }

    #[test]
    fn deterministic_and_payload_regenerates_the_page() {
        let mut sp = spec("A4", false, 100);
        sp.page = 2;
        sp.copy = 1;
        sp.total = 4;
        sp.side = Some(PageSide::Front);
        sp.job = "T1".into();
        let (a, la) = render(&sp);
        let (b, lb) = render(&sp);
        assert_eq!(a.as_raw(), b.as_raw());
        assert_eq!(la.json().to_string(), lb.json().to_string());
        let back = PageSpec::from_qr_payload(&sp.qr_payload()).unwrap();
        assert_eq!(back.paper, sp.paper);
        let (c, _) = render(&back);
        assert_eq!(a.as_raw(), c.as_raw());
    }

    #[test]
    fn qr_decodes_and_carries_the_parameters() {
        for (paper, dpi) in [("A4", 100u32), ("A4", 150), ("LETTER", 100), ("A5", 150)] {
            let mut sp = spec(paper, false, dpi);
            sp.page = 3;
            sp.job = "JOB9".into();
            let (img, lay) = render(&sp);
            let q = lay.els.iter().find(|e| e.id == "qr").unwrap();
            assert!(q.drawn);
            let gray = image::imageops::grayscale(&img);
            let mut prep = rqrr::PreparedImage::prepare(gray);
            let grids = prep.detect_grids();
            assert!(!grids.is_empty(), "no QR on {paper}@{dpi}");
            let (_, text) = grids[0].decode().unwrap();
            assert_eq!(text, sp.qr_payload());
            assert_eq!(PageSpec::from_qr_payload(&text).unwrap().page, 3);
        }
    }

    #[test]
    fn side_and_page_change_the_marker_sets() {
        let mut f = spec("A4", false, 100);
        f.side = Some(PageSide::Front);
        let mut bk = f.clone();
        bk.side = Some(PageSide::Back);
        assert_ne!(render(&f).0.as_raw(), render(&bk).0.as_raw());
        let lf = build_layout(&f).json();
        let lb = build_layout(&bk).json();
        let g = |l: &Value, id: &str| {
            l["elements"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["id"] == id)
                .unwrap()["expected"]["glyph"]
                .clone()
        };
        assert_ne!(g(&lf, "orient_tl"), g(&lb, "orient_tl"));
        let mut p = f.clone();
        p.page = 6;
        let l = build_layout(&p).json();
        let bars = l["elements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["id"] == "order_bars")
            .unwrap();
        assert_eq!(bars["expected"]["filled"], 7);
    }

    #[test]
    fn small_sheets_drop_rows_and_record_them() {
        let tiny = build_layout(&spec("A6", false, 100)).json();
        let dropped = tiny["elements"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["group"] == "dropped")
            .count();
        let big = build_layout(&spec("A3", false, 100)).json();
        assert!(
            big["scale"].as_f64().unwrap()
                > build_layout(&spec("A5", false, 100)).json()["scale"]
                    .as_f64()
                    .unwrap()
        );
        let _ = dropped;
        // mandatory elements exist on every sheet
        for p in ["A6", "A5", "A4", "A3", "LEGAL"] {
            let l = build_layout(&spec(p, false, 100)).json();
            for id in [
                "qr",
                "printable_frame",
                "orient_tl",
                "reg_tl",
                "page_number",
            ] {
                assert!(
                    l["elements"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|e| e["id"] == id && e["drawn"] == true),
                    "{p} lacks {id}"
                );
            }
        }
    }

    #[test]
    fn paper_names_and_custom_sizes_parse() {
        assert_eq!(paper_size_mm("a4"), Some((210.0, 297.0)));
        assert_eq!(paper_size_mm("US Letter"), Some((215.9, 279.4)));
        assert_eq!(paper_size_mm("jis_b5"), Some((182.0, 257.0)));
        assert_eq!(paper_size_mm("150x100"), Some((150.0, 100.0)));
        assert!(paper_size_mm("nope").is_none());
    }

    fn fnv64(bytes: &[u8]) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in bytes {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h
    }

    /// Pins: the generator is a pure function of its parameters; a change here is a layout change.
    /// Layout 3 revision 1 (ArUco markers, dual-unit rulers, boards); revision 0 pins were f6aa3fda618d9b02 (A4@100),
    /// beeebc4399ab038c (Letter@100) and 6aaaebb13625058a (A3@50).
    #[test]
    fn pinned_hashes() {
        for (paper, dpi, want) in [
            ("A4", 100u32, 0xf861_1b69_1b27_e9bcu64),
            ("LETTER", 100, 0x9e2c_a63d_82d8_2af6),
            ("A3", 50, 0x6200_ed7d_e6f9_1e9e),
        ] {
            let (img, _) = render(&spec(paper, false, dpi));
            assert_eq!(fnv64(img.as_raw()), want, "{paper}@{dpi}");
        }
    }

    fn el<'a>(lay: &'a Value, id: &str) -> &'a Value {
        lay["elements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["id"] == id)
            .unwrap_or_else(|| panic!("no element {id}"))
    }

    fn f(v: &Value) -> f64 {
        v.as_f64().unwrap()
    }

    fn luma(img: &RgbImage, x: u32, y: u32) -> i64 {
        img.get_pixel(x, y).0[1] as i64
    }

    const SHEETS: [(&str, bool, u32); 8] = [
        ("A4", false, 100),
        ("A4", false, 150),
        ("A4", false, 300),
        ("LETTER", false, 100),
        ("A3", false, 75),
        ("A5", false, 150),
        ("LEGAL", false, 100),
        ("A4", true, 100),
    ];

    #[test]
    fn aruco_markers_are_distinct_documented_and_match_pixels() {
        for (paper, land, dpi) in SHEETS {
            let sp = spec(paper, land, dpi);
            let (img, lay) = render(&sp);
            let l = lay.json();
            assert_eq!(l["layout_revision"], LAYOUT_REVISION);
            let mut ids = vec![];
            for e in l["elements"].as_array().unwrap() {
                if e["kind"] != "marker" || e["drawn"] != true {
                    continue;
                }
                let x = &e["expected"];
                assert_eq!(x["dictionary"], "DICT_4X4_50");
                let id = x["marker_id"].as_u64().unwrap() as usize;
                ids.push(id);
                let rect = &e["rect_mm"];
                let (x0, y0, x1, y1) = (f(&rect[0]), f(&rect[1]), f(&rect[2]), f(&rect[3]));
                let cell = f(&x["module_mm"]);
                assert!((x1 - x0 - 6.0 * cell).abs() < 1e-9 && (y1 - y0 - 6.0 * cell).abs() < 1e-9);
                assert!(
                    cell >= fid::MIN_MODULE_MM * 0.7 - 1e-9,
                    "{paper} id {id} module {cell}"
                );
                assert_eq!(x["quiet_modules"], 1);
                // corners: TL, TR, BR, BL
                let c = x["corners_mm"].as_array().unwrap();
                assert_eq!(c.len(), 4);
                assert!((f(&c[0][0]) - x0).abs() < 1e-9 && (f(&c[2][0]) - x1).abs() < 1e-9);
                assert!((f(&c[0][1]) - y0).abs() < 1e-9 && (f(&c[2][1]) - y1).abs() < 1e-9);
                let k = sp.k();
                if cell * k < 3.0 {
                    continue;
                }
                let cells = fid::aruco_cells(id);
                for (ri, row) in cells.iter().enumerate() {
                    for (ci, &black) in row.iter().enumerate() {
                        let (mx, my) =
                            (x0 + (ci as f64 + 0.5) * cell, y0 + (ri as f64 + 0.5) * cell);
                        let v = luma(&img, (mx * k) as u32, (my * k) as u32);
                        if black {
                            assert!(v < 90, "{paper}@{dpi} id {id} module {ri},{ci}: {v}");
                        } else {
                            assert!(v >= 120, "{paper}@{dpi} id {id} module {ri},{ci}: {v}");
                        }
                    }
                }
                // one module of paper around the marker (a dashed grey outline may cross it: >= 100)
                let q = cell;
                let px = |mm: f64| (mm * k).round() as i64;
                let (a0, b0, a1, b1) = (
                    px(x0 - q) + 1,
                    px(y0 - q) + 1,
                    px(x1 + q) - 1,
                    px(y1 + q) - 1,
                );
                let (i0, j0, i1, j1) = (
                    (x0 * k).floor() as i64 - 1,
                    (y0 * k).floor() as i64 - 1,
                    (x1 * k).ceil() as i64,
                    (y1 * k).ceil() as i64,
                );
                for y in b0..b1 {
                    for xx in a0..a1 {
                        if xx > i0 && xx < i1 && y > j0 && y < j1 {
                            continue;
                        }
                        assert!(
                            luma(&img, xx as u32, y as u32) >= 100,
                            "{paper}@{dpi} quiet zone of id {id} is not paper at px {xx},{y}"
                        );
                    }
                }
            }
            let board_ids: Vec<usize> = ids.iter().copied().filter(|i| *i >= 8).collect();
            let mut sorted = ids.clone();
            sorted.sort();
            sorted.dedup();
            assert_eq!(sorted.len(), ids.len(), "{paper}: duplicate marker ids");
            for want in 0..4 {
                assert!(ids.contains(&want), "{paper} lacks corner marker {want}");
            }
            if !land && (paper == "A4" || paper == "LETTER" || paper == "A3") {
                for want in 0..8 {
                    assert!(ids.contains(&want), "{paper} lacks marker {want}");
                }
                assert_eq!(board_ids.len(), 12, "{paper} ChArUco markers");
            }
        }
    }

    #[test]
    fn marker_edge_ids_and_positions() {
        let l = build_layout(&spec("A4", false, 100)).json();
        for (id, aid) in [
            ("marker_tl", 0),
            ("marker_tr", 1),
            ("marker_br", 2),
            ("marker_bl", 3),
            ("marker_tm", 4),
            ("marker_mr", 5),
            ("marker_bm", 6),
            ("marker_ml", 7),
        ] {
            let e = el(&l, id);
            assert_eq!(e["drawn"], true, "{id}");
            assert_eq!(e["expected"]["marker_id"], aid, "{id}");
        }
        // edge midpoints are centred on the sheet axes
        let cx = f(&l["box_mm"][0]) / 2.0;
        for id in ["marker_tm", "marker_bm"] {
            let r = &el(&l, id)["rect_mm"];
            assert!(((f(&r[0]) + f(&r[2])) / 2.0 - cx).abs() < 0.01);
        }
    }

    #[test]
    fn ruler_ticks_match_the_rendering_at_several_dpis_and_papers() {
        let mut checked = 0usize;
        for (paper, land, dpi) in SHEETS {
            let sp = spec(paper, land, dpi);
            if dpi < 100 {
                continue;
            }
            let (img, lay) = render(&sp);
            let l = lay.json();
            let k = sp.k();
            for e in l["elements"].as_array().unwrap() {
                if e["kind"] != "ruler" || e["drawn"] != true {
                    continue;
                }
                let x = &e["expected"];
                let horizontal = x["axis"] == "x";
                let pitch = f(&x["pitch_mm"]);
                let (a0, edge) = (f(&x["zero_line_mm"]), f(&x["tick_edge_mm"]));
                let d = if matches!(x["ticks_point"].as_str().unwrap(), "down" | "right") {
                    1.0
                } else {
                    -1.0
                };
                let cross = edge + d * 0.7;
                let pos = x["ticks"]["pos_mm"].as_array().unwrap();
                let len = x["ticks"]["length_mm"].as_array().unwrap();
                assert_eq!(pos.len(), len.len());
                assert!(!pos.is_empty());
                // dashed grey media outlines may cross a ruler: skip ticks next to their edges
                let edges: Vec<[f64; 2]> = l["elements"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|o| {
                        o["drawn"] == true && o["id"].as_str().unwrap().starts_with("outline_")
                    })
                    .map(|o| [f(&o["rect_mm"][2]), f(&o["rect_mm"][3])])
                    .collect();
                let near = |along: f64, across: f64| {
                    edges.iter().any(|q| {
                        let (qa, qc) = if horizontal {
                            (q[0], q[1])
                        } else {
                            (q[1], q[0])
                        };
                        (along - qa).abs() < 1.5 || (across - qc).abs() < 1.5
                    })
                };
                for (p, ln) in pos.iter().zip(len) {
                    let a = a0 + f(p);
                    if near(a, cross) {
                        continue;
                    }
                    // centroid of darkness around the tick along the axis
                    let (mut sw, mut swx) = (0.0, 0.0);
                    let lo = ((a - pitch / 2.0) * k).ceil() as i64;
                    let hi = ((a + pitch / 2.0) * k).floor() as i64;
                    for i in lo..hi {
                        let (ix, iy) = if horizontal {
                            (i, (cross * k) as i64)
                        } else {
                            ((cross * k) as i64, i)
                        };
                        let w = (255 - luma(&img, ix as u32, iy as u32)) as f64;
                        sw += w;
                        swx += w * ((i as f64 + 0.5) / k);
                    }
                    assert!(sw > 0.0, "{paper}@{dpi} {} no ink at tick {p}", e["id"]);
                    let c = swx / sw;
                    assert!(
                        (c - a).abs() < 0.5 * 25.4 / dpi as f64 + 0.03,
                        "{paper}@{dpi} {} tick at {a:.3} mm measured {c:.3}",
                        e["id"]
                    );
                    assert!(f(ln) >= 1.2 - 1e-9);
                    checked += 1;
                }
                // tick pitch from the table is exactly 1 mm or 25.4/16 mm
                let unit = x["unit"].as_str().unwrap();
                let want = if unit == "mm" { 1.0 } else { 25.4 / 16.0 };
                assert!((pitch - want).abs() < 1e-12);
                for w in x["ticks"]["pos_mm"].as_array().unwrap().windows(2) {
                    let step = f(&w[1]) - f(&w[0]);
                    assert!(step > 0.0 && (step / pitch - (step / pitch).round()).abs() < 1e-9);
                }
            }
        }
        assert!(checked > 2000, "only {checked} ticks checked");
    }

    #[test]
    fn rulers_are_dual_sided_on_both_axes_from_one_zero_line() {
        let l = build_layout(&spec("A4", false, 100)).json();
        for (m, i, axis) in [
            ("ruler_metric", "ruler_inch", "x"),
            ("ruler_v_metric", "ruler_v_inch", "y"),
            ("ruler_vr_metric", "ruler_vr_inch", "y"),
        ] {
            let (em, ei) = (el(&l, m), el(&l, i));
            assert_eq!(em["drawn"], true);
            assert_eq!(ei["drawn"], true);
            let (xm, xi) = (&em["expected"], &ei["expected"]);
            assert_eq!(xm["axis"], axis);
            assert_eq!(xi["axis"], axis);
            // same zero line, opposite edges
            assert!((f(&xm["zero_line_mm"]) - f(&xi["zero_line_mm"])).abs() < 1e-9);
            assert!((f(&xm["tick_edge_mm"]) - f(&xi["tick_edge_mm"])).abs() > 5.0);
            assert_eq!(xm["tiers"], json!([[1, 1.5], [5, 2.5], [10, 3.5]]));
            assert_eq!(
                xi["tiers"],
                json!([[1, 1.2], [2, 1.8], [4, 2.4], [8, 3.0], [16, 3.6]])
            );
            assert!(f(&xm["span_mm"]) >= 100.0);
        }
        assert!(f(&el(&l, "ruler_v_metric")["expected"]["span_mm"]) >= 200.0);
    }

    #[test]
    fn rulers_parameter_selects_the_scales_and_rides_the_qr() {
        for (r, metric, inch) in [
            (Rulers::Both, true, true),
            (Rulers::Metric, true, false),
            (Rulers::Inch, false, true),
        ] {
            let mut sp = spec("A4", false, 100);
            sp.rulers = r;
            let l = build_layout(&sp).json();
            assert_eq!(l["spec"]["rulers"], r.as_str());
            for (m, i) in [
                ("ruler_metric", "ruler_inch"),
                ("ruler_v_metric", "ruler_v_inch"),
            ] {
                assert_eq!(el(&l, m)["drawn"], metric);
                assert_eq!(el(&l, i)["drawn"], inch);
            }
            assert!(sp.qr_payload().contains(&format!("|rl={}|", r.as_str())));
            assert_eq!(
                PageSpec::from_qr_payload(&sp.qr_payload()).unwrap().rulers,
                r
            );
            let (a, _) = render(&sp);
            let (b, _) = render(&PageSpec::from_qr_payload(&sp.qr_payload()).unwrap());
            assert_eq!(a.as_raw(), b.as_raw());
            check_samples(&sp);
        }
    }

    #[test]
    fn qr_payload_names_the_revision_dictionary_and_ids_and_old_payloads_still_parse() {
        let sp = spec("A4", false, 100);
        let p = sp.qr_payload();
        assert!(p.contains("|lay=3|") && p.contains("|rev=1|") && p.contains("|ar=4x4_50|"));
        assert!(p.contains("|id=0-19"));
        let old = "fakescan|v1|page=2|pat=diag|side=front|lay=3|paper=A4|px=816x1144|dpi=100|m=4|b=0|copy=1|of=4|bn=44|g=0|job=AB-1";
        let s = PageSpec::from_qr_payload(old).unwrap();
        assert_eq!((s.page, s.dpi, s.rulers), (2, 100, Rulers::Both));
        assert_eq!(s.job, "AB-1");
    }

    #[test]
    fn calibration_boards_are_described_exactly() {
        let l = build_layout(&spec("A4", false, 150)).json();
        let ch = el(&l, "chessboard");
        assert_eq!(ch["drawn"], true);
        assert_eq!(ch["expected"]["inner_corners"], json!([8, 5]));
        assert_eq!(
            ch["expected"]["inner_corner_positions_mm"]
                .as_array()
                .unwrap()
                .len(),
            40
        );
        let cb = el(&l, "charuco_board");
        assert_eq!(cb["drawn"], true);
        let ids: Vec<u64> = cb["expected"]["marker_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap())
            .collect();
        assert_eq!(ids, (8..20).collect::<Vec<u64>>());
        assert_eq!(cb["expected"]["corner_count"], 15);
        assert_eq!(cb["expected"]["legacy_pattern"], false);
        let cg = el(&l, "circle_grid");
        assert_eq!(cg["drawn"], true);
        assert_eq!(cg["expected"]["points"], 30);
        assert_eq!(cg["expected"]["pattern"], "asymmetric");
        // squares are at least the documented minimum
        assert!(f(&ch["expected"]["square_mm"]) >= fid::MIN_SQUARE_MM);
        // every circle has a vector primitive
        let n = l["primitives"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p["id"] == "circle_grid" && p["op"] == "circle" && p["vector"] == true)
            .count();
        assert_eq!(n, 30);
        // markers and boards are vector only (rect / polygon / circle): no raster-only op carries them
        for p in l["primitives"].as_array().unwrap() {
            let id = p["id"].as_str().unwrap();
            if id.starts_with("marker_")
                || id.starts_with("charuco")
                || id.starts_with("bracket_")
                || id == "chessboard"
                || id.starts_with("ruler")
            {
                assert_eq!(p["vector"], true, "{id}");
            }
        }
    }

    #[test]
    fn small_sheets_drop_the_columns_and_boards_with_a_reason() {
        let l = build_layout(&spec("A6", false, 100)).json();
        for id in ["marker_ml", "marker_mr", "ruler_v_metric", "bracket_tl"] {
            let e = el(&l, id);
            assert_eq!(e["drawn"], false, "{id}");
            assert!(e["reason"].as_str().unwrap().contains("too small"), "{id}");
        }
        for e in l["elements"].as_array().unwrap() {
            if e["drawn"] == false {
                assert!(e["reason"].is_string(), "{} lacks a reason", e["id"]);
            }
        }
    }

    #[test]
    fn markers_render_as_vector_rects_without_seams() {
        // a marker module edge at a fractional pixel must not leave a lighter seam inside a black run
        let sp = spec("A4", false, 100);
        let (img, lay) = render(&sp);
        let l = lay.json();
        let e = el(&l, "marker_tl");
        let r = &e["rect_mm"];
        let k = sp.k();
        let cell = f(&e["expected"]["module_mm"]);
        // the top border row of 6 black modules is one run: every pixel inside is fully black
        let y = (f(&r[1]) + cell * 0.5) * k;
        for xi in ((f(&r[0]) + 1.0 / k) * k) as u32..((f(&r[2]) - 1.0 / k) * k) as u32 {
            assert_eq!(luma(&img, xi, y as u32), 0, "seam at {xi}");
        }
    }

    #[test]
    fn write_v3_pngs() {
        if let Ok(dir) = std::env::var("FAKESCAN_DIAG3_DIR") {
            for (name, paper, land, dpi) in [
                ("a4", "A4", false, 100),
                ("letter", "LETTER", false, 100),
                ("a5", "A5", false, 100),
                ("a3", "A3", false, 60),
                ("a4-landscape", "A4", true, 100),
            ] {
                let mut sp = spec(paper, land, dpi);
                sp.total = 3;
                sp.job = "DEMO".into();
                let (img, _) = render(&sp);
                std::fs::write(
                    format!("{dir}/diag-v3-{name}.png"),
                    png_bytes(&img, dpi).unwrap(),
                )
                .unwrap();
            }
        }
    }
}
