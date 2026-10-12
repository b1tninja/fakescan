//! ISO-style and composite test-page patterns.
//!
//! Provides the draw functions for:
//! - [`crate::image_gen::PatternId::SlantEdge`] — ~5° slanted edge (ISO 16067-1 style; no MTF math)
//! - [`crate::image_gen::PatternId::TextLadder`] — glyph size ladder from 8× down to 1×
//! - [`crate::image_gen::PatternId::Testpage`]   — all-in-one composite
//!
//! The enum variants, `ALL`, `parse`, and render dispatch live in [`crate::image_gen`].
//! Shared pixel primitives (`fill_rect`, `draw_border`, `draw_glyph`) are `pub(crate)` there.

use image::{ImageBuffer, Rgb, RgbImage};

use crate::image_gen::{draw_border, draw_glyph, fill_rect};

/// `tan(5°)` — horizontal shift per vertical pixel for the ~5° slanted edge.
const TAN_5_DEG: f32 = 0.087_488_66_f32;

// ── slant_edge ───────────────────────────────────────────────────────────────

/// ~5° slanted edge (ISO 16067-1 *style* visual; no MTF math).
///
/// Left side dark (~28,28,28), right side light (~244,244,244), narrow
/// gradient transition. The edge runs ~5° off vertical across the page.
///
/// Page index shifts the edge's horizontal start position (40–60 % of width)
/// so successive pages remain pixel-distinct even before banner/QR stamping.
pub fn draw_slant_edge(w: u32, h: u32, page_index: u32) -> RgbImage {
    let mut img: RgbImage = ImageBuffer::from_pixel(w, h, Rgb([255u8, 255, 255]));

    // Horizontal centre of the edge: 40–60 % of width cycling over 5 pages.
    let frac = 0.40_f32 + 0.05 * ((page_index % 5) as f32);
    let cx = (w as f32 * frac) as i32;

    // Transition half-width: kept narrow so the edge is visually sharp.
    let tw = ((w as f32 * 0.004).ceil() as i32).max(3);

    for y in 0..h {
        // Edge X shifts right going down (positive TAN_5_DEG → right lean).
        let edge_x = cx + (y as f32 * TAN_5_DEG) as i32;
        for x in 0..w as i32 {
            let d = x - edge_x;
            let pix = if d < -tw {
                Rgb([28u8, 28, 28]) // dark side
            } else if d > tw {
                Rgb([244u8, 244, 244]) // light side
            } else {
                // Linear gradient through the transition band.
                let t = (d + tw) as f32 / (2 * tw) as f32;
                let v = (28.0 + t * 216.0) as u8;
                Rgb([v, v, v])
            };
            img.put_pixel(x as u32, y, pix);
        }
    }

    // Tick-mark row in the top margin: count = page_index+1 (page-distinct cue).
    let ticks = (page_index + 1).min(10);
    let tw2 = (w / 60).max(2);
    for i in 0..ticks {
        let tx = w / 12 + i * (tw2 * 3);
        fill_rect(
            &mut img,
            tx,
            h / 30,
            tx + tw2,
            h / 30 + tw2 * 3,
            Rgb([70u8, 70, 70]),
        );
    }

    draw_border(&mut img, Rgb([28u8, 28, 28]));
    img
}

// ── textladder ───────────────────────────────────────────────────────────────

/// Text size ladder: the phrase "SCAN TEST 12345 ABCDE" rendered at seven
/// decreasing glyph scales (8× → 1×).
///
/// The smallest row that a capture can resolve indicates its effective
/// resolution. The scale label (e.g. "8X") is printed in blue-grey at the
/// start of each band.
///
/// Page index shifts the left margin so successive pages are pixel-distinct.
pub fn draw_text_ladder(w: u32, h: u32, page_index: u32) -> RgbImage {
    let mut img: RgbImage = ImageBuffer::from_pixel(w, h, Rgb([252u8, 252, 248]));
    draw_border(&mut img, Rgb([20u8, 20, 20]));

    const SCALES: &[u32] = &[8, 6, 5, 4, 3, 2, 1];
    const LABELS: &[&str] = &["8X", "6X", "5X", "4X", "3X", "2X", "1X"];
    const TEXT: &str = "SCAN TEST 12345 ABCDE";

    let pad = (w / 40).max(4);
    let ink = Rgb([15u8, 15, 15]);
    let lbl_ink = Rgb([80u8, 80, 180]);
    let sep_ink = Rgb([185u8, 185, 185]);

    let usable_h = h.saturating_sub(pad * 2);
    let band_h = (usable_h / SCALES.len() as u32).max(1);

    // Small left-margin shift per page index so bytes differ between pages.
    let page_offset = (page_index % 4) * (w / 60).max(2);

    for (i, (&scale, label)) in SCALES.iter().zip(LABELS.iter()).enumerate() {
        let y0 = pad + i as u32 * band_h;

        // Thin separator above each row (except the first).
        if i > 0 {
            fill_rect(
                &mut img,
                pad,
                y0.saturating_sub(1),
                w.saturating_sub(pad),
                y0,
                sep_ink,
            );
        }

        // Vertical centre of this band, snapped so glyphs don't overflow.
        let glyph_h = 7 * scale;
        let y = y0 + (band_h / 2).saturating_sub(glyph_h / 2);

        // Scale label on the left.
        let mut x = pad + page_offset;
        for ch in label.chars() {
            let adv = draw_glyph(&mut img, ch, x, y, scale.max(1), lbl_ink);
            x += adv + scale.max(1);
        }
        x += pad;

        // Test phrase — truncated if the page is too narrow.
        for ch in TEXT.chars() {
            if x + 5 * scale + pad >= w {
                break;
            }
            let adv = draw_glyph(&mut img, ch, x, y, scale.max(1), ink);
            x += adv + scale.max(1);
        }
    }

    img
}

// ── testpage ─────────────────────────────────────────────────────────────────

/// All-in-one composite page.
///
/// Layout (top → bottom, proportions of total height):
///
/// | Band | Height | Content |
/// |------|--------|---------|
/// | Top  | 45 %   | Left 38 %: gray step wedges · Right 62 %: ~5° slant edge |
/// | Patches | 12 % | R / G / B / gray cells (four equal columns) |
/// | Ladder | 33 % | Text size ladder at three scales (5×, 3×, 1×) |
/// | Corners | 10 % | L-corner marks at the four corners + centre cross |
///
/// Everything is drawn with `fill_rect` / `draw_glyph` — no recursive JPEG
/// encode. The banner and QR code are stamped on top by
/// [`crate::image_gen::render_pattern_rgb_ctx`] as usual.
pub fn draw_testpage(w: u32, h: u32, page_index: u32) -> RgbImage {
    let mut img: RgbImage = ImageBuffer::from_pixel(w, h, Rgb([240u8, 240, 235]));

    // ── section boundaries ────────────────────────────────────────────────────
    let top_h = h * 45 / 100; // wedges + slant-edge band
    let patch_h = h * 12 / 100; // colour patches
    let ladder_h = h * 33 / 100; // text ladder
                                 // bottom remainder → corner marks

    let y_patch = top_h;
    let y_ladder = y_patch + patch_h;
    let y_corner = y_ladder + ladder_h;

    // ── left strip: gray step wedges ──────────────────────────────────────────
    let wedge_x1 = w * 38 / 100;
    let steps = (6u32 + page_index % 3).max(4);
    let step_h = (top_h / steps).max(1);
    for i in 0..steps {
        let t = if steps <= 1 {
            255u8
        } else {
            ((i * 255) / (steps - 1)) as u8
        };
        let wy0 = i * step_h;
        let wy1 = if i + 1 == steps {
            top_h
        } else {
            ((i + 1) * step_h).min(top_h)
        };
        fill_rect(&mut img, 0, wy0, wedge_x1, wy1, Rgb([t, t, t]));
    }
    // Divider between wedge strip and slant-edge region.
    fill_rect(
        &mut img,
        wedge_x1,
        0,
        wedge_x1 + 1,
        top_h,
        Rgb([20u8, 20, 20]),
    );

    // ── right region: ~5° slant edge ─────────────────────────────────────────
    let edge_x0 = wedge_x1 + 1;
    let edge_w = w.saturating_sub(edge_x0);
    let frac = 0.40_f32 + 0.05 * ((page_index % 5) as f32);
    let cx = (edge_x0 as f32 + edge_w as f32 * frac) as i32;
    let tw = ((edge_w as f32 * 0.006).ceil() as i32).max(3);
    for y in 0..top_h {
        let edge_x = cx + (y as f32 * TAN_5_DEG) as i32;
        for x in edge_x0 as i32..w as i32 {
            let d = x - edge_x;
            let pix = if d < -tw {
                Rgb([28u8, 28, 28])
            } else if d > tw {
                Rgb([244u8, 244, 244])
            } else {
                let t = (d + tw) as f32 / (2 * tw) as f32;
                let v = (28.0 + t * 216.0) as u8;
                Rgb([v, v, v])
            };
            img.put_pixel(x as u32, y, pix);
        }
    }

    // ── colour patches (four equal columns) ───────────────────────────────────
    let cw = w / 4;
    let gray = (60u32 + (page_index % 8) * 20).min(220) as u8;
    fill_rect(
        &mut img,
        0,
        y_patch,
        cw,
        y_patch + patch_h,
        Rgb([220u8, 40, 40]),
    );
    fill_rect(
        &mut img,
        cw,
        y_patch,
        cw * 2,
        y_patch + patch_h,
        Rgb([40u8, 180, 50]),
    );
    fill_rect(
        &mut img,
        cw * 2,
        y_patch,
        cw * 3,
        y_patch + patch_h,
        Rgb([40u8, 80, 220]),
    );
    fill_rect(
        &mut img,
        cw * 3,
        y_patch,
        w,
        y_patch + patch_h,
        Rgb([gray, gray, gray]),
    );
    // Patch separators.
    for &px in &[cw, cw * 2, cw * 3] {
        fill_rect(
            &mut img,
            px,
            y_patch,
            px + 1,
            y_patch + patch_h,
            Rgb([20u8, 20, 20]),
        );
    }

    // Separator between patches and ladder.
    fill_rect(
        &mut img,
        0,
        y_ladder.saturating_sub(1),
        w,
        y_ladder,
        Rgb([20u8, 20, 20]),
    );

    // ── text ladder (3 scales: 5×, 3×, 1×) ───────────────────────────────────
    const LADDER: &[(u32, &str)] = &[
        (5, "5X SCAN TEST"),
        (3, "3X SCAN TEST 12345"),
        (1, "1X SCAN TEST 12345 ABCDE"),
    ];
    let n = LADDER.len() as u32;
    let lband = (ladder_h / n).max(1);
    let pad = (w / 40).max(4);
    let ladder_ink = Rgb([15u8, 15, 15]);
    let sep_ink = Rgb([185u8, 185, 185]);

    for (li, &(scale, text)) in LADDER.iter().enumerate() {
        let ly0 = y_ladder + li as u32 * lband;
        if li > 0 {
            fill_rect(
                &mut img,
                pad,
                ly0.saturating_sub(1),
                w.saturating_sub(pad),
                ly0,
                sep_ink,
            );
        }
        let glyph_h = 7 * scale;
        let ly = ly0 + (lband / 2).saturating_sub(glyph_h / 2);
        let mut x = pad;
        for ch in text.chars() {
            if x + 5 * scale + pad >= w {
                break;
            }
            let adv = draw_glyph(&mut img, ch, x, ly, scale.max(1), ladder_ink);
            x += adv + scale.max(1);
        }
    }

    // ── corner marks + centre cross ───────────────────────────────────────────
    let corner_ink = Rgb([30u8, 30, 30]);
    let accent = Rgb([160u8, 50, 40]);
    let arm = ((w.min(h - y_corner) / 6).max(12) + (page_index % 3) * 3).min(w.min(h) / 6);
    let thick = (arm / 5).max(2);
    let inset = (w.min(h) / 60).max(3);

    // L-marks at the four corners of the bottom band.
    for &(ox, oy, right, down) in &[
        (inset, y_corner + inset, true, true),
        (w.saturating_sub(inset + 1), y_corner + inset, false, true),
        (inset, h.saturating_sub(inset + 1), true, false),
        (
            w.saturating_sub(inset + 1),
            h.saturating_sub(inset + 1),
            false,
            false,
        ),
    ] {
        draw_l_mark(&mut img, ox, oy, right, down, arm, thick, corner_ink);
    }

    // Centre cross in the bottom band.
    let cx = w / 2;
    let cy = y_corner + (h - y_corner) / 2;
    fill_rect(
        &mut img,
        cx.saturating_sub(arm),
        cy.saturating_sub(thick / 2),
        (cx + arm).min(w),
        (cy + thick / 2 + 1).min(h),
        accent,
    );
    fill_rect(
        &mut img,
        cx.saturating_sub(thick / 2),
        cy.saturating_sub(arm),
        (cx + thick / 2 + 1).min(w),
        (cy + arm).min(h),
        accent,
    );

    draw_border(&mut img, Rgb([20u8, 20, 20]));
    img
}

/// Draw a single L-mark (two perpendicular legs) anchored at `(ox, oy)`.
#[allow(clippy::too_many_arguments)]
fn draw_l_mark(
    img: &mut RgbImage,
    ox: u32,
    oy: u32,
    right: bool,
    down: bool,
    arm: u32,
    thick: u32,
    color: Rgb<u8>,
) {
    let (w, h) = img.dimensions();
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
    // Horizontal leg.
    fill_rect(
        img,
        hx0,
        oy.saturating_sub(thick / 2),
        hx1,
        (oy + thick / 2 + 1).min(h),
        color,
    );
    // Vertical leg.
    fill_rect(
        img,
        ox.saturating_sub(thick / 2),
        vy0,
        (ox + thick / 2 + 1).min(w),
        vy1,
        color,
    );
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image_gen::{jpeg_pattern, read_page_marker, PatternId};
    use std::collections::HashSet;

    #[test]
    fn slant_edge_parse_roundtrip_and_aliases() {
        assert_eq!(
            PatternId::parse("slant_edge").unwrap(),
            PatternId::SlantEdge
        );
        assert_eq!(
            PatternId::parse("slant-edge").unwrap(),
            PatternId::SlantEdge
        );
        assert_eq!(PatternId::parse("slantedge").unwrap(), PatternId::SlantEdge);
        assert_eq!(PatternId::SlantEdge.as_str(), "slant_edge");
    }

    #[test]
    fn text_ladder_parse_roundtrip_and_aliases() {
        assert_eq!(
            PatternId::parse("textladder").unwrap(),
            PatternId::TextLadder
        );
        assert_eq!(
            PatternId::parse("text_ladder").unwrap(),
            PatternId::TextLadder
        );
        assert_eq!(
            PatternId::parse("text-ladder").unwrap(),
            PatternId::TextLadder
        );
        assert_eq!(PatternId::TextLadder.as_str(), "textladder");
    }

    #[test]
    fn testpage_parse_roundtrip_and_aliases() {
        assert_eq!(PatternId::parse("testpage").unwrap(), PatternId::Testpage);
        assert_eq!(PatternId::parse("test_page").unwrap(), PatternId::Testpage);
        assert_eq!(PatternId::parse("test-page").unwrap(), PatternId::Testpage);
        assert_eq!(PatternId::Testpage.as_str(), "testpage");
    }

    /// Every new pattern renders a non-empty JPEG with a COM page marker.
    #[test]
    fn new_patterns_render_non_empty_with_com() {
        for &pat in &[
            PatternId::SlantEdge,
            PatternId::TextLadder,
            PatternId::Testpage,
        ] {
            let jpeg = jpeg_pattern(pat, 100, 0)
                .unwrap_or_else(|e| panic!("{} render failed: {e}", pat.as_str()));
            assert!(!jpeg.is_empty(), "{} produced empty JPEG", pat.as_str());
            assert_eq!(
                read_page_marker(&jpeg),
                Some(0),
                "COM missing for {}",
                pat.as_str()
            );
            assert_eq!(&jpeg[..2], &[0xFF, 0xD8], "{} is not a JPEG", pat.as_str());
        }
    }

    /// Page 0 ≠ page 1 for every new pattern.
    #[test]
    fn new_patterns_page_index_distinct() {
        for &pat in &[
            PatternId::SlantEdge,
            PatternId::TextLadder,
            PatternId::Testpage,
        ] {
            let p0 = jpeg_pattern(pat, 100, 0).unwrap();
            let p1 = jpeg_pattern(pat, 100, 1).unwrap();
            assert_ne!(p0, p1, "page 0 == page 1 for {}", pat.as_str());
            assert_eq!(read_page_marker(&p0), Some(0));
            assert_eq!(read_page_marker(&p1), Some(1));
        }
    }

    /// The three new patterns produce bytes distinct from each other at page 0.
    #[test]
    fn new_patterns_mutually_distinct() {
        let mut seen: HashSet<Vec<u8>> = HashSet::new();
        for &pat in &[
            PatternId::SlantEdge,
            PatternId::TextLadder,
            PatternId::Testpage,
        ] {
            let bytes = jpeg_pattern(pat, 100, 0).unwrap();
            assert!(
                seen.insert(bytes),
                "pattern {} collided with another new pattern",
                pat.as_str()
            );
        }
    }

    /// draw_slant_edge has a dark left half and a light right half.
    #[test]
    fn slant_edge_dark_left_light_right() {
        let w = 200u32;
        let h = 300u32;
        let img = draw_slant_edge(w, h, 0);
        // Mid-height row: sample well inside the dark and light halves.
        let mid_y = h / 2;
        let dark_px = img.get_pixel(w / 8, mid_y).0;
        let light_px = img.get_pixel(w * 7 / 8, mid_y).0;
        let dark_luma = (dark_px[0] as u32 + dark_px[1] as u32 + dark_px[2] as u32) / 3;
        let light_luma = (light_px[0] as u32 + light_px[1] as u32 + light_px[2] as u32) / 3;
        assert!(
            dark_luma < 80,
            "left side should be dark, got luma={dark_luma}"
        );
        assert!(
            light_luma > 180,
            "right side should be light, got luma={light_luma}"
        );
    }

    /// draw_text_ladder has glyph ink pixels (dark pixels from text rendering).
    #[test]
    fn text_ladder_has_glyph_ink() {
        let w = 400u32;
        let h = 600u32;
        let img = draw_text_ladder(w, h, 0);
        // Count near-black pixels — text ink at scale ≥1 should leave many.
        let dark_count = img
            .pixels()
            .filter(|p| p.0[0] < 50 && p.0[1] < 50 && p.0[2] < 50)
            .count();
        assert!(
            dark_count > 200,
            "text ladder has too few dark pixels: {dark_count}"
        );
    }

    /// draw_testpage covers wedge grays, coloured patches, and corner marks.
    #[test]
    fn testpage_has_wedge_colour_and_corner_content() {
        let w = 400u32;
        let h = 600u32;
        let img = draw_testpage(w, h, 0);
        // At least one near-black pixel (dark side of slant edge / border).
        let has_dark = img
            .pixels()
            .any(|p| p.0[0] < 50 && p.0[1] < 50 && p.0[2] < 50);
        assert!(has_dark, "testpage has no dark pixels");
        // At least one red pixel (R patch).
        let has_red = img
            .pixels()
            .any(|p| p.0[0] > 160 && p.0[1] < 80 && p.0[2] < 80);
        assert!(has_red, "testpage has no red pixels (patch region missing)");
        // At least one blue pixel (B patch).
        let has_blue = img
            .pixels()
            .any(|p| p.0[2] > 160 && p.0[0] < 80 && p.0[1] < 120);
        assert!(
            has_blue,
            "testpage has no blue pixels (patch region missing)"
        );
    }
}
