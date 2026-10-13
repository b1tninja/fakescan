//! Post-pattern image defects (skew, speckles, streaks, hole punches, …).

use image::{Rgb, RgbImage};
use serde::{Deserialize, Serialize};

/// Color mode applied after effects (from ScanTicket ColorProcessing).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorMode {
    #[default]
    Rgb24,
    Gray8,
    /// Threshold bilevel (still RGB channels; black or white only).
    Bw1,
}

impl ColorMode {
    pub fn as_str(self) -> &'static str {
        match self {
            ColorMode::Rgb24 => "RGB24",
            ColorMode::Gray8 => "Gray8",
            ColorMode::Bw1 => "BW1",
        }
    }

    pub fn parse(s: &str) -> Self {
        let lc = s.trim().to_ascii_lowercase();
        match lc.as_str() {
            "grayscale8" | "grayscale" | "gray8" | "grey8" | "gray" | "grey" => ColorMode::Gray8,
            "bw1" | "blackandwhite1" | "blackandwhite" | "bilevel" | "binary" => ColorMode::Bw1,
            _ => ColorMode::Rgb24,
        }
    }

    /// WSD ColorProcessing / ColorEntry token for CreateScanJob / ImageInformation.
    /// Wire enum is `BlackAndWhite1` (not `BW1`) — see ColorEntry on learn.microsoft.com.
    pub fn wsd_token(self) -> &'static str {
        match self {
            ColorMode::Rgb24 => "RGB24",
            ColorMode::Gray8 => "Grayscale8",
            ColorMode::Bw1 => "BlackAndWhite1",
        }
    }
}

/// Photometric polarity of 1-bit (BlackAndWhite1) output.
///
/// Real scanners differ: some write BMP palette `[black, white]` / TIFF
/// `MinIsBlack`, others `[white, black]` / `MinIsWhite`. The visual image is
/// identical either way; only the stored bit sense and the header flip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BilevelPolarity {
    /// Bit 0 = black (BMP palette `[black, white]`, TIFF Photometric 1).
    #[default]
    BlackIsZero,
    /// Bit 0 = white (BMP palette `[white, black]`, TIFF Photometric 0).
    WhiteIsZero,
}

impl BilevelPolarity {
    pub fn as_str(self) -> &'static str {
        match self {
            BilevelPolarity::BlackIsZero => "black_is_zero",
            BilevelPolarity::WhiteIsZero => "white_is_zero",
        }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "black_is_zero" | "blackiszero" | "minisblack" | "min_is_black" | "" => {
                Ok(BilevelPolarity::BlackIsZero)
            }
            "white_is_zero" | "whiteiszero" | "miniswhite" | "min_is_white" => {
                Ok(BilevelPolarity::WhiteIsZero)
            }
            other => Err(format!(
                "unknown bilevel_polarity {other:?}; expected black_is_zero or white_is_zero"
            )),
        }
    }
}

/// How a Grayscale8 ticket is answered when the Format is JFIF / JPEG / EXIF.
///
/// Live proof (2026-10-12, Windows 11 inbox `wsdscdrv`): answering a gray ticket
/// with ColorProcessing RGB24 makes Windows accept CreateScanJob and then never
/// call RetrieveImage ("feeder has no paper"); the consistent gray answer works.
/// So `Gray8` is the default. This only changes the *answer* (FinalParameters /
/// ValidScanTicket / delivered bytes); the published ScannerConfiguration never
/// changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JfifGray {
    /// Legacy opt-in: ColorProcessing coerced to RGB24 (Override), BytesPerLine =
    /// width x 3, 3-component JPEG. Known to be rejected by the Windows inbox
    /// driver; kept for tests of the old coercion.
    Rgb24,
    /// Default. Consistent gray: ColorProcessing Grayscale8, BytesPerLine = width,
    /// and a genuine 1-component 8-bit gray JPEG (EXIF keeps its APP1).
    #[default]
    Gray8,
    /// Gray8 ColorProcessing and gray JPEG, but BytesPerLine = width x 3
    /// (also accepted by Windows in the live test).
    Gray8RgbStride,
    /// Gray ticket asking JFIF/JPEG/EXIF is answered with Format
    /// `tiff-single-uncompressed` (Override) and an 8-bit gray TIFF. Rejected by
    /// Windows in the live test (fails like `rgb24`). Needs the profile to offer TIFF.
    Gray8Tiff,
}

impl JfifGray {
    pub fn as_str(self) -> &'static str {
        match self {
            JfifGray::Rgb24 => "rgb24",
            JfifGray::Gray8 => "gray8",
            JfifGray::Gray8RgbStride => "gray8_rgb_stride",
            JfifGray::Gray8Tiff => "gray8_tiff",
        }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "rgb24" | "rgb" => Ok(JfifGray::Rgb24),
            "gray8" | "grayscale8" | "gray" | "grey8" | "" => Ok(JfifGray::Gray8),
            "gray8_rgb_stride" | "gray8rgbstride" => Ok(JfifGray::Gray8RgbStride),
            "gray8_tiff" | "gray8tiff" | "tiff" => Ok(JfifGray::Gray8Tiff),
            other => Err(format!(
                "unknown jfif_gray {other:?}; expected rgb24, gray8, gray8_rgb_stride or gray8_tiff"
            )),
        }
    }

    /// Gray tickets keep ColorProcessing Grayscale8 on the wire in this mode.
    pub fn keeps_gray_token(self) -> bool {
        !matches!(self, JfifGray::Rgb24)
    }

    /// Delivered JFIF/EXIF is a one-component JPEG.
    pub fn gray_jpeg(self) -> bool {
        matches!(self, JfifGray::Gray8 | JfifGray::Gray8RgbStride)
    }
}

/// How a BlackAndWhite1 ticket is answered when the Format is JFIF / JPEG / EXIF.
///
/// JPEG cannot carry 1-bit samples, so the conforming answer (same kind as for
/// Grayscale8) is a one-component 8-bit image whose samples are the thresholded
/// 0/255 values. Only the *answer* changes; the published ScannerConfiguration
/// never does. TIFF/BMP/PNG/DIB BW answers (real 1-bit) are unaffected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JfifBw {
    /// Legacy Brother-style fallback: ColorProcessing coerced to RGB24 (Override),
    /// BytesPerLine = width x 3, 3-component JPEG with bilevel samples.
    Rgb24,
    /// Default. ColorProcessing Grayscale8 (Override), BytesPerLine = width, and a
    /// genuine 1-component JPEG with thresholded 0/255 samples.
    #[default]
    Gray8,
    /// Spec-faithful: a BlackAndWhite1 ticket with a JPEG-family Format is refused
    /// with ClientErrorDocumentFormatNotSupported (ValidateScanTicket and
    /// CreateScanJob) so the Windows driver falls back to another Format.
    Reject,
}

impl JfifBw {
    pub fn as_str(self) -> &'static str {
        match self {
            JfifBw::Rgb24 => "rgb24",
            JfifBw::Gray8 => "gray8",
            JfifBw::Reject => "reject",
        }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "rgb24" | "rgb" => Ok(JfifBw::Rgb24),
            "gray8" | "grayscale8" | "gray" | "grey8" | "" => Ok(JfifBw::Gray8),
            "reject" | "refuse" | "fault" => Ok(JfifBw::Reject),
            other => Err(format!(
                "unknown jfif_bw {other:?}; expected gray8, rgb24 or reject"
            )),
        }
    }
}

/// Axis for ADF / glass dust streaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreakAxis {
    #[default]
    Vertical,
    Horizontal,
}

impl StreakAxis {
    pub fn as_str(self) -> &'static str {
        match self {
            StreakAxis::Vertical => "vertical",
            StreakAxis::Horizontal => "horizontal",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "horizontal" | "h" | "row" => StreakAxis::Horizontal,
            _ => StreakAxis::Vertical,
        }
    }
}

/// Composable post-process knobs applied after pattern draw, before encode.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageEffects {
    /// Deterministic RNG seed (combined with page index).
    pub seed: u64,
    /// Page rotation in degrees (misfeed / skew).
    pub skew_deg: f32,
    /// Dust density 0.0–1.0 (fraction of pixels that get a speck attempt).
    pub speckles: f32,
    /// Number of full-page streak lines.
    pub streaks: u32,
    pub streak_axis: StreakAxis,
    /// Number of binder hole rings on the left edge (0 = none).
    pub hole_punch: u32,
    /// Glass scratch count (thin dark polylines).
    pub scratches: u32,
    /// Box-blur radius in pixels (0 = off).
    pub blur_px: u32,
    /// Brightness multiplier (1.0 = unchanged).
    pub brightness: f32,
    /// Contrast multiplier around mid-gray (1.0 = unchanged).
    pub contrast: f32,
    /// JPEG quality override for jfif/exif/pdf/xps JPEG embeds (0 = default 95).
    pub jpeg_quality: u8,
    /// Horizontal illumination / roller band count (0 = off).
    pub banding: u32,
    /// Additive sensor noise strength 0.0–1.0.
    pub noise: f32,
    /// Edge darkening 0.0–1.0.
    pub vignette: f32,
    /// Crop this many pixels from each edge (autocrop miss).
    pub margin_crop_px: u32,
    /// Duplex show-through / bleed strength 0.0–1.0 (ghost of opposite side).
    pub show_through: f32,
    /// Halftone-style moiré overlay when true.
    pub moire: bool,
    /// Bit sense of 1-bit output (BMP palette order / TIFF Photometric). Not a
    /// pixel defect: the visual stays identical, so it is ignored by `is_clean`.
    #[serde(default)]
    pub bilevel_polarity: BilevelPolarity,
    /// Wire answer for Grayscale8 + JFIF/JPEG/EXIF (see [`JfifGray`]; default `gray8`). Not a pixel
    /// defect: ignored by `is_clean`, kept across presets, cleared by `clear`.
    #[serde(default)]
    pub jfif_gray: JfifGray,
    /// Wire answer for BlackAndWhite1 + JFIF/JPEG/EXIF (see [`JfifBw`]; default `gray8`).
    /// Same status as `jfif_gray`: not a pixel defect.
    #[serde(default)]
    pub jfif_bw: JfifBw,
    /// Scan tickets the device refuses (see [`RefuseRule`]). Not a pixel defect: ignored
    /// by `is_clean`, kept across presets, cleared by `clear`.
    #[serde(default)]
    pub refuse_tickets: Vec<RefuseRule>,
}

/// Default fault for a refused ticket.
pub const REFUSE_DEFAULT_FAULT: &str = "ClientErrorDocumentFormatNotSupported";
/// Fault names `refuse_tickets` may use (each has a wire subcode mapping).
const REFUSE_FAULTS: &[&str] = &[
    "ClientErrorDocumentFormatNotSupported",
    "ClientErrorFormatNotSupported",
    "ClientErrorConflictingRequiredParameters",
    "ClientErrorNoImagesAvailable",
];

/// One `refuse_tickets` rule. A ticket is refused when every field present matches
/// (absent field = wildcard).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RefuseRule {
    /// Wire Format token, e.g. `tiff-single-uncompressed`, `jfif`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// `BlackAndWhite1` | `Grayscale8` | `RGB24`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// `Platen` | `ADF` | `ADFDuplex`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<u32>,
    /// Fault name; default [`REFUSE_DEFAULT_FAULT`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fault: Option<String>,
}

impl RefuseRule {
    pub fn fault_name(&self) -> &str {
        self.fault.as_deref().unwrap_or(REFUSE_DEFAULT_FAULT)
    }

    /// Parse the JSON value of `refuse_tickets` (null / `[]` = clear).
    pub fn parse_list(v: &serde_json::Value) -> Result<Vec<RefuseRule>, String> {
        if v.is_null() {
            return Ok(Vec::new());
        }
        let rules: Vec<RefuseRule> = serde_json::from_value(v.clone()).map_err(|e| {
            format!(
                "invalid refuse_tickets (expected a list of {{format?, color?, input_source?, resolution?, fault?}}): {e}"
            )
        })?;
        for r in &rules {
            if let Some(f) = r.fault.as_deref() {
                if !REFUSE_FAULTS.contains(&f) {
                    return Err(format!(
                        "unknown refuse_tickets fault {f:?}; expected one of {}",
                        REFUSE_FAULTS.join(", ")
                    ));
                }
            }
            if let Some(c) = r.color.as_deref() {
                if !["BlackAndWhite1", "Grayscale8", "RGB24"]
                    .iter()
                    .any(|k| k.eq_ignore_ascii_case(c))
                {
                    return Err(format!(
                        "unknown refuse_tickets color {c:?}; expected BlackAndWhite1, Grayscale8 or RGB24"
                    ));
                }
            }
            if let Some(s) = r.input_source.as_deref() {
                if !["Platen", "ADF", "ADFDuplex"]
                    .iter()
                    .any(|k| k.eq_ignore_ascii_case(s))
                {
                    return Err(format!(
                        "unknown refuse_tickets input_source {s:?}; expected Platen, ADF or ADFDuplex"
                    ));
                }
            }
            if r.format.as_deref().is_some_and(|f| f.trim().is_empty()) {
                return Err("refuse_tickets format must not be empty".into());
            }
        }
        Ok(rules)
    }
}

fn deserialize_some_value<'de, D>(d: D) -> Result<Option<serde_json::Value>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Some(serde::Deserialize::deserialize(d)?))
}

impl Default for ImageEffects {
    fn default() -> Self {
        Self {
            seed: 0,
            skew_deg: 0.0,
            speckles: 0.0,
            streaks: 0,
            streak_axis: StreakAxis::Vertical,
            hole_punch: 0,
            scratches: 0,
            blur_px: 0,
            brightness: 1.0,
            contrast: 1.0,
            jpeg_quality: 0,
            banding: 0,
            noise: 0.0,
            vignette: 0.0,
            margin_crop_px: 0,
            show_through: 0.0,
            moire: false,
            bilevel_polarity: BilevelPolarity::BlackIsZero,
            jfif_gray: JfifGray::Gray8,
            jfif_bw: JfifBw::Gray8,
            refuse_tickets: Vec::new(),
        }
    }
}

impl ImageEffects {
    pub fn is_clean(&self) -> bool {
        self.skew_deg.abs() < 1e-6
            && self.speckles <= 0.0
            && self.streaks == 0
            && self.hole_punch == 0
            && self.scratches == 0
            && self.blur_px == 0
            && (self.brightness - 1.0).abs() < 1e-6
            && (self.contrast - 1.0).abs() < 1e-6
            && self.jpeg_quality == 0
            && self.banding == 0
            && self.noise <= 0.0
            && self.vignette <= 0.0
            && self.margin_crop_px == 0
            && self.show_through <= 0.0
            && !self.moire
    }

    /// Effective JPEG quality (default high for QR readability).
    pub fn effective_jpeg_quality(&self) -> u8 {
        if self.jpeg_quality == 0 {
            95
        } else {
            self.jpeg_quality.clamp(1, 100)
        }
    }

    pub fn summary(&self) -> EffectsSummary {
        EffectsSummary {
            seed: self.seed,
            skew_deg: self.skew_deg,
            speckles: self.speckles,
            streaks: self.streaks,
            streak_axis: self.streak_axis.as_str().to_string(),
            hole_punch: self.hole_punch,
            scratches: self.scratches,
            blur_px: self.blur_px,
            brightness: self.brightness,
            contrast: self.contrast,
            jpeg_quality: self.jpeg_quality,
            banding: self.banding,
            noise: self.noise,
            vignette: self.vignette,
            margin_crop_px: self.margin_crop_px,
            show_through: self.show_through,
            moire: self.moire,
            bilevel_polarity: self.bilevel_polarity.as_str().to_string(),
            jfif_gray: self.jfif_gray.as_str().to_string(),
            jfif_bw: self.jfif_bw.as_str().to_string(),
            refuse_tickets: self.refuse_tickets.clone(),
            clean: self.is_clean(),
        }
    }

    pub fn apply_preset(&mut self, name: &str) -> Result<(), String> {
        let seed = self.seed;
        let polarity = self.bilevel_polarity;
        let jfif_gray = self.jfif_gray;
        let jfif_bw = self.jfif_bw;
        let refuse = std::mem::take(&mut self.refuse_tickets);
        let res = self.apply_preset_inner(name, seed);
        self.jfif_bw = jfif_bw;
        self.refuse_tickets = refuse;
        // Polarity / jfif_gray are format knobs, not defects: presets never reset them.
        self.bilevel_polarity = polarity;
        self.jfif_gray = jfif_gray;
        res
    }

    fn apply_preset_inner(&mut self, name: &str, seed: u64) -> Result<(), String> {
        match name.trim().to_ascii_lowercase().as_str() {
            "clean" => {
                *self = Self::default();
                self.seed = seed;
                Ok(())
            }
            "skewed" => {
                *self = Self {
                    seed,
                    skew_deg: 1.5,
                    ..Self::default()
                };
                Ok(())
            }
            "dirty_adf" | "dirty-adf" => {
                *self = Self {
                    seed: if seed == 0 { 1 } else { seed },
                    speckles: 0.015,
                    streaks: 4,
                    streak_axis: StreakAxis::Vertical,
                    ..Self::default()
                };
                Ok(())
            }
            "punched" => {
                *self = Self {
                    seed,
                    skew_deg: 0.6,
                    hole_punch: 3,
                    ..Self::default()
                };
                Ok(())
            }
            "dirty_glass" | "dirty-glass" => {
                *self = Self {
                    seed: if seed == 0 { 2 } else { seed },
                    speckles: 0.04,
                    scratches: 5,
                    streak_axis: StreakAxis::Horizontal,
                    streaks: 1,
                    ..Self::default()
                };
                Ok(())
            }
            "dim" => {
                *self = Self {
                    seed,
                    brightness: 0.72,
                    contrast: 0.9,
                    ..Self::default()
                };
                Ok(())
            }
            "harsh_jpeg" | "harsh-jpeg" => {
                *self = Self {
                    seed,
                    jpeg_quality: 28,
                    blur_px: 1,
                    ..Self::default()
                };
                Ok(())
            }
            "office_bad" | "office-bad" => {
                *self = Self {
                    seed: if seed == 0 { 7 } else { seed },
                    skew_deg: 0.9,
                    speckles: 0.02,
                    streaks: 3,
                    streak_axis: StreakAxis::Vertical,
                    scratches: 2,
                    brightness: 0.88,
                    contrast: 1.15,
                    jpeg_quality: 45,
                    banding: 2,
                    noise: 0.04,
                    vignette: 0.15,
                    ..Self::default()
                };
                Ok(())
            }
            other => Err(format!(
                "unknown effects preset {other:?}; expected clean, skewed, dirty_adf, punched, dirty_glass, dim, harsh_jpeg, office_bad"
            )),
        }
    }
}

/// Compact status view of active effects.
#[derive(Debug, Clone, Serialize)]
pub struct EffectsSummary {
    pub seed: u64,
    pub skew_deg: f32,
    pub speckles: f32,
    pub streaks: u32,
    pub streak_axis: String,
    pub hole_punch: u32,
    pub scratches: u32,
    pub blur_px: u32,
    pub brightness: f32,
    pub contrast: f32,
    pub jpeg_quality: u8,
    pub banding: u32,
    pub noise: f32,
    pub vignette: f32,
    pub margin_crop_px: u32,
    pub show_through: f32,
    pub moire: bool,
    pub bilevel_polarity: String,
    pub jfif_gray: String,
    pub jfif_bw: String,
    pub refuse_tickets: Vec<RefuseRule>,
    pub clean: bool,
}

/// Body for `POST /v1/effects` (merge; unset keys leave prior values).
#[derive(Debug, Deserialize, Default)]
pub struct EffectsBody {
    pub seed: Option<u64>,
    pub skew_deg: Option<f32>,
    pub speckles: Option<f32>,
    pub streaks: Option<u32>,
    pub streak_axis: Option<String>,
    pub hole_punch: Option<u32>,
    pub scratches: Option<u32>,
    pub blur_px: Option<u32>,
    pub brightness: Option<f32>,
    pub contrast: Option<f32>,
    pub jpeg_quality: Option<u8>,
    pub banding: Option<u32>,
    pub noise: Option<f32>,
    pub vignette: Option<f32>,
    pub margin_crop_px: Option<u32>,
    pub show_through: Option<f32>,
    pub moire: Option<bool>,
    /// `black_is_zero` (default) | `white_is_zero`; see [`BilevelPolarity`].
    pub bilevel_polarity: Option<String>,
    /// `gray8` (default) | `rgb24` (legacy, Windows rejects) | `gray8_rgb_stride` | `gray8_tiff`; see [`JfifGray`].
    pub jfif_gray: Option<String>,
    /// `gray8` (default) | `rgb24` (legacy Brother-style fallback) | `reject`; see [`JfifBw`].
    pub jfif_bw: Option<String>,
    /// List of `{format?, color?, input_source?, resolution?, fault?}` rules; `[]` or `null`
    /// clears. Kept as raw JSON so a malformed rule is a 400 with a clear message.
    #[serde(default, deserialize_with = "deserialize_some_value")]
    pub refuse_tickets: Option<serde_json::Value>,
    pub preset: Option<String>,
    #[serde(default)]
    pub clear: bool,
}

impl ImageEffects {
    pub fn apply_body(&mut self, body: &EffectsBody) -> Result<(), String> {
        if body.clear {
            *self = Self::default();
        }
        if let Some(ref preset) = body.preset {
            self.apply_preset(preset)?;
        }
        if let Some(seed) = body.seed {
            self.seed = seed;
        }
        if let Some(v) = body.skew_deg {
            self.skew_deg = v;
        }
        if let Some(v) = body.speckles {
            self.speckles = v.clamp(0.0, 1.0);
        }
        if let Some(v) = body.streaks {
            self.streaks = v.min(64);
        }
        if let Some(ref axis) = body.streak_axis {
            self.streak_axis = StreakAxis::parse(axis);
        }
        if let Some(v) = body.hole_punch {
            self.hole_punch = v.min(8);
        }
        if let Some(v) = body.scratches {
            self.scratches = v.min(32);
        }
        if let Some(v) = body.blur_px {
            self.blur_px = v.min(16);
        }
        if let Some(v) = body.brightness {
            self.brightness = v.clamp(0.1, 3.0);
        }
        if let Some(v) = body.contrast {
            self.contrast = v.clamp(0.1, 3.0);
        }
        if let Some(v) = body.jpeg_quality {
            self.jpeg_quality = if v == 0 { 0 } else { v.clamp(1, 100) };
        }
        if let Some(v) = body.banding {
            self.banding = v.min(32);
        }
        if let Some(v) = body.noise {
            self.noise = v.clamp(0.0, 1.0);
        }
        if let Some(v) = body.vignette {
            self.vignette = v.clamp(0.0, 1.0);
        }
        if let Some(v) = body.margin_crop_px {
            self.margin_crop_px = v.min(200);
        }
        if let Some(v) = body.show_through {
            self.show_through = v.clamp(0.0, 1.0);
        }
        if let Some(v) = body.moire {
            self.moire = v;
        }
        if let Some(ref v) = body.bilevel_polarity {
            self.bilevel_polarity = BilevelPolarity::parse(v)?;
        }
        if let Some(ref v) = body.jfif_gray {
            self.jfif_gray = JfifGray::parse(v)?;
        }
        if let Some(ref v) = body.jfif_bw {
            self.jfif_bw = JfifBw::parse(v)?;
        }
        if let Some(ref v) = body.refuse_tickets {
            self.refuse_tickets = RefuseRule::parse_list(v)?;
        }
        Ok(())
    }
}

fn pixel_effects_active(effects: &ImageEffects) -> bool {
    effects.skew_deg.abs() >= 1e-6
        || effects.speckles > 0.0
        || effects.streaks > 0
        || effects.hole_punch > 0
        || effects.scratches > 0
        || effects.blur_px > 0
        || (effects.brightness - 1.0).abs() >= 1e-6
        || (effects.contrast - 1.0).abs() >= 1e-6
        || effects.banding > 0
        || effects.noise > 0.0
        || effects.vignette > 0.0
        || effects.margin_crop_px > 0
        || effects.show_through > 0.0
        || effects.moire
}

/// Apply effects then optional grayscale / BW1 conversion.
pub fn apply_pipeline(
    img: &mut RgbImage,
    effects: &ImageEffects,
    page_index: u32,
    color: ColorMode,
) {
    if pixel_effects_active(effects) {
        apply_effects(img, effects, page_index);
    }
    match color {
        ColorMode::Rgb24 => {}
        ColorMode::Gray8 => to_gray8(img),
        ColorMode::Bw1 => to_bw1(img),
    }
}

fn apply_effects(img: &mut RgbImage, effects: &ImageEffects, page_index: u32) {
    let mut rng =
        Rng::new(effects.seed ^ (u64::from(page_index).wrapping_mul(0x9E37_79B9_7F4A_7C15)));
    if effects.margin_crop_px > 0 {
        apply_margin_crop(img, effects.margin_crop_px);
    }
    if effects.hole_punch > 0 {
        draw_hole_punches(img, effects.hole_punch);
    }
    if effects.scratches > 0 {
        draw_scratches(img, effects.scratches, &mut rng);
    }
    if effects.streaks > 0 {
        draw_streaks(img, effects.streaks, effects.streak_axis, &mut rng);
    }
    if effects.speckles > 0.0 {
        draw_speckles(img, effects.speckles, &mut rng);
    }
    if effects.show_through > 0.0 {
        apply_show_through(img, effects.show_through, page_index, &mut rng);
    }
    if effects.banding > 0 {
        draw_banding(img, effects.banding, &mut rng);
    }
    if effects.moire {
        apply_moire(img, &mut rng);
    }
    if effects.noise > 0.0 {
        apply_noise(img, effects.noise, &mut rng);
    }
    if (effects.brightness - 1.0).abs() >= 1e-6 || (effects.contrast - 1.0).abs() >= 1e-6 {
        apply_brightness_contrast(img, effects.brightness, effects.contrast);
    }
    if effects.vignette > 0.0 {
        apply_vignette(img, effects.vignette);
    }
    if effects.blur_px > 0 {
        *img = box_blur(img, effects.blur_px);
    }
    if effects.skew_deg.abs() > 1e-6 {
        *img = rotate_nearest(img, effects.skew_deg);
    }
}

fn to_gray8(img: &mut RgbImage) {
    for p in img.pixels_mut() {
        let y = ((u32::from(p[0]) * 77 + u32::from(p[1]) * 150 + u32::from(p[2]) * 29) >> 8) as u8;
        *p = Rgb([y, y, y]);
    }
}

fn to_bw1(img: &mut RgbImage) {
    for p in img.pixels_mut() {
        let y = ((u32::from(p[0]) * 77 + u32::from(p[1]) * 150 + u32::from(p[2]) * 29) >> 8) as u8;
        let v = if y < 128 { 0 } else { 255 };
        *p = Rgb([v, v, v]);
    }
}

fn apply_brightness_contrast(img: &mut RgbImage, brightness: f32, contrast: f32) {
    for p in img.pixels_mut() {
        for c in 0..3 {
            let mut v = f32::from(p[c]) / 255.0;
            v = (v - 0.5) * contrast + 0.5;
            v *= brightness;
            p[c] = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }
}

fn box_blur(src: &RgbImage, radius: u32) -> RgbImage {
    let r = radius.max(1);
    let (w, h) = src.dimensions();
    let mut out = src.clone();
    let r_i = r as i32;
    for y in 0..h {
        for x in 0..w {
            let mut sum = [0u32; 3];
            let mut n = 0u32;
            for dy in -r_i..=r_i {
                for dx in -r_i..=r_i {
                    let sx = x as i32 + dx;
                    let sy = y as i32 + dy;
                    if sx >= 0 && sy >= 0 && (sx as u32) < w && (sy as u32) < h {
                        let p = src.get_pixel(sx as u32, sy as u32);
                        sum[0] += u32::from(p[0]);
                        sum[1] += u32::from(p[1]);
                        sum[2] += u32::from(p[2]);
                        n += 1;
                    }
                }
            }
            if let (Some(r), Some(g), Some(b)) = (
                sum[0].checked_div(n),
                sum[1].checked_div(n),
                sum[2].checked_div(n),
            ) {
                out.put_pixel(x, y, Rgb([r as u8, g as u8, b as u8]));
            }
        }
    }
    out
}

fn draw_scratches(img: &mut RgbImage, count: u32, rng: &mut Rng) {
    let (w, h) = img.dimensions();
    if w < 8 || h < 8 {
        return;
    }
    for _ in 0..count {
        let mut x = rng.gen_range(w) as i32;
        let mut y = rng.gen_range(h) as i32;
        let steps = 20 + rng.gen_range(60);
        let ink = 20 + rng.gen_range(40) as u8;
        for _ in 0..steps {
            if x >= 0 && y >= 0 && (x as u32) < w && (y as u32) < h {
                img.put_pixel(x as u32, y as u32, Rgb([ink, ink, ink.saturating_add(8)]));
            }
            let dx = rng.gen_range(3) as i32 - 1;
            let dy = rng.gen_range(3) as i32 - 1;
            x += dx;
            y += dy;
        }
    }
}

fn draw_speckles(img: &mut RgbImage, density: f32, rng: &mut Rng) {
    let (w, h) = img.dimensions();
    let n = ((w as f32) * (h as f32) * density * 0.002).round() as u32;
    let n = n.clamp(1, w.saturating_mul(h) / 50).max(1);
    for _ in 0..n {
        let x = rng.gen_range(w);
        let y = rng.gen_range(h);
        let dark = rng.gen_range(80) as u8;
        let r = 1 + rng.gen_range(3);
        for dy in 0..r {
            for dx in 0..r {
                let px = x.saturating_add(dx);
                let py = y.saturating_add(dy);
                if px < w && py < h {
                    img.put_pixel(px, py, Rgb([dark, dark, dark]));
                }
            }
        }
    }
}

fn draw_streaks(img: &mut RgbImage, count: u32, axis: StreakAxis, rng: &mut Rng) {
    let (w, h) = img.dimensions();
    let ink = Rgb([28, 28, 32]);
    for _ in 0..count {
        let thickness = 1 + rng.gen_range(3);
        match axis {
            StreakAxis::Vertical => {
                let x0 = rng.gen_range(w.saturating_sub(thickness).max(1));
                for y in 0..h {
                    for t in 0..thickness {
                        let x = x0 + t;
                        if x < w {
                            img.put_pixel(x, y, ink);
                        }
                    }
                }
            }
            StreakAxis::Horizontal => {
                let y0 = rng.gen_range(h.saturating_sub(thickness).max(1));
                for x in 0..w {
                    for t in 0..thickness {
                        let y = y0 + t;
                        if y < h {
                            img.put_pixel(x, y, ink);
                        }
                    }
                }
            }
        }
    }
}

/// Binder holes along the left (binding) edge — darkened rings.
fn draw_hole_punches(img: &mut RgbImage, count: u32) {
    let (w, h) = img.dimensions();
    if w < 40 || h < 80 || count == 0 {
        return;
    }
    let cx = (w as f32 * 0.06).round() as i32;
    let outer_r = ((w.min(h) as f32) * 0.028).round().max(6.0) as i32;
    let inner_r = (outer_r as f32 * 0.45).round().max(2.0) as i32;
    let margin = (h as f32 * 0.12).round() as i32;
    let usable = (h as i32) - 2 * margin;
    if usable <= 0 {
        return;
    }
    for i in 0..count {
        let t = if count == 1 {
            0.5
        } else {
            i as f32 / (count - 1) as f32
        };
        let cy = margin + ((usable as f32) * t).round() as i32;
        fill_ring(img, cx, cy, outer_r, inner_r, Rgb([22, 22, 26]));
        fill_ring(img, cx, cy, outer_r + 2, outer_r, Rgb([90, 90, 95]));
    }
}

fn fill_ring(img: &mut RgbImage, cx: i32, cy: i32, outer: i32, inner: i32, color: Rgb<u8>) {
    let (w, h) = img.dimensions();
    let o2 = outer * outer;
    let i2 = inner * inner;
    for dy in -outer..=outer {
        for dx in -outer..=outer {
            let d2 = dx * dx + dy * dy;
            if d2 <= o2 && d2 >= i2 {
                let x = cx + dx;
                let y = cy + dy;
                if x >= 0 && y >= 0 && (x as u32) < w && (y as u32) < h {
                    img.put_pixel(x as u32, y as u32, color);
                }
            }
        }
    }
}

/// Expand canvas by cropping inward then padding white (simulates missed autocrop).
fn apply_margin_crop(img: &mut RgbImage, crop: u32) {
    let (w, h) = img.dimensions();
    if crop == 0 || w <= 2 * crop + 8 || h <= 2 * crop + 8 {
        return;
    }
    let inner_w = w - 2 * crop;
    let inner_h = h - 2 * crop;
    let mut cropped = RgbImage::new(inner_w, inner_h);
    for y in 0..inner_h {
        for x in 0..inner_w {
            cropped.put_pixel(x, y, *img.get_pixel(x + crop, y + crop));
        }
    }
    let mut out = RgbImage::from_pixel(w, h, Rgb([255, 255, 255]));
    let ox = crop;
    let oy = crop;
    for y in 0..inner_h {
        for x in 0..inner_w {
            out.put_pixel(x + ox, y + oy, *cropped.get_pixel(x, y));
        }
    }
    *img = out;
}

/// Soft ghost of inverted/flipped content (thin paper duplex bleed).
fn apply_show_through(img: &mut RgbImage, strength: f32, page_index: u32, rng: &mut Rng) {
    let (w, h) = img.dimensions();
    let s = strength.clamp(0.0, 1.0) * 0.22;
    if s < 1e-4 {
        return;
    }
    let flip = page_index.is_multiple_of(2);
    let ox = (rng.gen_range(5) as i32) - 2;
    let oy = (rng.gen_range(5) as i32) - 2;
    let src = img.clone();
    for y in 0..h {
        for x in 0..w {
            let sx = if flip {
                (w as i32 - 1 - x as i32 + ox).clamp(0, w as i32 - 1) as u32
            } else {
                (x as i32 + ox).clamp(0, w as i32 - 1) as u32
            };
            let sy = (y as i32 + oy).clamp(0, h as i32 - 1) as u32;
            let ghost = src.get_pixel(sx, sy).0;
            let base = img.get_pixel(x, y).0;
            // Ghost is a lightened, desaturated inverse-ish bleed.
            let g = ((ghost[0] as u16 + ghost[1] as u16 + ghost[2] as u16) / 3) as f32;
            let bleed = 255.0 - (255.0 - g) * 0.35;
            let mix = |c: u8| -> u8 {
                ((c as f32) * (1.0 - s) + bleed * s)
                    .round()
                    .clamp(0.0, 255.0) as u8
            };
            img.put_pixel(x, y, Rgb([mix(base[0]), mix(base[1]), mix(base[2])]));
        }
    }
}

/// Horizontal luminance bands (uneven lamp / roller).
fn draw_banding(img: &mut RgbImage, count: u32, rng: &mut Rng) {
    let (w, h) = img.dimensions();
    if count == 0 || h < 16 {
        return;
    }
    for _ in 0..count {
        let band_h = (8 + rng.gen_range(24)).min(h / 4).max(4);
        let y0 = rng.gen_range(h.saturating_sub(band_h).max(1));
        let dim = 0.82 + (rng.gen_range(12) as f32) * 0.01;
        for y in y0..y0.saturating_add(band_h).min(h) {
            let edge = {
                let t = (y - y0) as f32 / band_h as f32;
                let soft = (t * std::f32::consts::PI).sin().max(0.0);
                1.0 - (1.0 - dim) * soft
            };
            for x in 0..w {
                let p = img.get_pixel(x, y).0;
                img.put_pixel(
                    x,
                    y,
                    Rgb([
                        ((p[0] as f32) * edge).round().clamp(0.0, 255.0) as u8,
                        ((p[1] as f32) * edge).round().clamp(0.0, 255.0) as u8,
                        ((p[2] as f32) * edge).round().clamp(0.0, 255.0) as u8,
                    ]),
                );
            }
        }
    }
}

/// Two-frequency interference overlay (halftone / screen clash).
fn apply_moire(img: &mut RgbImage, rng: &mut Rng) {
    let (w, h) = img.dimensions();
    let f1 = 0.12 + (rng.gen_range(8) as f32) * 0.01;
    let f2 = 0.13 + (rng.gen_range(8) as f32) * 0.01;
    let angle = (rng.gen_range(30) as f32 + 5.0).to_radians();
    let (sa, ca) = angle.sin_cos();
    let amp = 18.0;
    for y in 0..h {
        for x in 0..w {
            let xf = x as f32;
            let yf = y as f32;
            let u = xf * ca + yf * sa;
            let v = -xf * sa + yf * ca;
            let wave = ((u * f1).sin() * (v * f2).sin()) * amp;
            let p = img.get_pixel(x, y).0;
            let add = |c: u8| -> u8 { (c as f32 + wave).round().clamp(0.0, 255.0) as u8 };
            img.put_pixel(x, y, Rgb([add(p[0]), add(p[1]), add(p[2])]));
        }
    }
}

fn apply_noise(img: &mut RgbImage, strength: f32, rng: &mut Rng) {
    let (w, h) = img.dimensions();
    let amp = (strength.clamp(0.0, 1.0) * 48.0).round() as i32;
    if amp == 0 {
        return;
    }
    for y in 0..h {
        for x in 0..w {
            let delta = (rng.gen_range((2 * amp + 1) as u32) as i32) - amp;
            let p = img.get_pixel(x, y).0;
            let n = |c: u8| -> u8 { (c as i32 + delta).clamp(0, 255) as u8 };
            img.put_pixel(x, y, Rgb([n(p[0]), n(p[1]), n(p[2])]));
        }
    }
}

fn apply_vignette(img: &mut RgbImage, strength: f32) {
    let (w, h) = img.dimensions();
    let s = strength.clamp(0.0, 1.0);
    if s < 1e-4 {
        return;
    }
    let cx = (w as f32 - 1.0) * 0.5;
    let cy = (h as f32 - 1.0) * 0.5;
    let max_r = (cx * cx + cy * cy).sqrt().max(1.0);
    for y in 0..h {
        for x in 0..w {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let t = ((dx * dx + dy * dy).sqrt() / max_r).clamp(0.0, 1.0);
            // Smoothstep falloff toward corners.
            let fall = t * t * (3.0 - 2.0 * t);
            let mul = 1.0 - s * fall * 0.55;
            let p = img.get_pixel(x, y).0;
            img.put_pixel(
                x,
                y,
                Rgb([
                    ((p[0] as f32) * mul).round().clamp(0.0, 255.0) as u8,
                    ((p[1] as f32) * mul).round().clamp(0.0, 255.0) as u8,
                    ((p[2] as f32) * mul).round().clamp(0.0, 255.0) as u8,
                ]),
            );
        }
    }
}

/// Nearest-neighbor rotate about center; uncovered pixels white.
fn rotate_nearest(src: &RgbImage, deg: f32) -> RgbImage {
    let (w, h) = src.dimensions();
    let mut out = RgbImage::from_pixel(w, h, Rgb([255, 255, 255]));
    let rad = deg.to_radians();
    let (sin, cos) = rad.sin_cos();
    let cx = (w as f32 - 1.0) * 0.5;
    let cy = (h as f32 - 1.0) * 0.5;
    for y in 0..h {
        for x in 0..w {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let sx = cos * dx + sin * dy + cx;
            let sy = -sin * dx + cos * dy + cy;
            if sx >= 0.0 && sy >= 0.0 {
                let ix = sx.round() as i32;
                let iy = sy.round() as i32;
                if ix >= 0 && iy >= 0 && (ix as u32) < w && (iy as u32) < h {
                    out.put_pixel(x, y, *src.get_pixel(ix as u32, iy as u32));
                }
            }
        }
    }
    out
}

/// Tiny deterministic LCG (no extra crate).
struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: u64) -> Self {
        Self { state: seed | 1 }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1);
        self.state
    }

    fn gen_range(&mut self, max: u32) -> u32 {
        if max == 0 {
            return 0;
        }
        (self.next_u64() % u64::from(max)) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank(w: u32, h: u32) -> RgbImage {
        RgbImage::from_pixel(w, h, Rgb([240, 240, 240]))
    }

    #[test]
    fn preset_dirty_adf_sets_streaks() {
        let mut e = ImageEffects::default();
        e.apply_preset("dirty_adf").unwrap();
        assert!(e.streaks > 0);
        assert!(e.speckles > 0.0);
        assert_eq!(e.streak_axis, StreakAxis::Vertical);
    }

    #[test]
    fn preset_office_bad_and_harsh_jpeg() {
        let mut e = ImageEffects::default();
        e.apply_preset("harsh_jpeg").unwrap();
        assert_eq!(e.jpeg_quality, 28);
        e.apply_preset("office_bad").unwrap();
        assert!(e.streaks > 0);
        assert!(e.jpeg_quality > 0);
        assert!(e.banding > 0);
        assert!(e.noise > 0.0);
        assert!(e.vignette > 0.0);
        e.apply_preset("dirty_glass").unwrap();
        assert!(e.scratches > 0);
        e.apply_preset("dim").unwrap();
        assert!(e.brightness < 1.0);
    }

    #[test]
    fn merge_body_sets_advanced_effects() {
        let mut e = ImageEffects::default();
        e.apply_body(&EffectsBody {
            banding: Some(3),
            noise: Some(0.2),
            vignette: Some(0.4),
            margin_crop_px: Some(12),
            show_through: Some(0.5),
            moire: Some(true),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(e.banding, 3);
        assert!((e.noise - 0.2).abs() < 1e-6);
        assert!((e.vignette - 0.4).abs() < 1e-6);
        assert_eq!(e.margin_crop_px, 12);
        assert!((e.show_through - 0.5).abs() < 1e-6);
        assert!(e.moire);
        assert!(!e.is_clean());
    }

    #[test]
    fn advanced_effects_change_pixels() {
        let mut base = blank(160, 200);
        for y in 20..180 {
            for x in 20..140 {
                base.put_pixel(x, y, Rgb([180, 120, 60]));
            }
        }
        let cases = [
            ImageEffects {
                seed: 3,
                banding: 2,
                ..Default::default()
            },
            ImageEffects {
                seed: 3,
                noise: 0.3,
                ..Default::default()
            },
            ImageEffects {
                seed: 3,
                vignette: 0.8,
                ..Default::default()
            },
            ImageEffects {
                seed: 3,
                margin_crop_px: 10,
                ..Default::default()
            },
            ImageEffects {
                seed: 3,
                show_through: 0.7,
                ..Default::default()
            },
            ImageEffects {
                seed: 3,
                moire: true,
                ..Default::default()
            },
        ];
        for e in cases {
            let mut img = base.clone();
            apply_effects(&mut img, &e, 0);
            assert_ne!(
                base.as_raw(),
                img.as_raw(),
                "effect should alter pixels: {e:?}"
            );
        }
    }

    #[test]
    fn same_seed_same_speckles() {
        let mut a = blank(200, 200);
        let mut b = blank(200, 200);
        let e = ImageEffects {
            seed: 99,
            speckles: 0.05,
            ..Default::default()
        };
        apply_effects(&mut a, &e, 0);
        apply_effects(&mut b, &e, 0);
        assert_eq!(a.as_raw(), b.as_raw());
    }

    #[test]
    fn different_seed_different_speckles() {
        let mut a = blank(200, 200);
        let mut b = blank(200, 200);
        let mut e = ImageEffects {
            seed: 1,
            speckles: 0.05,
            ..Default::default()
        };
        apply_effects(&mut a, &e, 0);
        e.seed = 2;
        apply_effects(&mut b, &e, 0);
        assert_ne!(a.as_raw(), b.as_raw());
    }

    #[test]
    fn skew_changes_pixels() {
        let mut clean = blank(120, 160);
        let mut skewed = blank(120, 160);
        for y in 40..80 {
            for x in 40..80 {
                clean.put_pixel(x, y, Rgb([0, 0, 0]));
                skewed.put_pixel(x, y, Rgb([0, 0, 0]));
            }
        }
        let e = ImageEffects {
            skew_deg: 8.0,
            ..Default::default()
        };
        apply_effects(&mut skewed, &e, 0);
        assert_ne!(clean.as_raw(), skewed.as_raw());
    }

    #[test]
    fn hole_punch_darkens_left_edge() {
        let mut img = blank(300, 400);
        let e = ImageEffects {
            hole_punch: 3,
            ..Default::default()
        };
        apply_effects(&mut img, &e, 0);
        let cx = (300.0_f32 * 0.06).round() as u32;
        let mut dark = 0u32;
        for y in 0..400 {
            let p = img.get_pixel(cx, y);
            if p[0] < 80 {
                dark += 1;
            }
        }
        assert!(
            dark > 20,
            "expected dark ring pixels near binding edge, got {dark}"
        );
    }

    #[test]
    fn gray8_collapses_chroma() {
        let mut img = blank(10, 10);
        img.put_pixel(0, 0, Rgb([255, 0, 0]));
        to_gray8(&mut img);
        let p = img.get_pixel(0, 0);
        assert_eq!(p[0], p[1]);
        assert_eq!(p[1], p[2]);
    }

    #[test]
    fn bw1_thresholds() {
        let mut img = blank(4, 4);
        img.put_pixel(0, 0, Rgb([200, 200, 200]));
        img.put_pixel(1, 0, Rgb([40, 40, 40]));
        to_bw1(&mut img);
        assert_eq!(img.get_pixel(0, 0), &Rgb([255, 255, 255]));
        assert_eq!(img.get_pixel(1, 0), &Rgb([0, 0, 0]));
    }

    #[test]
    fn brightness_dims() {
        let mut img = blank(8, 8);
        let e = ImageEffects {
            brightness: 0.5,
            ..Default::default()
        };
        apply_effects(&mut img, &e, 0);
        assert!(img.get_pixel(0, 0)[0] < 200);
    }

    #[test]
    fn color_mode_parse() {
        assert_eq!(ColorMode::parse("Grayscale8"), ColorMode::Gray8);
        assert_eq!(ColorMode::parse("RGB24"), ColorMode::Rgb24);
        assert_eq!(ColorMode::parse("BW1"), ColorMode::Bw1);
    }
}
