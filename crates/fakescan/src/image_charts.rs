//! Printer / scanner test-page patterns (phase 2).
//!
//! Each `draw_*` function accepts pixel dimensions `(w, h)` (already scaled to
//! the requested DPI by [`crate::image_gen::capped_page_pixels`]) and a
//! 0-based `page_index`, and returns a fresh `RgbImage`.  The caller
//! ([`crate::image_gen::render_pattern_rgb_opts`]) stamps the banner and QR
//! code after the draw function returns; do **not** add them here.
//!
//! Patterns implemented here:
//!
//! | Id | Draw function | Purpose |
//! |----|---------------|---------|
//! | `cmyk` | [`draw_cmyk`] | CMYK bars + tint ladders |
//! | `nozzle` | [`draw_nozzle`] | Nozzle-check horizontal / vertical bands |
//! | `align` | [`draw_align`] | Registration crosses + edge tick marks |
//! | `grid` | [`draw_grid`] | Margin frames + inch / quarter-inch scale rules |
//! | `lines` | [`draw_lines`] | Resolution line ladder (coarse → fine) |

use image::{ImageBuffer, Rgb, RgbImage};

use crate::image_gen::{draw_border, draw_glyph, fill_rect};

// ── local helpers ─────────────────────────────────────────────────────────────

/// Draw a centred cross (horizontal + vertical bar) at `(cx, cy)`.
fn draw_cross(img: &mut RgbImage, cx: u32, cy: u32, arm: u32, thick: u32, color: Rgb<u8>) {
    let (w, h) = img.dimensions();
    // Horizontal bar.
    fill_rect(
        img,
        cx.saturating_sub(arm),
        cy.saturating_sub(thick / 2),
        (cx + arm + 1).min(w),
        (cy + thick / 2 + 1).min(h),
        color,
    );
    // Vertical bar.
    fill_rect(
        img,
        cx.saturating_sub(thick / 2),
        cy.saturating_sub(arm),
        (cx + thick / 2 + 1).min(w),
        (cy + arm + 1).min(h),
        color,
    );
}

// ── CMYK ──────────────────────────────────────────────────────────────────────

/// CMYK bars + tint ladders (RGB-approximated; channels labelled C / M / Y / K).
///
/// Layout (top to bottom):
/// 1. Four equal columns with a solid 100 % bar and a channel-letter label.
/// 2. Eight tint steps per column fading from 100 % to ~12 % (not pure white,
///    so the lightest step is still distinguishable).
/// 3. A narrow overprint strip: C+M (blue), M+Y (red), C+Y (green), C+M+Y (near-black).
/// 4. Outer border.
pub fn draw_cmyk(w: u32, h: u32, _page_index: u32) -> RgbImage {
    let mut img: RgbImage = ImageBuffer::from_pixel(w, h, Rgb([255u8, 255, 255]));

    // Channel primaries as RGB approximations, with their label character.
    const CHANNELS: [(u8, u8, u8, char); 4] = [
        (0, 210, 210, 'C'), // Cyan
        (210, 0, 210, 'M'), // Magenta
        (210, 210, 0, 'Y'), // Yellow
        (20, 20, 20, 'K'),  // Key / Black
    ];
    let nc = CHANNELS.len() as u32;
    let col_w = w / nc;
    // Glyph scale fits in ~1/6 of column width.
    let scale = (col_w / 50).clamp(2, 6);
    let glyph_h = 7 * scale;
    let pad = scale * 2;
    let solid_h = glyph_h + pad * 4;
    let tint_steps: u32 = 8;
    let overprint_h = (h / 16).clamp(8, 40);
    let ladder_h = h.saturating_sub(10 + solid_h + 10 + overprint_h + 14);
    let step_h = (ladder_h / tint_steps).max(4);
    let ladder_y0 = 10 + solid_h + 10;

    for (ci, &(r, g, b, label)) in CHANNELS.iter().enumerate() {
        let ci = ci as u32;
        let x0 = ci * col_w + 1;
        let x1 = (x0 + col_w - 1).min(w);
        let color = Rgb([r, g, b]);

        // Solid 100 % bar.
        fill_rect(&mut img, x0, 10, x1, 10 + solid_h, color);

        // Channel label centred in the solid bar (white ink).
        let gw = 5 * scale; // glyph advance width
        let label_x = (x0 + (col_w / 2)).saturating_sub(gw / 2 + 1);
        let label_y = 10 + pad;
        let ink = if r < 50 && g < 50 && b < 50 {
            Rgb([220u8, 220, 220]) // K bar: light gray for contrast
        } else {
            Rgb([255u8, 255, 255])
        };
        draw_glyph(&mut img, label, label_x, label_y, scale, ink);

        // Tint ladder: step 0 = 100 %, step 7 = ~12 %.
        for step in 0..tint_steps {
            // alpha: 255 at step 0, down to ~32 at the last step.
            let alpha = 255u32 - step * 223 / tint_steps.saturating_sub(1).max(1);
            // Blend channel colour toward white.
            let sr = (255u32 - (255 - r as u32) * alpha / 255) as u8;
            let sg = (255u32 - (255 - g as u32) * alpha / 255) as u8;
            let sb = (255u32 - (255 - b as u32) * alpha / 255) as u8;
            let y0 = ladder_y0 + step * step_h;
            let y1 = (y0 + step_h).min(h.saturating_sub(overprint_h + 14));
            fill_rect(&mut img, x0, y0, x1, y1, Rgb([sr, sg, sb]));
        }
    }

    // Overprint strip at the bottom of the ladder area.
    // C+M → Blue, M+Y → Red, C+Y → Green, C+M+Y → near-black.
    const OP: [(u8, u8, u8); 4] = [(0, 0, 200), (200, 0, 0), (0, 200, 0), (10, 10, 10)];
    let op_y0 = h.saturating_sub(overprint_h + 6);
    let op_col = w / 4;
    for (ci, &rgb) in OP.iter().enumerate() {
        let x0 = ci as u32 * op_col + 1;
        let x1 = (x0 + op_col - 1).min(w);
        fill_rect(
            &mut img,
            x0,
            op_y0,
            x1,
            (op_y0 + overprint_h).min(h - 4),
            Rgb([rgb.0, rgb.1, rgb.2]),
        );
    }

    draw_border(&mut img, Rgb([0u8, 0, 0]));
    img
}

// ── NOZZLE ────────────────────────────────────────────────────────────────────

/// Nozzle-check style horizontal and vertical band groups (7 colour channels).
///
/// The top half of the page holds horizontal line bands (one group per channel,
/// left-to-right across the full page width).  The bottom half holds vertical
/// line bands.  A broken or missing nozzle would appear as a visible gap or
/// lighter stripe in the corresponding row of that channel's group.
pub fn draw_nozzle(w: u32, h: u32, _page_index: u32) -> RgbImage {
    let mut img: RgbImage = ImageBuffer::from_pixel(w, h, Rgb([255u8, 255, 255]));

    const COLORS: [Rgb<u8>; 7] = [
        Rgb([0, 200, 200]), // C
        Rgb([200, 0, 200]), // M
        Rgb([200, 200, 0]), // Y
        Rgb([20, 20, 20]),  // K
        Rgb([220, 40, 40]), // R
        Rgb([40, 200, 40]), // G
        Rgb([40, 40, 220]), // B
    ];
    let nc = COLORS.len() as u32;
    let line_count = 12u32;

    // ── Top half: horizontal line bands ──────────────────────────────────────
    let h_half = h / 2;
    let margin_v = (h / 30).max(4);
    let usable_h = h_half.saturating_sub(2 * margin_v);
    let group_h = (usable_h / nc).max(1);
    // Keep line_thick ≥ 1 and small enough to fit line_count lines with gaps.
    let line_thick_h = (group_h / (line_count * 2 + 2)).clamp(1, 4);
    let gap_h = line_thick_h;

    for (ci, &color) in COLORS.iter().enumerate() {
        let ci = ci as u32;
        let group_top = margin_v + ci * group_h;
        for line in 0..line_count {
            let y0 = group_top + line * (line_thick_h + gap_h);
            let y1 = (y0 + line_thick_h).min(h_half.saturating_sub(1));
            fill_rect(&mut img, w / 20, y0, w - w / 20, y1, color);
        }
    }

    // Horizontal separator between halves.
    fill_rect(&mut img, 0, h_half, w, h_half + 1, Rgb([160u8, 160, 160]));

    // ── Bottom half: vertical line bands ─────────────────────────────────────
    let margin_h = (w / 30).max(4);
    let usable_w = w.saturating_sub(2 * margin_h);
    let group_w = (usable_w / nc).max(1);
    let line_thick_v = (group_w / (line_count * 2 + 2)).clamp(1, 4);
    let gap_v = line_thick_v;

    for (ci, &color) in COLORS.iter().enumerate() {
        let ci = ci as u32;
        let group_left = margin_h + ci * group_w;
        for line in 0..line_count {
            let x0 = group_left + line * (line_thick_v + gap_v);
            let x1 = (x0 + line_thick_v).min(w.saturating_sub(1));
            fill_rect(&mut img, x0, h_half + 4, x1, h - margin_v, color);
        }
    }

    draw_border(&mut img, Rgb([60u8, 60, 60]));
    img
}

// ── ALIGN ─────────────────────────────────────────────────────────────────────

/// Alignment / registration crosses + edge tick marks.
///
/// Draws:
/// * A centred accent crosshair (the primary alignment reference).
/// * Four corner registration crosses with a small open square around them.
/// * Tick marks at regular intervals along all four page edges
///   (even ticks longer, odd shorter; `page_index % 4` shifts the count
///   so adjacent pages look subtly different).
/// * Mid-edge registration marks (top, bottom, left, right centres).
pub fn draw_align(w: u32, h: u32, page_index: u32) -> RgbImage {
    let mut img: RgbImage = ImageBuffer::from_pixel(w, h, Rgb([252u8, 252, 248]));
    let ink = Rgb([10u8, 10, 10]);
    let accent = Rgb([170u8, 30, 30]);

    let thick = (w.min(h) / 200).clamp(1, 3);

    // Centre crosshair.
    let (cx, cy) = (w / 2, h / 2);
    let center_arm = w.min(h) / 12;
    draw_cross(&mut img, cx, cy, center_arm, thick + 1, accent);
    // Tiny solid square at the crosshair origin for precision.
    let dot = (thick + 1).max(2);
    fill_rect(
        &mut img,
        cx.saturating_sub(dot),
        cy.saturating_sub(dot),
        (cx + dot + 1).min(w),
        (cy + dot + 1).min(h),
        ink,
    );

    // Corner registration crosses + small open squares.
    let inset = (w.min(h) / 12).max(20u32);
    let corner_arm = (w.min(h) / 25).max(10u32);
    let sq_half = corner_arm / 2;
    let corners: [(u32, u32); 4] = [
        (inset, inset),
        (w.saturating_sub(inset + 1), inset),
        (inset, h.saturating_sub(inset + 1)),
        (w.saturating_sub(inset + 1), h.saturating_sub(inset + 1)),
    ];
    for (ocx, ocy) in corners {
        draw_cross(&mut img, ocx, ocy, corner_arm, thick, ink);
        // Open square outline around the corner cross centre.
        let qx0 = ocx.saturating_sub(sq_half);
        let qy0 = ocy.saturating_sub(sq_half);
        let qx1 = (ocx + sq_half + 1).min(w);
        let qy1 = (ocy + sq_half + 1).min(h);
        fill_rect(&mut img, qx0, qy0, qx1, (qy0 + thick).min(qy1), ink); // top
        fill_rect(&mut img, qx0, qy1.saturating_sub(thick), qx1, qy1, ink); // bot
        fill_rect(&mut img, qx0, qy0, (qx0 + thick).min(qx1), qy1, ink); // left
        fill_rect(&mut img, qx1.saturating_sub(thick), qy0, qx1, qy1, ink); // right
    }

    // Edge ticks: 8 + (page_index % 4) * 2 divisions per axis.
    let divs = 8u32 + (page_index % 4) * 2;
    let major_len = w.min(h) / 30;
    let minor_len = (major_len / 2).max(2);

    // Top + bottom.
    for i in 1..divs {
        let x = w * i / divs;
        let len = if i % 2 == 0 { major_len } else { minor_len };
        fill_rect(&mut img, x, 0, x + thick, len, ink);
        fill_rect(&mut img, x, h.saturating_sub(len), x + thick, h, ink);
    }
    // Left + right.
    for i in 1..divs {
        let y = h * i / divs;
        let len = if i % 2 == 0 { major_len } else { minor_len };
        fill_rect(&mut img, 0, y, len, y + thick, ink);
        fill_rect(&mut img, w.saturating_sub(len), y, w, y + thick, ink);
    }

    // Mid-edge crosses (top, bottom, left, right centres).
    let mid_arm = corner_arm / 2;
    draw_cross(&mut img, w / 2, 0, mid_arm, thick, ink);
    draw_cross(&mut img, w / 2, h.saturating_sub(1), mid_arm, thick, ink);
    draw_cross(&mut img, 0, h / 2, mid_arm, thick, ink);
    draw_cross(&mut img, w.saturating_sub(1), h / 2, mid_arm, thick, ink);

    draw_border(&mut img, ink);
    img
}

// ── GRID ──────────────────────────────────────────────────────────────────────

/// Margin frames + scale rules (inch / quarter-inch cues).
///
/// Letter size is 8.5 × 11 in, so at any DPI:
/// * `pixels_per_inch_x = w / 8.5 ≈ w * 10 / 85`
/// * `pixels_per_inch_y = h / 11 ≈ h * 10 / 110`
///
/// Draws: outer border; a 0.5-inch inner margin frame; light gray grid lines
/// at every inch; and tick marks along all four edges at inch + quarter-inch
/// intervals (`page_index % 2` toggles between 4 and 8 minor subdivisions).
pub fn draw_grid(w: u32, h: u32, page_index: u32) -> RgbImage {
    let mut img: RgbImage = ImageBuffer::from_pixel(w, h, Rgb([250u8, 250, 248]));
    let ink = Rgb([30u8, 30, 30]);
    let light = Rgb([185u8, 185, 185]);
    let thick = 1u32;

    // Pixels per inch (letter page: 8.5 × 11 in).
    let ppix = (w * 10 / 85).max(1);
    let ppiy = (h * 10 / 110).max(1);

    // Inner margin frame at ~0.5 in from the outer edge.
    let mx = ppix / 2;
    let my = ppiy / 2;
    if mx > 1 && my > 1 && mx < w / 2 && my < h / 2 {
        fill_rect(&mut img, mx, my, w - mx, my + thick, ink);
        fill_rect(&mut img, mx, h - my, w - mx, h - my + thick, ink);
        fill_rect(&mut img, mx, my, mx + thick, h - my, ink);
        fill_rect(&mut img, w - mx, my, w - mx + thick, h - my, ink);
    }

    // Light gray grid lines at every inch.
    for xi in 1u32..9 {
        let x = xi * ppix;
        if x < w {
            fill_rect(&mut img, x, 0, x + thick, h, light);
        }
    }
    for yi in 1u32..12 {
        let y = yi * ppiy;
        if y < h {
            fill_rect(&mut img, 0, y, w, y + thick, light);
        }
    }

    // Ruler tick marks: inch marks are longer; `minor_divs` sub-divisions per inch.
    let minor_divs = if page_index.is_multiple_of(2) {
        4u32
    } else {
        8
    };
    let major_tick = (ppiy / 4).max(6);
    let minor_tick = (major_tick / 2).max(3);

    // Top + bottom ruler.
    for xi in 0u32..=8 {
        for sub in 0..minor_divs {
            let x = xi * ppix + sub * ppix / minor_divs;
            if x >= w {
                continue;
            }
            let tlen = if sub == 0 { major_tick } else { minor_tick };
            fill_rect(&mut img, x, 0, x + thick, tlen, ink);
            fill_rect(&mut img, x, h.saturating_sub(tlen), x + thick, h, ink);
        }
    }
    // Left + right ruler.
    for yi in 0u32..=11 {
        for sub in 0..minor_divs {
            let y = yi * ppiy + sub * ppiy / minor_divs;
            if y >= h {
                continue;
            }
            let tlen = if sub == 0 { major_tick } else { minor_tick };
            fill_rect(&mut img, 0, y, tlen, y + thick, ink);
            fill_rect(&mut img, w.saturating_sub(tlen), y, w, y + thick, ink);
        }
    }

    // Draw outer border last so it overrides any grid line that clips the edge.
    draw_border(&mut img, ink);
    img
}

// ── LINES ─────────────────────────────────────────────────────────────────────

/// Resolution line ladder (coarse → fine spatial frequency).
///
/// The left half holds six groups of **horizontal** lines at decreasing
/// half-period widths `[32, 16, 8, 4, 3, 2]` pixels.  The right half holds
/// the same six frequencies as **vertical** lines.  A scanner that can resolve
/// the finest group will show distinct lines; one that cannot will render that
/// group as a uniform gray smear.
///
/// The half-period widths are in raw pixels, so the effective line pairs per
/// inch (lpi) scale with DPI: at 100 dpi the coarsest group is ≈ 1.6 lpi and
/// the finest ≈ 25 lpi.
pub fn draw_lines(w: u32, h: u32, _page_index: u32) -> RgbImage {
    let mut img: RgbImage = ImageBuffer::from_pixel(w, h, Rgb([255u8, 255, 255]));
    let ink = Rgb([10u8, 10, 10]);
    let sep = Rgb([190u8, 190, 190]);

    // Half-period (black half + white half) in pixels, coarse → fine.
    const LP_HALF: [u32; 6] = [32, 16, 8, 4, 3, 2];
    let ng = LP_HALF.len() as u32;
    let pad = 10u32;

    // ── Left half: horizontal lines ───────────────────────────────────────────
    let left_w = w / 2;
    let inner_h = h.saturating_sub(2 * pad);
    let group_h = inner_h / ng;

    for (gi, &half) in LP_HALF.iter().enumerate() {
        let gi = gi as u32;
        let g_top = pad + gi * group_h;
        let g_bot = (g_top + group_h).min(h.saturating_sub(pad));
        let mut y = g_top;
        let mut dark = true;
        while y < g_bot {
            let y1 = (y + half).min(g_bot);
            if dark {
                fill_rect(&mut img, w / 20, y, left_w - w / 20, y1, ink);
            }
            y = y1;
            dark = !dark;
        }
        // Light separator between groups.
        if gi + 1 < ng {
            fill_rect(
                &mut img,
                w / 20,
                g_bot.saturating_sub(1),
                left_w - w / 20,
                g_bot,
                sep,
            );
        }
    }

    // ── Right half: vertical lines ────────────────────────────────────────────
    let right_start = w / 2 + 2;
    let inner_w = w.saturating_sub(right_start + 4);
    let group_w = inner_w / ng;

    for (gi, &half) in LP_HALF.iter().enumerate() {
        let gi = gi as u32;
        let g_left = right_start + gi * group_w;
        let g_right = (g_left + group_w).min(w - 4);
        let mut x = g_left;
        let mut dark = true;
        while x < g_right {
            let x1 = (x + half).min(g_right);
            if dark {
                fill_rect(&mut img, x, pad, x1, h.saturating_sub(pad), ink);
            }
            x = x1;
            dark = !dark;
        }
        // Light separator between groups.
        if gi + 1 < ng {
            fill_rect(
                &mut img,
                g_right.saturating_sub(1),
                pad,
                g_right,
                h.saturating_sub(pad),
                sep,
            );
        }
    }

    // Centre divider between H and V halves.
    fill_rect(&mut img, w / 2, 0, w / 2 + 2, h, Rgb([100u8, 100, 100]));

    draw_border(&mut img, Rgb([40u8, 40, 40]));
    img
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image_gen::{job_page_pixels, page_pixels, render_pattern_rgb, PatternId};

    // ── size checks ───────────────────────────────────────────────────────────

    #[test]
    fn new_patterns_render_correct_size_at_100dpi() {
        let dpi = 100u32;
        // Encode / render use Brother optical crop, not full letter extent.
        let (ew, eh) = job_page_pixels(dpi);
        for pat in [
            PatternId::Cmyk,
            PatternId::Nozzle,
            PatternId::Align,
            PatternId::Grid,
            PatternId::Lines,
        ] {
            let (img, _) = render_pattern_rgb(pat, dpi, 0, None)
                .unwrap_or_else(|e| panic!("render {:?} failed: {e}", pat));
            assert_eq!(img.dimensions(), (ew, eh), "wrong dimensions for {pat:?}");
        }
    }

    // ── id round-trip ─────────────────────────────────────────────────────────

    #[test]
    fn new_pattern_ids_parse_and_round_trip() {
        for (input, expected) in [
            ("cmyk", PatternId::Cmyk),
            ("nozzle", PatternId::Nozzle),
            ("align", PatternId::Align),
            ("grid", PatternId::Grid),
            ("lines", PatternId::Lines),
        ] {
            let parsed =
                PatternId::parse(input).unwrap_or_else(|e| panic!("parse {input:?} failed: {e}"));
            assert_eq!(parsed, expected);
            assert_eq!(parsed.as_str(), input);
        }
    }

    // ── CMYK spot checks ──────────────────────────────────────────────────────

    #[test]
    fn cmyk_first_column_is_cyan() {
        let (w, h) = page_pixels(100);
        let img = draw_cmyk(w, h, 0);
        assert_eq!(img.dimensions(), (w, h));
        // First column (Cyan): solid bar near the top.
        // At ~1/4 of column width, 20px from the top should be cyan (R low, G+B high).
        let col_w = w / 4;
        let x = col_w / 4;
        let y = 20u32;
        let px = img.get_pixel(x, y);
        assert!(
            px[0] < 80,
            "cyan bar: R channel should be low, got {:?}",
            px
        );
        assert!(
            px[1] > 150,
            "cyan bar: G channel should be high, got {:?}",
            px
        );
        assert!(
            px[2] > 150,
            "cyan bar: B channel should be high, got {:?}",
            px
        );
    }

    #[test]
    fn cmyk_fourth_column_is_dark() {
        let (w, h) = page_pixels(100);
        let img = draw_cmyk(w, h, 0);
        // Fourth column (K): solid bar should be very dark.
        let col_w = w / 4;
        let x = 3 * col_w + col_w / 2;
        let y = 20u32;
        let px = img.get_pixel(x.min(w - 2), y);
        assert!(
            px[0] < 60 && px[1] < 60 && px[2] < 60,
            "K bar should be dark, got {:?}",
            px
        );
    }

    // ── NOZZLE spot checks ────────────────────────────────────────────────────

    #[test]
    fn nozzle_has_colored_lines_in_top_half() {
        let (w, h) = page_pixels(100);
        let img = draw_nozzle(w, h, 0);
        assert_eq!(img.dimensions(), (w, h));
        // Scan the centre-x column in the top half; expect at least one
        // non-white pixel (some colored line).
        let cx = w / 2;
        let has_color = (0..h / 2).any(|y| {
            let px = img.get_pixel(cx, y);
            px[0] < 230 || px[1] < 230 || px[2] < 230
        });
        assert!(
            has_color,
            "nozzle top half should have non-white pixels at centre x"
        );
    }

    #[test]
    fn nozzle_has_colored_lines_in_bottom_half() {
        let (w, h) = page_pixels(100);
        let img = draw_nozzle(w, h, 0);
        // Scan a row in the bottom half for non-white pixels.
        let mid_y = h * 3 / 4;
        let has_color = (0..w).any(|x| {
            let px = img.get_pixel(x, mid_y);
            px[0] < 230 || px[1] < 230 || px[2] < 230
        });
        assert!(
            has_color,
            "nozzle bottom half should have non-white pixels at mid row"
        );
    }

    // ── ALIGN spot checks ─────────────────────────────────────────────────────

    #[test]
    fn align_centre_crosshair_is_dark() {
        let (w, h) = page_pixels(100);
        let img = draw_align(w, h, 0);
        assert_eq!(img.dimensions(), (w, h));
        // The centre of the page should be dark (accent crosshair).
        let (cx, cy) = (w / 2, h / 2);
        let px = img.get_pixel(cx, cy);
        assert!(
            px[0] < 200 || px[1] < 200 || px[2] < 200,
            "centre pixel should be non-white (crosshair), got {:?}",
            px
        );
    }

    #[test]
    fn align_top_left_corner_mark_present() {
        let (w, h) = page_pixels(100);
        let img = draw_align(w, h, 0);
        // Top-left corner registration cross: vertical arm runs through (inset, inset).
        // Check a pixel on the vertical arm of that cross.
        let inset = ((w.min(h) / 12).max(20)).min(w / 3);
        let arm = (w.min(h) / 25).max(10);
        // Scan the column at x=inset over the arm range.
        let found_dark = (inset.saturating_sub(arm)..=(inset + arm).min(h - 1)).any(|y| {
            let px = img.get_pixel(inset.min(w - 1), y);
            px[0] < 100 && px[1] < 100 && px[2] < 100
        });
        assert!(
            found_dark,
            "align: no dark pixel on TL corner cross vertical arm (inset={inset}, arm={arm})"
        );
    }

    // ── GRID spot checks ──────────────────────────────────────────────────────

    #[test]
    fn grid_border_is_dark() {
        let (w, h) = page_pixels(100);
        let img = draw_grid(w, h, 0);
        assert_eq!(img.dimensions(), (w, h));
        // Top-left corner and mid-top border pixel should be dark.
        let corner = img.get_pixel(0, 0);
        assert!(
            corner[0] < 100 && corner[1] < 100 && corner[2] < 100,
            "top-left corner should be dark ink, got {:?}",
            corner
        );
        // Mid-top: grid lines may hit the border at ppix multiples, but
        // at w/2 (= 425) the border should be dark (no grid line there).
        let mid_top = img.get_pixel(w / 2, 0);
        assert!(
            mid_top[0] < 100,
            "top centre border should be dark, got {:?}",
            mid_top
        );
    }

    #[test]
    fn grid_inner_margin_frame_visible() {
        let (w, h) = page_pixels(100);
        let img = draw_grid(w, h, 0);
        // Inner margin frame top edge is at y ≈ ppiy/2 ≈ 50 px at 100dpi.
        // At x = w/2 (centre), we should hit the inner frame top edge.
        let ppiy = (h * 10 / 110).max(1);
        let my = ppiy / 2;
        if my > 1 && my < h - 1 {
            let px = img.get_pixel(w / 2, my);
            assert!(
                px[0] < 100,
                "inner margin frame top edge should be dark at x=w/2, y={my}; got {:?}",
                px
            );
        }
    }

    // ── LINES spot checks ─────────────────────────────────────────────────────

    #[test]
    fn lines_coarsest_group_starts_dark() {
        let (w, h) = page_pixels(100);
        let img = draw_lines(w, h, 0);
        assert_eq!(img.dimensions(), (w, h));
        // The coarsest H group (half-period 32 px) starts at y=10 (pad).
        // At x = w/20 + 5 (inside the left margin), y=12 should be dark (first line).
        let x = w / 20 + 5;
        let y = 12u32;
        let px = img.get_pixel(x, y);
        assert!(
            px[0] < 100 && px[1] < 100 && px[2] < 100,
            "first H-line group should start dark at ({x},{y}), got {:?}",
            px
        );
    }

    #[test]
    fn lines_coarsest_group_first_gap_is_white() {
        let (w, h) = page_pixels(100);
        let img = draw_lines(w, h, 0);
        // First dark band in group 0 is y=10..42 (32 px). The white gap follows.
        // At y=45 (into the white gap), the pixel should be light.
        let x = w / 20 + 5;
        let y = 45u32;
        let px = img.get_pixel(x, y);
        assert!(
            px[0] > 180 && px[1] > 180 && px[2] > 180,
            "first H-line group gap should be white at ({x},{y}), got {:?}",
            px
        );
    }
}
