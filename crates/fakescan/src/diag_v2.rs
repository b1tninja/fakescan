//! Layout v2 of the `diag` page: print marks, resolution / alignment targets and colour targets,
//! all placed in areas layout v1 leaves empty (v1 pixels are never moved or altered).
//!
//! Every element is described once in [`elements`] (position and size in millimetres in the page
//! coordinate system, expected values, ground-truth sample points) and drawn by [`draw`] from the
//! same constants, so [`layout_json`] and the rendered pixels cannot drift apart (tests sample
//! every element's points). Colours are 8-bit sRGB code values; the colour space is declared in
//! the JSON and in the PNG written by [`png_bytes`]. Everything is deterministic.
//!
//! Page coordinate system: millimetres, origin top-left of the 206 x 288 mm layout box, x right,
//! y down (`px = round(mm * dpi / 25.4)`, dpi = the effective dpi; see `diag::effective_dpi`).
//! Elements with `anchor: "raster"` are positioned from the raster edges instead (the physical
//! sheet edge), which lie a little beyond the layout box.

use crate::image_gen::{self, diag, fill_rect, glyph_rows};
use image::{Rgb, RgbImage};
use serde_json::{json, Value};

pub const LAYOUT_VERSION: u32 = 2;
pub const COLOUR_SPACE: &str = "sRGB";
pub const SCHEMA: &str = "fakescan.diag.layout/1";

const INK: Rgb<u8> = Rgb([0, 0, 0]);
/// Printable-area frame colour (distinct from the black page border).
pub const FRAME_RGB: [u8; 3] = [0, 90, 200];

/// Printable-area frame: outer rectangle in mm; the 0.4 mm line is drawn inside it.
/// The top edge sits just below the tester banner (banner < 11.3 mm), the other three are
/// `PRINTABLE_INSET_MM` from the layout box.
pub const PRINTABLE_INSET_MM: f64 = 3.0;
pub const PRINTABLE: diag::RectMm = (3.0, 12.5, 203.0, 285.0);
pub const FRAME_LINE_MM: f64 = 0.4;

/// Crop marks and centre marks.
pub const MARK_LINE_MM: f64 = 0.3;
pub const CENTRE_X_MM: f64 = 103.0;
pub const CENTRE_Y_MM: f64 = 144.0;

/// Registration crosses (ring radius 3.0, arms 3.8, line 0.3 mm) at these centres.
pub const REG: [(&str, f64, f64); 4] = [
    ("reg_bl", 27.0, 278.0),
    ("reg_br", 192.0, 250.0),
    ("reg_tr", 195.0, 40.0),
    ("reg_centre", CENTRE_X_MM, CENTRE_Y_MM),
];
pub const REG_RING_MM: f64 = 3.0;
pub const REG_ARM_MM: f64 = 3.8;

/// Graduated rulers (see [`rulers`]) and the long measuring bar.
pub const MEASURE_BAR: diag::RectMm = (6.0, 266.2, 196.0, 267.4);
pub const MEASURE_LEN_MM: f64 = 190.0;
pub const MEASURE_TEXT: [(f64, f64, &str); 2] = [
    (24.0, 268.0, "MEASURE HERE: THIS BAR IS EXACTLY 190 MM"),
    (
        24.0,
        270.0,
        "SCALE CHECK: PUT A RULER ON THE BAR; IT SHOULD READ 190 MM WITHIN 1 MM",
    ),
];
pub const RULER_LABEL_PT: u32 = 6;
pub const RULER_LINE_MM: f64 = 0.2;

/// Hue wheel, Siemens star, rings (centre x, y, radius mm).
pub const WHEEL: (f64, f64, f64) = (175.0, 70.0, 16.0);
pub const STAR: (f64, f64, f64) = (175.0, 104.0, 16.0);
pub const STAR_CYCLES: u32 = 36;

/// Sierpinski triangle (apex up, equilateral): left base corner x, top y, side, depth.
pub const SIERP_X0: f64 = 102.0;
pub const SIERP_TOP: f64 = 152.0;
pub const SIERP_SIDE: f64 = 36.0;
pub const SIERP_DEPTH: u32 = 7;
/// Golden construction: Fibonacci squares (size, x, y in units) tiling a 21 x 13 unit rectangle
/// whose top-left is at `GOLD_ORIGIN`, plus one quarter-circle per square (centre x, y, r, a0, a1 deg,
/// angles in image coordinates) forming the spiral.
pub const GOLD_ORIGIN: (f64, f64) = (140.0, 152.0);
pub const GOLD_UNIT_MM: f64 = 2.0;
pub const GOLD_SQUARES: [(f64, f64, f64); 7] = [
    (13.0, 0.0, 0.0),
    (8.0, 13.0, 0.0),
    (5.0, 16.0, 8.0),
    (3.0, 13.0, 10.0),
    (2.0, 13.0, 8.0),
    (1.0, 15.0, 8.0),
    (1.0, 15.0, 9.0),
];
pub const GOLD_ARCS: [(f64, f64, f64, f64, f64); 7] = [
    (13.0, 13.0, 13.0, 180.0, 270.0),
    (13.0, 8.0, 8.0, 270.0, 360.0),
    (16.0, 8.0, 5.0, 0.0, 90.0),
    (16.0, 10.0, 3.0, 90.0, 180.0),
    (15.0, 10.0, 2.0, 180.0, 270.0),
    (15.0, 9.0, 1.0, 270.0, 360.0),
    (15.0, 9.0, 1.0, 0.0, 90.0),
];
/// Concentric circles (centre, radii), circle in square with diagonals, hexagon, right-angle markers.
pub const CIRCLES: (f64, f64) = (148.0, 189.0);
pub const CIRCLE_RADII_MM: [f64; 4] = [2.0, 4.0, 6.0, 8.0];
pub const SQUARE_CIRCLE: diag::RectMm = (158.0, 181.0, 174.0, 197.0);
pub const HEXAGON: (f64, f64, f64) = (183.0, 189.0, 7.5);
pub const MARKER_ARM_MM: f64 = 2.4;
/// (cell x0, y0, x1, y1, angle deg)
pub const MARKERS: [(f64, f64, f64, f64, f64); 4] = [
    (184.0, 199.0, 188.75, 203.0, 0.0),
    (188.75, 199.0, 193.5, 203.0, 20.0),
    (184.0, 203.0, 188.75, 207.0, 40.0),
    (188.75, 203.0, 193.5, 207.0, 60.0),
];
/// Slanted-edge squares: (id, centre x, centre y, angle deg); half side 6 mm, dark 40 on paper.
pub const SLANT: [(&str, f64, f64, f64); 2] = [
    ("slant_edge_a", 166.0, 131.0, 5.0),
    ("slant_edge_b", 184.0, 131.0, -5.0),
];
pub const SLANT_HALF_MM: f64 = 6.0;
pub const SLANT_DARK: u8 = 40;
/// Line-pair ladders: 8 patches of 4 x 4 mm, period (one ink + one paper line) in mm.
pub const RES_PERIODS_MM: [f64; 8] = [2.0, 1.0, 0.75, 0.5, 0.4, 0.3, 0.25, 0.2];
pub const RES_X0_MM: f64 = 159.0;
pub const RES_PATCH_MM: f64 = 4.0;
pub const RES_V_Y: (f64, f64) = (141.0, 145.0);
pub const RES_H_Y: (f64, f64) = (146.0, 150.0);
/// Pixel-exact line pairs: 4 patches of 6 x 6 mm: 1px V, 1px H, 2px V, 2px H (1 or 2 px ink, same paper).
pub const PX_LINES: (f64, f64, f64) = (140.0, 199.0, 5.0);
pub const GRID: diag::RectMm = (162.0, 199.0, 182.0, 207.0);

/// Ramps: 100 mm wide, 5 mm tall, 6 mm pitch, level = round(255 * (x - x0) / width).
pub const RAMP_X: (f64, f64) = (26.0, 126.0);
pub const RAMP_Y0: f64 = 184.0;
pub const RAMP_H: f64 = 5.0;
pub const RAMP_PITCH: f64 = 6.0;
pub const RAMP_IDS: [&str; 4] = ["ramp_r", "ramp_g", "ramp_b", "ramp_n"];
pub const STEPS_Y: (f64, f64) = (209.0, 217.0);
pub const STEP_OUTLINE_MM: f64 = 0.3;
pub const NEAR_BLACK: [u8; 8] = [1, 2, 3, 4, 6, 8, 12, 16];
pub const NEAR_WHITE: [u8; 8] = [254, 253, 252, 251, 249, 247, 243, 239];
pub const NEAR_BLACK_X: (f64, f64) = (26.0, 66.0);
pub const NEAR_WHITE_X: (f64, f64) = (68.0, 108.0);
pub const CLIP_BLACK_X: (f64, f64) = (110.0, 118.0);
pub const CLIP_WHITE_X: (f64, f64) = (118.0, 126.0);
pub const HALF_Y: (f64, f64) = (219.0, 229.0);
pub const HALF_X0: f64 = 26.0;
pub const HALF_W: f64 = 6.0;
pub const HALF_LEVEL: u8 = 128;
pub const SKIN: [[u8; 3]; 3] = [[255, 224, 196], [224, 172, 125], [141, 85, 60]];
pub const SKIN_X0: f64 = 64.0;
pub const GREYS: [u8; 2] = [118, 128];
pub const GREYS_X0: f64 = 84.0;
pub const GREY_W: f64 = 6.0;

/// Text blocks: (pt, top y mm); x = 26. Glyphs are the built-in 5x7 font, cap height 0.72 em.
pub const TEXT_X: f64 = 26.0;
pub const TEXT_LINES: [(u32, f64); 4] = [(12, 232.0), (10, 237.5), (8, 242.5), (6, 246.5)];
pub const TEXT_BODY: &str = "SCAN TEST 0123456789 ABCXYZ";
pub const LABELS: [&str; 4] = [
    "PAGE 206X288 MM LAYOUT V2",
    "PAGE EDGE: BLACK HAIRLINE AT RASTER EDGE",
    "PRINTABLE AREA INSET 3 MM: BLUE FRAME",
    "COLOURS ARE SRGB CODE VALUES",
];
pub const LABEL_PT: u32 = 6;
pub const LABEL_Y0: f64 = 251.0;
pub const LABEL_PITCH: f64 = 3.5;

/// Sheet outlines drawn dashed when they fit inside the raster (they never do for the 206 x 288
/// box; the entries are kept in the layout so loops can see the sizes and that they were skipped).
pub const SHEETS: [(&str, f64, f64); 2] =
    [("sheet_a4", 210.0, 297.0), ("sheet_letter", 215.9, 279.4)];

fn pxmm(dpi: u32) -> f64 {
    dpi as f64 / 25.4
}

fn px(mm: f64, dpi: u32) -> u32 {
    diag::px(mm, dpi)
}

/// Fill a rectangle given in mm; never thinner than one pixel.
fn fill_mm(img: &mut RgbImage, dpi: u32, r: diag::RectMm, c: Rgb<u8>) {
    let (x0, y0) = (px(r.0, dpi), px(r.1, dpi));
    let x1 = px(r.2, dpi).max(x0 + 1);
    let y1 = px(r.3, dpi).max(y0 + 1);
    fill_rect(img, x0, y0, x1, y1, c);
}

fn hsv_to_rgb(h_deg: f64, s: f64, v: f64) -> [u8; 3] {
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

/// Hue (degrees, image coordinates: 0 = +x, increasing clockwise on screen) at offset (dx, dy).
pub fn wheel_hue_deg(dx: f64, dy: f64) -> f64 {
    dy.atan2(dx).to_degrees().rem_euclid(360.0)
}

/// Text geometry: mm per glyph cell for a size in points, and the 5x7 ink fraction of a string.
pub fn text_unit_mm(pt: u32) -> f64 {
    0.72 * pt as f64 * 25.4 / 72.0 / 7.0
}

fn text_cells(text: &str) -> (usize, usize) {
    let mut ink = 0;
    for ch in text.chars() {
        for row in glyph_rows(ch) {
            ink += (row & 0x1f).count_ones() as usize;
        }
    }
    let total = (text.chars().count() * 6 - 1) * 7;
    (ink, total)
}

/// (width mm, height mm, analytic ink fraction) of a text block.
pub fn text_extent(text: &str, pt: u32) -> (f64, f64, f64) {
    let u = text_unit_mm(pt);
    let (ink, total) = text_cells(text);
    (
        (text.chars().count() * 6 - 1) as f64 * u,
        7.0 * u,
        ink as f64 / total as f64,
    )
}

fn draw_text(img: &mut RgbImage, dpi: u32, x: f64, y: f64, text: &str, pt: u32, c: Rgb<u8>) {
    let u = text_unit_mm(pt);
    for (i, ch) in text.chars().enumerate() {
        for (row, bits) in glyph_rows(ch).iter().enumerate() {
            for col in 0..5usize {
                if bits & (1 << (4 - col)) != 0 {
                    let cx = x + (i * 6 + col) as f64 * u;
                    let cy = y + row as f64 * u;
                    let (x0, y0) = (px(cx, dpi), px(cy, dpi));
                    let (x1, y1) = (px(cx + u, dpi), px(cy + u, dpi));
                    if x1 > x0 && y1 > y0 {
                        fill_rect(img, x0, y0, x1, y1, c);
                    }
                }
            }
        }
    }
}

/// Ink coverage (0..1) of a 50 % duty square wave of `period` mm (ink first) over `[a, b)` mm.
fn stripe_coverage(a: f64, b: f64, origin: f64, period: f64) -> f64 {
    const N: usize = 8;
    let mut ink = 0;
    for s in 0..N {
        let t = a + (b - a) * (s as f64 + 0.5) / N as f64;
        if ((t - origin) / period).rem_euclid(1.0) < 0.5 {
            ink += 1;
        }
    }
    ink as f64 / N as f64
}

fn gray(v: f64) -> Rgb<u8> {
    let l = v.round().clamp(0.0, 255.0) as u8;
    Rgb([l, l, l])
}

fn dashed_rect(img: &mut RgbImage, dpi: u32, r: diag::RectMm, c: Rgb<u8>) {
    let t = MARK_LINE_MM;
    let mut x = r.0;
    while x < r.2 {
        let e = (x + 4.0).min(r.2);
        fill_mm(img, dpi, (x, r.1, e, r.1 + t), c);
        fill_mm(img, dpi, (x, r.3 - t, e, r.3), c);
        x += 6.0;
    }
    let mut y = r.1;
    while y < r.3 {
        let e = (y + 4.0).min(r.3);
        fill_mm(img, dpi, (r.0, y, r.0 + t, e), c);
        fill_mm(img, dpi, (r.2 - t, y, r.2, e), c);
        y += 6.0;
    }
}

/// Draw the v2 elements over a rendered v1 page (`dpi` = effective dpi).
pub fn draw(img: &mut RgbImage, dpi: u32) {
    let (w, h) = img.dimensions();
    let k = pxmm(dpi);
    let (wm, hm) = (w as f64 / k, h as f64 / k);
    let frame = Rgb(FRAME_RGB);
    let t = MARK_LINE_MM;

    // --- print marks -------------------------------------------------------------------------
    // Printable-area frame (blue), line drawn inside the outer rectangle.
    let p = PRINTABLE;
    let f = FRAME_LINE_MM;
    for r in [
        (p.0, p.1, p.0 + f, p.3),
        (p.2 - f, p.1, p.2, p.3),
        (p.0, p.1, p.2, p.1 + f),
        (p.0, p.3 - f, p.2, p.3),
    ] {
        fill_mm(img, dpi, r, frame);
    }
    // Sheet outlines that fit the raster (none do for A4 / Letter on this box).
    for (_, sw, sh) in SHEETS {
        if sw <= wm && sh <= hm {
            dashed_rect(img, dpi, (0.0, 0.0, sw, sh), INK);
        }
    }
    // Raster corner ticks (0.5 mm in from the physical sheet edge, 8 mm arms, 0.3 mm line).
    let (o, a) = (0.5, 8.0);
    for r in [
        (o, o, o + a, o + t),
        (o, o, o + t, o + a),
        (wm - o - a, o, wm - o, o + t),
        (wm - o - t, o, wm - o, o + a),
        (o, hm - o - t, o + a, hm - o),
        (o, hm - o - a, o + t, hm - o),
        (wm - o - a, hm - o - t, wm - o, hm - o),
        (wm - o - t, hm - o - a, wm - o, hm - o),
    ] {
        fill_mm(img, dpi, r, INK);
    }
    // Crop marks: in the margin, 1.3 mm outside the frame edges (top ones are vertical only).
    for r in [
        (1.7, 12.6, 2.0, 18.6),
        (204.0, 12.6, 204.3, 18.6),
        (1.7, 279.6, 2.0, 285.6),
        (204.0, 279.6, 204.3, 285.6),
        (1.7, 286.3, 7.7, 286.6),
        (198.3, 286.3, 204.3, 286.6),
    ] {
        fill_mm(img, dpi, r, INK);
    }
    // Centre marks on each edge.
    for r in centre_mark_rects() {
        fill_mm(img, dpi, r.1, INK);
    }
    // Registration crosses.
    for (_, cx, cy) in REG {
        draw_reg(img, dpi, cx, cy);
    }
    // Graduated rulers, measuring bar and instructions.
    let wpx = tick_px(dpi);
    for r in rulers() {
        r.draw(img, dpi, wpx);
    }
    fill_mm(img, dpi, MEASURE_BAR, INK);
    for x in [MEASURE_BAR.0, MEASURE_BAR.2] {
        fill_mm(img, dpi, (x - 0.15, 264.8, x + 0.15, MEASURE_BAR.1), INK);
    }
    for (x, y, t) in MEASURE_TEXT {
        draw_text(img, dpi, x, y, t, RULER_LABEL_PT, INK);
    }

    // --- alignment / resolution --------------------------------------------------------------
    let (cx, cy, r) = WHEEL;
    for y in px(cy - r, dpi)..=px(cy + r, dpi).min(h - 1) {
        for x in px(cx - r, dpi)..=px(cx + r, dpi).min(w - 1) {
            let (dx, dy) = ((x as f64 + 0.5) / k - cx, (y as f64 + 0.5) / k - cy);
            let d = dx.hypot(dy);
            if d <= r {
                img.put_pixel(x, y, Rgb(hsv_to_rgb(wheel_hue_deg(dx, dy), d / r, 1.0)));
            }
        }
    }
    let (cx, cy, r) = STAR;
    for y in px(cy - r, dpi)..=px(cy + r, dpi).min(h - 1) {
        for x in px(cx - r, dpi)..=px(cx + r, dpi).min(w - 1) {
            let (dx, dy) = ((x as f64 + 0.5) / k - cx, (y as f64 + 0.5) / k - cy);
            if dx.hypot(dy) <= r && (STAR_CYCLES as f64 * dy.atan2(dx)).sin() >= 0.0 {
                img.put_pixel(x, y, INK);
            }
        }
    }
    draw_geometry(img, dpi);
    for (_, cx, cy, ang) in SLANT {
        let (sa, ca) = ang.to_radians().sin_cos();
        let e = SLANT_HALF_MM * (ca.abs() + sa.abs()) + 0.2;
        for y in px(cy - e, dpi)..=px(cy + e, dpi).min(h - 1) {
            for x in px(cx - e, dpi)..=px(cx + e, dpi).min(w - 1) {
                let mut hit = 0;
                for sy in 0..4 {
                    for sx in 0..4 {
                        let dx = (x as f64 + (sx as f64 + 0.5) / 4.0) / k - cx;
                        let dy = (y as f64 + (sy as f64 + 0.5) / 4.0) / k - cy;
                        let (u, v) = (dx * ca + dy * sa, -dx * sa + dy * ca);
                        if u.abs() <= SLANT_HALF_MM && v.abs() <= SLANT_HALF_MM {
                            hit += 1;
                        }
                    }
                }
                if hit > 0 {
                    let cov = hit as f64 / 16.0;
                    img.put_pixel(x, y, gray(255.0 - cov * (255.0 - SLANT_DARK as f64)));
                }
            }
        }
    }
    for (i, &period) in RES_PERIODS_MM.iter().enumerate() {
        let x0 = RES_X0_MM + i as f64 * RES_PATCH_MM;
        let x1 = x0 + RES_PATCH_MM;
        for xp in px(x0, dpi)..px(x1, dpi) {
            let cov = stripe_coverage(xp as f64 / k, (xp + 1) as f64 / k, x0, period);
            let c = gray(255.0 * (1.0 - cov));
            fill_rect(img, xp, px(RES_V_Y.0, dpi), xp + 1, px(RES_V_Y.1, dpi), c);
        }
        for yp in px(RES_H_Y.0, dpi)..px(RES_H_Y.1, dpi) {
            let cov = stripe_coverage(yp as f64 / k, (yp + 1) as f64 / k, RES_H_Y.0, period);
            let c = gray(255.0 * (1.0 - cov));
            fill_rect(img, px(x0, dpi), yp, px(x1, dpi), yp + 1, c);
        }
    }
    let (px0, py0, ps) = PX_LINES;
    for i in 0..4u32 {
        let (x0, y0) = (px(px0 + i as f64 * ps, dpi), px(py0, dpi));
        let (x1, y1) = (px(px0 + (i + 1) as f64 * ps, dpi), px(py0 + ps, dpi));
        let on = 1 + i / 2; // ink width in px: 1, 1, 2, 2
        let vertical = i % 2 == 0;
        for y in y0..y1 {
            for x in x0..x1 {
                let n = if vertical { x - x0 } else { y - y0 };
                if n % (2 * on) < on {
                    img.put_pixel(x, y, INK);
                }
            }
        }
    }
    // Fine line grid, 1 mm pitch; every 5th line heavier.
    let g = GRID;
    for kx in 0..=((g.2 - g.0) as u32) {
        let heavy = kx % 5 == 0;
        let wpx = if heavy {
            px(0.3, dpi).max(2)
        } else {
            px(0.15, dpi).max(1)
        };
        let x = px(g.0 + kx as f64, dpi);
        fill_rect(img, x, px(g.1, dpi), x + wpx, px(g.3, dpi) + wpx, INK);
    }
    for ky in 0..=((g.3 - g.1) as u32) {
        let heavy = ky % 5 == 0;
        let wpx = if heavy {
            px(0.3, dpi).max(2)
        } else {
            px(0.15, dpi).max(1)
        };
        let y = px(g.1 + ky as f64, dpi);
        fill_rect(img, px(g.0, dpi), y, px(g.2, dpi) + wpx, y + wpx, INK);
    }

    // --- colour -------------------------------------------------------------------------------
    for (i, _) in RAMP_IDS.iter().enumerate() {
        let y0 = RAMP_Y0 + i as f64 * RAMP_PITCH;
        for x in px(RAMP_X.0, dpi)..px(RAMP_X.1, dpi) {
            let l = ramp_level((x as f64 + 0.5) / k);
            let c = match i {
                0 => Rgb([l, 0, 0]),
                1 => Rgb([0, l, 0]),
                2 => Rgb([0, 0, l]),
                _ => Rgb([l, l, l]),
            };
            fill_rect(img, x, px(y0, dpi), x + 1, px(y0 + RAMP_H, dpi), c);
        }
    }
    step_strip(img, dpi, NEAR_BLACK_X, &NEAR_BLACK);
    step_strip(img, dpi, NEAR_WHITE_X, &NEAR_WHITE);
    step_strip(img, dpi, CLIP_BLACK_X, &[0]);
    step_strip(img, dpi, CLIP_WHITE_X, &[255]);
    for i in 0..6 {
        let c = diag::PATCH_COLORS[i].map(|v| if v == 255 { HALF_LEVEL } else { 0 });
        let x = HALF_X0 + i as f64 * HALF_W;
        fill_mm(img, dpi, (x, HALF_Y.0, x + HALF_W, HALF_Y.1), Rgb(c));
    }
    for (i, c) in SKIN.iter().enumerate() {
        let x = SKIN_X0 + i as f64 * HALF_W;
        fill_mm(img, dpi, (x, HALF_Y.0, x + HALF_W, HALF_Y.1), Rgb(*c));
    }
    for (i, l) in GREYS.iter().enumerate() {
        let x = GREYS_X0 + i as f64 * GREY_W;
        fill_mm(
            img,
            dpi,
            (x, HALF_Y.0, x + GREY_W, HALF_Y.1),
            Rgb([*l, *l, *l]),
        );
    }
    for (pt, y) in TEXT_LINES {
        draw_text(img, dpi, TEXT_X, y, &text_for(pt), pt, INK);
    }
    for (i, l) in LABELS.iter().enumerate() {
        draw_text(
            img,
            dpi,
            TEXT_X,
            LABEL_Y0 + i as f64 * LABEL_PITCH,
            l,
            LABEL_PT,
            INK,
        );
    }
}

/// Block geometry of one near-black / near-white / clip strip: outline plus equal patches.
fn strip_patch_rect(xr: (f64, f64), n: usize, i: usize) -> diag::RectMm {
    let o = STEP_OUTLINE_MM;
    let pw = (xr.1 - xr.0 - 2.0 * o) / n as f64;
    let x = xr.0 + o + i as f64 * pw;
    (x, STEPS_Y.0 + o, x + pw, STEPS_Y.1 - o)
}

fn step_strip(img: &mut RgbImage, dpi: u32, xr: (f64, f64), levels: &[u8]) {
    fill_mm(img, dpi, (xr.0, STEPS_Y.0, xr.1, STEPS_Y.1), INK);
    for (i, &l) in levels.iter().enumerate() {
        fill_mm(
            img,
            dpi,
            strip_patch_rect(xr, levels.len(), i),
            Rgb([l, l, l]),
        );
    }
}

/// Ruler tick width in device pixels: 1 px up to ~150 dpi, then ~0.2 mm.
pub fn tick_px(dpi: u32) -> u32 {
    ((RULER_LINE_MM * pxmm(dpi)).round() as u32).max(1)
}

/// One graduated ruler: ticks every `pitch` mm along an axis from `a0`, baseline at `b`.
pub struct Ruler {
    pub id: &'static str,
    pub unit: &'static str,
    pub horizontal: bool,
    pub a0: f64,
    pub b: f64,
    pub pitch: f64,
    pub n: u32,
    /// (divisor, tick length mm): the longest tick whose divisor divides the index is used.
    pub tiers: &'static [(u32, f64)],
    /// Ticks point towards +perp (down / right) when true, else up / left.
    pub positive: bool,
    /// Number every `label_every` ticks (value = index / label_every); 0 = unlabelled.
    pub label_every: u32,
}

impl Ruler {
    pub fn tick_len(&self, i: u32) -> f64 {
        self.tiers
            .iter()
            .rev()
            .find(|(d, _)| i.is_multiple_of(*d))
            .map(|t| t.1)
            .unwrap_or(self.tiers[0].1)
    }
    pub fn pos(&self, i: u32) -> f64 {
        self.a0 + i as f64 * self.pitch
    }
    fn long_len(&self) -> f64 {
        self.tiers.last().unwrap().1
    }
    /// Label text and top-left anchor (x, y mm) for tick `i`.
    pub fn label(&self, i: u32) -> Option<(String, f64, f64)> {
        if self.label_every == 0 || !i.is_multiple_of(self.label_every) {
            return None;
        }
        let text = (i / self.label_every).to_string();
        let (tw, th, _) = text_extent(&text, RULER_LABEL_PT);
        let gap = 0.4;
        Some(if self.horizontal {
            let y = if self.positive {
                self.b + self.long_len() + gap
            } else {
                self.b - self.long_len() - gap - th
            };
            (text, self.pos(i) + 0.1 - tw / 2.0, y)
        } else {
            let x = if self.positive {
                self.b + self.long_len() + gap
            } else {
                self.b - self.long_len() - gap - tw
            };
            (text, x, self.pos(i) + 0.1 - th / 2.0)
        })
    }
    pub fn rect(&self) -> diag::RectMm {
        let l = self.long_len();
        let a1 = self.pos(self.n) + 0.3;
        let (p0, p1) = if self.positive {
            (self.b, self.b + l)
        } else {
            (self.b - l, self.b + 0.3)
        };
        if self.horizontal {
            (self.a0 - 0.3, p0, a1, p1)
        } else {
            (p0, self.a0 - 0.3, p1, a1)
        }
    }
    fn draw(&self, img: &mut RgbImage, dpi: u32, wpx: u32) {
        let bpx = px(self.b, dpi);
        let (a_start, a_end) = (px(self.a0, dpi), px(self.pos(self.n), dpi) + wpx);
        if self.horizontal {
            fill_rect(img, a_start, bpx, a_end, bpx + wpx, INK);
        } else {
            fill_rect(img, bpx, a_start, bpx + wpx, a_end, INK);
        }
        for i in 0..=self.n {
            let a = px(self.pos(i), dpi);
            let len = px(self.tick_len(i), dpi).max(1);
            let (p0, p1) = if self.positive {
                (bpx, bpx + len + wpx)
            } else {
                ((bpx + wpx).saturating_sub(len + wpx), bpx + wpx)
            };
            if self.horizontal {
                fill_rect(img, a, p0, a + wpx, p1, INK);
            } else {
                fill_rect(img, p0, a, p1, a + wpx, INK);
            }
            if let Some((t, x, y)) = self.label(i) {
                draw_text(img, dpi, x, y, &t, RULER_LABEL_PT, INK);
            }
        }
    }
}

const METRIC_TIERS_H: &[(u32, f64)] = &[(1, 1.5), (5, 2.5), (10, 3.5)];
const METRIC_TIERS_V: &[(u32, f64)] = &[(1, 2.0), (5, 3.0), (10, 4.0)];
const INCH_TIERS_H: &[(u32, f64)] = &[(1, 1.2), (2, 1.8), (4, 2.4), (8, 3.0), (16, 3.6)];
const INCH_TIERS_V: &[(u32, f64)] = &[(1, 1.0), (2, 1.6), (4, 2.2), (8, 2.8), (16, 3.2)];

/// The four rulers: metric + inch, horizontal (bottom, shared baseline) and vertical (right edge).
pub fn rulers() -> Vec<Ruler> {
    vec![
        Ruler {
            id: "ruler_metric_h",
            unit: "mm",
            horizontal: true,
            a0: 34.0,
            b: 277.5,
            pitch: 1.0,
            n: 150,
            tiers: METRIC_TIERS_H,
            positive: false,
            label_every: 10,
        },
        Ruler {
            id: "ruler_inch_h",
            unit: "in/16",
            horizontal: true,
            a0: 34.0,
            b: 277.5,
            pitch: 25.4 / 16.0,
            n: 80,
            tiers: INCH_TIERS_H,
            positive: true,
            label_every: 16,
        },
        Ruler {
            id: "ruler_metric_v",
            unit: "mm",
            horizontal: false,
            a0: 52.0,
            b: 199.0,
            pitch: 1.0,
            n: 140,
            tiers: METRIC_TIERS_V,
            positive: false,
            label_every: 10,
        },
        Ruler {
            id: "ruler_inch_v",
            unit: "in/16",
            horizontal: false,
            a0: 52.0,
            b: 199.0,
            pitch: 25.4 / 16.0,
            n: 88,
            tiers: INCH_TIERS_V,
            positive: true,
            label_every: 0,
        },
    ]
}

pub fn ramp_level(x_mm: f64) -> u8 {
    (255.0 * ((x_mm - RAMP_X.0) / (RAMP_X.1 - RAMP_X.0)).clamp(0.0, 1.0)).round() as u8
}

pub fn text_for(pt: u32) -> String {
    format!("{pt}PT {TEXT_BODY}")
}

fn centre_mark_rects() -> Vec<(&'static str, diag::RectMm)> {
    let t = MARK_LINE_MM;
    let (cx, cy) = (CENTRE_X_MM, CENTRE_Y_MM);
    vec![
        ("centre_mark_left", (0.8, cy - t / 2.0, 2.6, cy + t / 2.0)),
        (
            "centre_mark_right",
            (203.6, cy - t / 2.0, 205.4, cy + t / 2.0),
        ),
        ("centre_mark_top", (cx - t / 2.0, 12.5, cx + t / 2.0, 13.9)),
        (
            "centre_mark_bottom",
            (cx - t / 2.0, 285.2, cx + t / 2.0, 287.4),
        ),
    ]
}

fn draw_reg(img: &mut RgbImage, dpi: u32, cx: f64, cy: f64) {
    let k = pxmm(dpi);
    let hw = (MARK_LINE_MM / 2.0).max(0.5 / k);
    let e = REG_ARM_MM + 0.4;
    for y in px(cy - e, dpi)..=px(cy + e, dpi) {
        for x in px(cx - e, dpi)..=px(cx + e, dpi) {
            let (dx, dy) = ((x as f64 + 0.5) / k - cx, (y as f64 + 0.5) / k - cy);
            let ring = (dx.hypot(dy) - REG_RING_MM).abs() <= hw;
            let cross = (dx.abs() <= hw && dy.abs() <= REG_ARM_MM)
                || (dy.abs() <= hw && dx.abs() <= REG_ARM_MM);
            if (ring || cross) && x < img.width() && y < img.height() {
                img.put_pixel(x, y, INK);
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Geometry: Sierpinski triangle, golden construction, circles, hexagon, right-angle markers
// ------------------------------------------------------------------------------------------------

/// Golden construction straight edges (units): vertical `(x, y0, y1)`, horizontal `(y, x0, x1)`.
pub const GOLD_VLINES: [(f64, f64, f64); 5] = [
    (0.0, 0.0, 13.0),
    (13.0, 0.0, 13.0),
    (15.0, 8.0, 10.0),
    (16.0, 8.0, 13.0),
    (21.0, 0.0, 13.0),
];
pub const GOLD_HLINES: [(f64, f64, f64); 5] = [
    (0.0, 0.0, 21.0),
    (8.0, 13.0, 21.0),
    (9.0, 15.0, 16.0),
    (10.0, 13.0, 16.0),
    (13.0, 0.0, 21.0),
];

pub fn sierp_height() -> f64 {
    SIERP_SIDE * 3f64.sqrt() / 2.0
}

/// Ink test of the Sierpinski triangle at an mm point (lattice rule `i & j == 0`, up triangles).
pub fn sierp_ink(x: f64, y: f64) -> bool {
    let hgt = sierp_height();
    let t = (SIERP_TOP + hgt - y) / hgt;
    let s = (x - SIERP_X0) / SIERP_SIDE - 0.5 * t;
    if s < 0.0 || t < 0.0 || s + t >= 1.0 {
        return false;
    }
    let n = (1u32 << SIERP_DEPTH) as f64;
    let (fs, ft) = (s * n, t * n);
    let (i, j) = (fs.floor() as u32, ft.floor() as u32);
    i & j == 0 && (fs - i as f64) + (ft - j as f64) < 1.0
}

/// Centre (mm) of every level-`level` hole (holes have side `SIERP_SIDE / 2^level`).
/// Level l (1..=depth) holes have side `SIERP_SIDE / 2^l`.
pub fn sierp_hole_centres(level: u32) -> Vec<(f64, f64)> {
    let m = level - 1;
    let n = 1u32 << m;
    let s = SIERP_SIDE / n as f64;
    let hgt = sierp_height();
    let (base_y, e1, e2) = (SIERP_TOP + hgt, (1.0, 0.0), (0.5, -(3f64.sqrt() / 2.0)));
    let mut out = vec![];
    for j in 0..n {
        for i in 0..n {
            if i & j == 0 && i + j < n {
                let (a, b) = (i as f64 * s, j as f64 * s);
                let (cx, cy) = (SIERP_X0 + a * e1.0 + b * e2.0, base_y + a * e1.1 + b * e2.1);
                out.push((cx + s / 3.0 * (e1.0 + e2.0), cy + s / 3.0 * (e1.1 + e2.1)));
            }
        }
    }
    out
}

fn line_half_width(dpi: u32) -> f64 {
    (MARK_LINE_MM / 2.0).max(0.5 / pxmm(dpi))
}

fn dist_to_segment(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
    };
    (p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy)
}

/// Stroke a polyline segment (mm) into the image.
fn stroke(img: &mut RgbImage, dpi: u32, a: (f64, f64), b: (f64, f64)) {
    let k = pxmm(dpi);
    let hw = line_half_width(dpi);
    let (w, h) = img.dimensions();
    let (x0, x1) = (a.0.min(b.0) - hw - 0.3, a.0.max(b.0) + hw + 0.3);
    let (y0, y1) = (a.1.min(b.1) - hw - 0.3, a.1.max(b.1) + hw + 0.3);
    for y in px(y0, dpi)..=px(y1, dpi).min(h - 1) {
        for x in px(x0, dpi)..=px(x1, dpi).min(w - 1) {
            let p = ((x as f64 + 0.5) / k, (y as f64 + 0.5) / k);
            if dist_to_segment(p, a, b) <= hw {
                img.put_pixel(x, y, INK);
            }
        }
    }
}

/// Stroke a full circle or an arc (angles in degrees, image coordinates, `a0 <= a1`).
fn stroke_arc(img: &mut RgbImage, dpi: u32, c: (f64, f64), r: f64, a0: f64, a1: f64) {
    let k = pxmm(dpi);
    let hw = line_half_width(dpi);
    let (w, h) = img.dimensions();
    let e = r + hw + 0.3;
    for y in px(c.1 - e, dpi)..=px(c.1 + e, dpi).min(h - 1) {
        for x in px(c.0 - e, dpi)..=px(c.0 + e, dpi).min(w - 1) {
            let (dx, dy) = ((x as f64 + 0.5) / k - c.0, (y as f64 + 0.5) / k - c.1);
            if (dx.hypot(dy) - r).abs() > hw {
                continue;
            }
            let ang = dy.atan2(dx).to_degrees().rem_euclid(360.0);
            let eps = 1e-9;
            if (a1 - a0 >= 360.0) || (ang >= a0 - eps && ang <= a1 + eps) {
                img.put_pixel(x, y, INK);
            }
        }
    }
}

pub fn hexagon_vertices() -> [(f64, f64); 6] {
    let (cx, cy, r) = HEXAGON;
    let mut v = [(0.0, 0.0); 6];
    for (i, p) in v.iter_mut().enumerate() {
        let a = (60.0 * i as f64).to_radians();
        *p = (cx + r * a.cos(), cy + r * a.sin());
    }
    v
}

/// Marker vertex and arm unit vectors for `MARKERS[i]`.
pub fn marker_geometry(i: usize) -> ((f64, f64), (f64, f64), (f64, f64)) {
    let (x0, y0, x1, y1, ang) = MARKERS[i];
    let (sa, ca) = ang.to_radians().sin_cos();
    let (u1, u2) = ((ca, sa), (-sa, ca));
    let c = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let v = (
        c.0 - 0.5 * MARKER_ARM_MM * (u1.0 + u2.0),
        c.1 - 0.5 * MARKER_ARM_MM * (u1.1 + u2.1),
    );
    (v, u1, u2)
}

fn draw_geometry(img: &mut RgbImage, dpi: u32) {
    let k = pxmm(dpi);
    let (w, h) = img.dimensions();
    // Sierpinski: 3 x 3 supersampled coverage, ink 0 on paper 255.
    let hgt = sierp_height();
    for y in px(SIERP_TOP, dpi)..=px(SIERP_TOP + hgt, dpi).min(h - 1) {
        for x in px(SIERP_X0, dpi)..=px(SIERP_X0 + SIERP_SIDE, dpi).min(w - 1) {
            let mut hit = 0;
            for sy in 0..3 {
                for sx in 0..3 {
                    let (mx, my) = (
                        (x as f64 + (sx as f64 + 0.5) / 3.0) / k,
                        (y as f64 + (sy as f64 + 0.5) / 3.0) / k,
                    );
                    if sierp_ink(mx, my) {
                        hit += 1;
                    }
                }
            }
            if hit > 0 {
                img.put_pixel(x, y, gray(255.0 * (1.0 - hit as f64 / 9.0)));
            }
        }
    }
    // Golden construction.
    let (ox, oy) = GOLD_ORIGIN;
    let u = GOLD_UNIT_MM;
    let t = MARK_LINE_MM;
    for (xu, y0, y1) in GOLD_VLINES {
        let x = ox + xu * u;
        fill_mm(
            img,
            dpi,
            (
                x - t / 2.0,
                oy + y0 * u - t / 2.0,
                x + t / 2.0,
                oy + y1 * u + t / 2.0,
            ),
            INK,
        );
    }
    for (yu, x0, x1) in GOLD_HLINES {
        let y = oy + yu * u;
        fill_mm(
            img,
            dpi,
            (
                ox + x0 * u - t / 2.0,
                y - t / 2.0,
                ox + x1 * u + t / 2.0,
                y + t / 2.0,
            ),
            INK,
        );
    }
    for (cx, cy, r, a0, a1) in GOLD_ARCS {
        stroke_arc(img, dpi, (ox + cx * u, oy + cy * u), r * u, a0, a1);
    }
    // Concentric circles, circle in square with diagonals.
    for r in CIRCLE_RADII_MM {
        stroke_arc(img, dpi, CIRCLES, r, 0.0, 360.0);
    }
    let q = SQUARE_CIRCLE;
    for r in [
        (q.0, q.1, q.2, q.1 + t),
        (q.0, q.3 - t, q.2, q.3),
        (q.0, q.1, q.0 + t, q.3),
        (q.2 - t, q.1, q.2, q.3),
    ] {
        fill_mm(img, dpi, r, INK);
    }
    let qc = ((q.0 + q.2) / 2.0, (q.1 + q.3) / 2.0);
    stroke_arc(img, dpi, qc, (q.2 - q.0) / 2.0 - t / 2.0, 0.0, 360.0);
    stroke(img, dpi, (q.0, q.1), (q.2, q.3));
    stroke(img, dpi, (q.2, q.1), (q.0, q.3));
    // Hexagon.
    let hv = hexagon_vertices();
    for i in 0..6 {
        stroke(img, dpi, hv[i], hv[(i + 1) % 6]);
    }
    // Right-angle markers.
    for i in 0..MARKERS.len() {
        let (v, u1, u2) = marker_geometry(i);
        stroke(
            img,
            dpi,
            (v.0 + u1.0 * MARKER_ARM_MM, v.1 + u1.1 * MARKER_ARM_MM),
            v,
        );
        stroke(
            img,
            dpi,
            v,
            (v.0 + u2.0 * MARKER_ARM_MM, v.1 + u2.1 * MARKER_ARM_MM),
        );
    }
}

fn geometry_elements() -> Vec<El> {
    let mut v = vec![];
    let hgt = sierp_height();
    let base = SIERP_TOP + hgt;
    let mut sierp = El::new(
        "sierpinski",
        "sierpinski",
        "geometry",
        (SIERP_X0, SIERP_TOP, SIERP_X0 + SIERP_SIDE, base),
        2,
    );
    for centres in (1..=3).map(sierp_hole_centres) {
        for c in centres.iter().take(3) {
            sierp.sample(c.0, c.1, [255, 255, 255], 8);
        }
    }
    let levels: Vec<f64> = (1..=SIERP_DEPTH)
        .map(|l| SIERP_SIDE / (1u32 << l) as f64)
        .collect();
    v.push(sierp.extra(json!({
        "apex": "up", "x0_mm": SIERP_X0, "top_mm": SIERP_TOP, "base_y_mm": base, "side_mm": SIERP_SIDE, "height_mm": hgt,
        "depth": SIERP_DEPTH, "vertices_mm": [[SIERP_X0, base], [SIERP_X0 + SIERP_SIDE, base], [SIERP_X0 + SIERP_SIDE / 2.0, SIERP_TOP]],
        "rule": "P = O + s*SIDE*(1,0) + t*SIDE*(0.5,-sqrt(3)/2); n = 2^depth; ink iff i&j==0 and frac(s*n)+frac(t*n)<1 with i=floor(s*n), j=floor(t*n)",
        "hole_side_mm_by_level": levels, "hole_centres": "up-triangle centroids at depth level-1 with i&j==0; hole side = SIDE/2^level",
        "ink": 0, "paper": 255, "supersample": "3x3",
    })));
    let (ox, oy) = GOLD_ORIGIN;
    let u = GOLD_UNIT_MM;
    let squares: Vec<Value> = GOLD_SQUARES.iter().map(|&(s, x, y)| json!({"size_units": s, "size_mm": s * u, "rect_mm": [ox + x * u, oy + y * u, ox + (x + s) * u, oy + (y + s) * u]})).collect();
    let arcs: Vec<Value> = GOLD_ARCS.iter().map(|&(cx, cy, r, a0, a1)| json!({"centre_mm": [ox + cx * u, oy + cy * u], "radius_mm": r * u, "a0_deg": a0, "a1_deg": a1})).collect();
    let vl: Vec<Value> = GOLD_VLINES
        .iter()
        .map(|&(x, y0, y1)| json!({"x_mm": ox + x * u, "y_span_mm": [oy + y0 * u, oy + y1 * u]}))
        .collect();
    let hl: Vec<Value> = GOLD_HLINES
        .iter()
        .map(|&(y, x0, x1)| json!({"y_mm": oy + y * u, "x_span_mm": [ox + x0 * u, ox + x1 * u]}))
        .collect();
    let fib = [1.0f64, 1.0, 2.0, 3.0, 5.0, 8.0, 13.0];
    let ratios: Vec<f64> = (1..fib.len()).map(|i| fib[i] / fib[i - 1]).collect();
    v.push(El::new("golden", "golden_construction", "geometry", (ox - t_half(), oy - t_half(), ox + 21.0 * u + t_half(), oy + 13.0 * u + t_half()), 2).extra(json!({
        "unit_mm": u, "origin_mm": [ox, oy], "width_units": 21, "height_units": 13, "size_sequence": fib, "successive_ratios": ratios,
        "phi": (1.0 + 5f64.sqrt()) / 2.0, "width_over_height": 21.0 / 13.0, "squares": squares, "arcs": arcs,
        "vertical_lines": vl, "horizontal_lines": hl, "line_mm": MARK_LINE_MM,
        "spiral": "quarter circles through the square corners, drawn outward from the 1 unit squares to the 13 unit square",
    })));
    v.push(El::new("concentric_circles", "concentric_circles", "geometry", bbox(CIRCLES.0, CIRCLES.1, 8.2), 2).extra(json!({"centre_mm": [CIRCLES.0, CIRCLES.1], "radii_mm": CIRCLE_RADII_MM, "line_mm": MARK_LINE_MM})));
    let q = SQUARE_CIRCLE;
    v.push(El::new("circle_in_square", "circle_in_square", "geometry", q, 2).extra(json!({
        "square_mm": [q.0, q.1, q.2, q.3], "side_mm": q.2 - q.0, "centre_mm": [(q.0 + q.2) / 2.0, (q.1 + q.3) / 2.0], "radius_mm": (q.2 - q.0) / 2.0 - MARK_LINE_MM / 2.0,
        "diagonals_mm": [[[q.0, q.1], [q.2, q.3]], [[q.2, q.1], [q.0, q.3]]], "line_mm": MARK_LINE_MM,
    })));
    let hv: Vec<Value> = hexagon_vertices()
        .iter()
        .map(|p| json!([p.0, p.1]))
        .collect();
    v.push(El::new("hexagon", "regular_polygon", "geometry", bbox(HEXAGON.0, HEXAGON.1, HEXAGON.2 + 0.3), 2).extra(json!({
        "centre_mm": [HEXAGON.0, HEXAGON.1], "circumradius_mm": HEXAGON.2, "apothem_mm": HEXAGON.2 * 3f64.sqrt() / 2.0, "sides": 6,
        "vertices_mm": hv, "first_vertex_angle_deg": 0, "line_mm": MARK_LINE_MM,
    })));
    for (i, m) in MARKERS.iter().enumerate() {
        let (vx, u1, u2) = marker_geometry(i);
        v.push(El::new(&format!("marker_{i}"), "right_angle_marker", "geometry", (m.0, m.1, m.2, m.3), 2).extra(json!({
            "vertex_mm": [vx.0, vx.1], "arm_mm": MARKER_ARM_MM, "u1": [u1.0, u1.1], "u2": [u2.0, u2.1], "rotation_deg": m.4, "angle_between_arms_deg": 90.0, "line_mm": MARK_LINE_MM,
        })));
    }
    v
}

fn t_half() -> f64 {
    MARK_LINE_MM / 2.0
}

// ------------------------------------------------------------------------------------------------
// Layout description
// ------------------------------------------------------------------------------------------------

struct El {
    id: String,
    kind: &'static str,
    group: &'static str,
    rect: diag::RectMm,
    anchor: &'static str,
    version: u32,
    drawn: bool,
    pixel_exact: bool,
    extra: Value,
    samples: Vec<Value>,
}

impl El {
    fn new(
        id: &str,
        kind: &'static str,
        group: &'static str,
        rect: diag::RectMm,
        version: u32,
    ) -> Self {
        El {
            id: id.to_string(),
            kind,
            group,
            rect,
            anchor: "layout",
            version,
            drawn: true,
            pixel_exact: false,
            extra: json!({}),
            samples: vec![],
        }
    }
    fn extra(mut self, v: Value) -> Self {
        self.extra = v;
        self
    }
    fn sample(&mut self, x: f64, y: f64, rgb: [u8; 3], tol: u8) {
        self.samples
            .push(json!({"x_mm": x, "y_mm": y, "rgb": rgb, "tol": tol}));
    }
    fn json(&self) -> Value {
        json!({
            "id": self.id, "kind": self.kind, "group": self.group,
            "rect_mm": [self.rect.0, self.rect.1, self.rect.2, self.rect.3],
            "size_mm": [self.rect.2 - self.rect.0, self.rect.3 - self.rect.1],
            "anchor": self.anchor, "since_layout": self.version, "drawn": self.drawn,
            "pixel_exact": self.pixel_exact, "expected": self.extra, "samples": self.samples,
        })
    }
}

fn bbox(cx: f64, cy: f64, r: f64) -> diag::RectMm {
    (cx - r, cy - r, cx + r, cy + r)
}

/// Layout-v1 elements (unchanged content; listed so loops see the whole page).
fn elements_v1() -> Vec<El> {
    let mut v = vec![];
    let fid = [
        ("fiducial_tl", diag::FID_TL, 1),
        ("fiducial_tr", diag::FID_TR, 2),
        ("fiducial_bl", diag::FID_BL, 3),
        ("fiducial_br", diag::FID_BR, 4),
    ];
    for (id, o, bars) in fid {
        let s = diag::FID_UNIT_MM * diag::FID_UNITS as f64;
        v.push(
            El::new(id, "fiducial", "v1", (o.0, o.1, o.0 + s, o.1 + s), 1)
                .extra(json!({"teeth": bars, "spine": "left", "ink": [0,0,0]})),
        );
    }
    let b = diag::barker_run().len() as f64 * diag::BARKER_CHIP_MM;
    let m = diag::MSEQ_LEN as f64 * diag::MSEQ_CHIP_MM;
    let t = diag::STRIP_THICK_MM;
    let strips = [
        ("strip_h_barker", diag::HSTRIP_BARKER, b, t, "barker13x3"),
        ("strip_h_mseq", diag::HSTRIP_MSEQ, m, t, "mseq63"),
        ("strip_v_barker", diag::VSTRIP_BARKER, t, b, "barker13x3"),
        ("strip_v_mseq", diag::VSTRIP_MSEQ, t, m, "mseq63"),
    ];
    for (id, o, w, h, seq) in strips {
        v.push(
            El::new(
                id,
                "correlation_strip",
                "v1",
                (o.0, o.1, o.0 + w, o.1 + h),
                1,
            )
            .extra(json!({"sequence": seq})),
        );
    }
    let wb = diag::WEDGE_BAND;
    let mut wedge = El::new("wedge_grey16", "grey_wedge", "v1", wb, 1);
    let ix0 = wb.0 + diag::WEDGE_OUTLINE_MM;
    let ymid = (wb.1 + wb.3) / 2.0;
    for k in 0..diag::WEDGE_STEPS {
        let l = diag::wedge_level(k);
        wedge.sample(
            ix0 + diag::WEDGE_REF_MM + (k as f64 + 0.5) * diag::WEDGE_STEP_MM,
            ymid,
            [l, l, l],
            1,
        );
    }
    wedge.sample(ix0 + 1.0, ymid, [0, 0, 0], 1);
    wedge.sample(
        wb.2 - diag::WEDGE_OUTLINE_MM - 1.0,
        ymid,
        [255, 255, 255],
        1,
    );
    v.push(
        wedge.extra(json!({"steps": 16, "level": "17*k", "black_ref_mm": 13, "white_ref_mm": 13})),
    );
    let mut pat = El::new(
        "patches_rgbcmy",
        "colour_patches",
        "v1",
        (
            diag::PATCHES_X0,
            diag::PATCHES_Y.0,
            diag::PATCHES_X0 + 6.0 * diag::PATCH_W_MM,
            diag::PATCHES_Y.1,
        ),
        1,
    );
    for (i, c) in diag::PATCH_COLORS.iter().enumerate() {
        pat.sample(
            diag::PATCHES_X0 + (i as f64 + 0.5) * diag::PATCH_W_MM,
            (diag::PATCHES_Y.0 + diag::PATCHES_Y.1) / 2.0,
            *c,
            1,
        );
    }
    v.push(pat.extra(json!({"order": ["R","G","B","C","M","Y"]})));
    let mut comb = El::new(
        "bit_comb",
        "pixel_exact",
        "v1",
        (
            diag::COMB_X0_MM,
            diag::COMB_Y_MM,
            diag::COMB_X0_MM + diag::COMB_WIDTH_MM,
            diag::COMB_Y_MM + 8.0 * 25.4 / 100.0,
        ),
        1,
    );
    comb.pixel_exact = true;
    v.push(comb);
    let mut bars = El::new(
        "golomb_bars",
        "pixel_exact",
        "v1",
        (
            diag::BARS_X0_MM,
            diag::BARS_Y_MM.0,
            diag::BARS_X0_MM + 90.0,
            diag::BARS_Y_MM.1,
        ),
        1,
    );
    bars.pixel_exact = true;
    v.push(bars);
    v.push(El::new(
        "raw_bits",
        "raw_bit_strip",
        "v1",
        (
            diag::RAW_X0_MM,
            diag::RAW_Y_MM.0,
            diag::RAW_X0_MM + diag::RAW_BITS as f64 * diag::RAW_CHIP_MM,
            diag::RAW_Y_MM.1,
        ),
        1,
    ));
    let pn = diag::POS_N as f64 * diag::POS_CELL_MM;
    v.push(El::new(
        "position_array",
        "position_array",
        "v1",
        (
            diag::POS_X0_MM,
            diag::POS_Y0_MM,
            diag::POS_X0_MM + pn,
            diag::POS_Y0_MM + pn,
        ),
        1,
    ));
    v.push(El::new("zone_plate", "zone_plate", "v1", diag::ZONE_BOX, 1));
    v.push(
        El::new(
            "qr",
            "qr",
            "v1",
            (132.0, 212.0, diag::QR_ANCHOR_MM.0, diag::QR_ANCHOR_MM.1),
            1,
        )
        .extra(json!({"anchor_bottom_right_mm": [diag::QR_ANCHOR_MM.0, diag::QR_ANCHOR_MM.1]})),
    );
    v
}

fn edge_info(side: &str, pos: f64, span: (f64, f64)) -> Value {
    json!({"side": side, "axis": if side == "left" || side == "right" { "x" } else { "y" }, "pos_mm": pos, "span_mm": [span.0, span.1]})
}

fn elements_v2(dpi: u32) -> Vec<El> {
    let (w, h) = image_gen::job_page_pixels(dpi);
    let e = diag::effective_dpi(dpi);
    let (wm, hm) = (w as f64 / pxmm(e), h as f64 / pxmm(e));
    let mut v = vec![];

    // Print marks.
    let mut border = El::new("page_border", "frame", "print_marks", (0.0, 0.0, wm, hm), 1).extra(json!({
        "meaning": "physical sheet edge: 1 px black hairline at the very edge of the raster",
        "style": "solid 1px", "rgb": [0, 0, 0], "inset_px": 0,
        "edges": [edge_info("left", 0.0, (60.0, hm - 60.0)), edge_info("right", wm, (60.0, hm - 60.0)),
                  edge_info("top", 0.0, (60.0, wm - 60.0)), edge_info("bottom", hm, (60.0, wm - 60.0))],
        "top_edge_note": "the top row lies under the tester banner",
    }));
    border.anchor = "raster";
    v.push(border);
    let mut ticks = El::new("corner_ticks", "marks", "print_marks", (0.5, 0.5, wm - 0.5, hm - 0.5), 2).extra(json!({
        "arm_mm": 8.0, "line_mm": MARK_LINE_MM, "inset_mm": 0.5, "note": "top pair lies under the tester banner",
    }));
    ticks.anchor = "raster";
    v.push(ticks);
    let p = PRINTABLE;
    let (l, r, t, b) = (
        p.0 + FRAME_LINE_MM / 2.0,
        p.2 - FRAME_LINE_MM / 2.0,
        p.1 + FRAME_LINE_MM / 2.0,
        p.3 - FRAME_LINE_MM / 2.0,
    );
    v.push(El::new("printable_frame", "frame", "print_marks", p, 2).extra(json!({
        "meaning": "printable area: frame at a known inset from the layout box (top edge sits below the banner)",
        "inset_mm": {"left": p.0, "right": 206.0 - p.2, "bottom": 288.0 - p.3, "top": p.1, "top_note": "measured from the layout top, just below the tester banner"},
        "line_mm": FRAME_LINE_MM, "style": "solid", "rgb": FRAME_RGB,
        "edges": [edge_info("left", l, (196.0, 262.0)), edge_info("right", r, (200.0, 260.0)),
                  edge_info("top", t, (30.0, 100.0)), edge_info("bottom", b, (30.0, 100.0))],
    })));
    for (id, sw, sh) in SHEETS {
        let fits = sw <= wm && sh <= hm;
        let mut s = El::new(id, "frame", "print_marks", (0.0, 0.0, sw, sh), 2).extra(json!({"style": "dashed 4 on 2 off", "fits_raster": fits, "note": "drawn only when it fits inside the raster"}));
        s.drawn = fits;
        v.push(s);
    }
    for (id, r) in [
        ("crop_top_left_v", (1.7, 12.6, 2.0, 18.6)),
        ("crop_top_right_v", (204.0, 12.6, 204.3, 18.6)),
        ("crop_bottom_left_v", (1.7, 279.6, 2.0, 285.6)),
        ("crop_bottom_right_v", (204.0, 279.6, 204.3, 285.6)),
        ("crop_bottom_left_h", (1.7, 286.3, 7.7, 286.6)),
        ("crop_bottom_right_h", (198.3, 286.3, 204.3, 286.6)),
    ] {
        v.push(
            El::new(id, "marks", "print_marks", r, 2).extra(
                json!({"line_mm": MARK_LINE_MM, "note": "top pair is vertical only (banner)"}),
            ),
        );
    }
    for (id, r) in centre_mark_rects() {
        v.push(
            El::new(id, "marks", "print_marks", r, 2)
                .extra(json!({"centre_mm": [CENTRE_X_MM, CENTRE_Y_MM]})),
        );
    }
    for (id, cx, cy) in REG {
        v.push(El::new(id, "registration_cross", "print_marks", bbox(cx, cy, REG_ARM_MM + 0.4), 2).extra(json!({"centre_mm": [cx, cy], "ring_radius_mm": REG_RING_MM, "arm_mm": REG_ARM_MM, "line_mm": MARK_LINE_MM})));
    }
    for r in rulers() {
        let ticks: Vec<f64> = (0..=r.n).map(|i| r.pos(i)).collect();
        let lens: Vec<f64> = (0..=r.n).map(|i| r.tick_len(i)).collect();
        let labels: Vec<Value> = (0..=r.n)
            .filter_map(|i| r.label(i))
            .map(|(t, x, y)| json!({"text": t, "x_mm": x, "y_mm": y}))
            .collect();
        let divisors: Vec<u32> = r.tiers.iter().map(|t| t.0).collect();
        let points = if r.positive {
            "+perp (down/right)"
        } else {
            "-perp (up/left)"
        };
        v.push(El::new(r.id, "ruler", "print_marks", r.rect(), 2).extra(json!({
            "unit": r.unit, "axis": if r.horizontal { "x" } else { "y" }, "origin_mm": r.a0, "baseline_mm": r.b,
            "pitch_mm": r.pitch, "count": r.n, "length_mm": r.n as f64 * r.pitch, "ticks_mm": ticks, "tick_len_mm": lens,
            "tier_divisors": divisors, "ticks_point": points,
            "label_every_ticks": r.label_every, "labels": labels, "tick_width_px": tick_px(e),
            "render": "tick i is the tick_width_px-wide pixel column starting at round(pos_mm * dpi / 25.4); exact at 254 dpi, otherwise rounded per tick (error <= 0.5 px, never accumulates)",
        })));
    }
    v.push(El::new("measure_bar", "measure_bar", "print_marks", MEASURE_BAR, 2).extra(json!({
        "length_mm": MEASURE_LEN_MM, "x0_mm": MEASURE_BAR.0, "x1_mm": MEASURE_BAR.2, "thickness_mm": MEASURE_BAR.3 - MEASURE_BAR.1,
        "end_ticks_mm": [MEASURE_BAR.0, MEASURE_BAR.2], "text": MEASURE_TEXT.iter().map(|t| t.2).collect::<Vec<_>>(),
        "note": "a physical ruler laid on the bar reads 190 mm only if the print is at 100 percent; inch baselines: ruler_inch_h 5 in, ruler_inch_v 5.5 in",
    })));
    for (i, (x, y, t)) in MEASURE_TEXT.iter().enumerate() {
        let (tw, th, _) = text_extent(t, RULER_LABEL_PT);
        v.push(
            El::new(
                &format!("measure_text_{i}"),
                "text",
                "text",
                (*x, *y, x + tw, y + th),
                2,
            )
            .extra(json!({"pt": RULER_LABEL_PT, "text": t})),
        );
    }

    // Resolution / alignment.
    let (cx, cy, r) = WHEEL;
    let mut wheel = El::new("hue_wheel", "hue_wheel", "colour", bbox(cx, cy, r), 2);
    for i in 0..36 {
        let hdeg = i as f64 * 10.0;
        let a = hdeg.to_radians();
        let rs = 0.8 * r;
        wheel.sample(
            cx + rs * a.cos(),
            cy + rs * a.sin(),
            hsv_to_rgb(hdeg, 0.8, 1.0),
            12,
        );
    }
    v.push(wheel.extra(json!({
        "centre_mm": [cx, cy], "radius_mm": r, "hue_deg": "atan2(dy, dx) in page coordinates (y down): 0 = +x (red), increasing clockwise on screen",
        "saturation": "radius / R", "value": 1.0, "sample_ring": 0.8, "sample_hues_deg": (0..36).map(|i| i as f64 * 10.0).collect::<Vec<_>>(),
    })));
    let (cx, cy, r) = STAR;
    v.push(
        El::new(
            "siemens_star",
            "siemens_star",
            "resolution",
            bbox(cx, cy, r),
            2,
        )
        .extra(json!({
            "centre_mm": [cx, cy], "radius_mm": r, "cycles": STAR_CYCLES, "ink": 0, "paper": 255,
            "lp_per_mm_at_radius": "cycles / (2 * pi * r)",
        })),
    );
    v.extend(geometry_elements());
    for (id, cx, cy, ang) in SLANT {
        let (sa, ca) = ang.to_radians().sin_cos();
        let ext = SLANT_HALF_MM * (ca.abs() + sa.abs()) + 0.2;
        v.push(El::new(id, "slanted_edge", "resolution", bbox(cx, cy, ext), 2).extra(json!({
            "centre_mm": [cx, cy], "half_side_mm": SLANT_HALF_MM, "angle_deg": ang, "dark": SLANT_DARK, "light": 255, "supersample": "4x4",
        })));
    }
    let n = RES_PERIODS_MM.len() as f64;
    for (id, ys, vert) in [
        ("res_lines_v", RES_V_Y, true),
        ("res_lines_h", RES_H_Y, false),
    ] {
        v.push(El::new(id, "line_pairs", "resolution", (RES_X0_MM, ys.0, RES_X0_MM + n * RES_PATCH_MM, ys.1), 2).extra(json!({
            "orientation": if vert { "vertical lines (period along x)" } else { "horizontal lines (period along y)" },
            "periods_mm": RES_PERIODS_MM, "patch_mm": RES_PATCH_MM, "ink": 0, "paper": 255, "duty": 0.5, "area_coverage": "exact 1-D box",
            "fundamental_amplitude": "(4/pi)*127.5 around 127.5 (0/255 square wave)",
        })));
    }
    let (qx, qy, qs) = PX_LINES;
    let mut pxl = El::new(
        "res_lines_px",
        "pixel_exact",
        "resolution",
        (qx, qy, qx + 4.0 * qs, qy + qs),
        2,
    );
    pxl.pixel_exact = true;
    v.push(pxl.extra(json!({"patches": ["1px vertical", "1px horizontal", "2px vertical", "2px horizontal"], "patch_mm": qs, "note": "pixel-defined: sizes in mm are nominal, lost by any resampling"})));
    v.push(
        El::new(
            "fine_grid",
            "line_grid",
            "resolution",
            (GRID.0, GRID.1, GRID.2 + 0.4, GRID.3 + 0.4),
            2,
        )
        .extra(json!({"pitch_mm": 1.0, "heavy_every": 5, "line_mm": 0.15, "heavy_mm": 0.3})),
    );

    // Colour.
    for (i, id) in RAMP_IDS.iter().enumerate() {
        let y0 = RAMP_Y0 + i as f64 * RAMP_PITCH;
        let mut el = El::new(
            id,
            "ramp",
            "colour",
            (RAMP_X.0, y0, RAMP_X.1, y0 + RAMP_H),
            2,
        );
        for s in 0..33 {
            let x = RAMP_X.0 + (s as f64 + 0.5) * (RAMP_X.1 - RAMP_X.0) / 33.0;
            let l = ramp_level(x);
            let c = match i {
                0 => [l, 0, 0],
                1 => [0, l, 0],
                2 => [0, 0, l],
                _ => [l, l, l],
            };
            el.sample(x, y0 + RAMP_H / 2.0, c, 2);
        }
        let chan = ["R", "G", "B", "neutral"][i];
        v.push(el.extra(json!({"channel": chan, "level": "round(255 * (x_mm - x0) / width)", "x0_mm": RAMP_X.0, "width_mm": RAMP_X.1 - RAMP_X.0})));
    }
    let ymid = (STEPS_Y.0 + STEPS_Y.1) / 2.0;
    for (id, xr, levels) in [
        ("near_black", NEAR_BLACK_X, &NEAR_BLACK[..]),
        ("near_white", NEAR_WHITE_X, &NEAR_WHITE[..]),
    ] {
        let mut el = El::new(
            id,
            "step_strip",
            "colour",
            (xr.0, STEPS_Y.0, xr.1, STEPS_Y.1),
            2,
        );
        for (i, &l) in levels.iter().enumerate() {
            let r = strip_patch_rect(xr, levels.len(), i);
            el.sample((r.0 + r.2) / 2.0, ymid, [l, l, l], 1);
        }
        v.push(el.extra(json!({"levels": levels, "outline": 0, "outline_mm": STEP_OUTLINE_MM})));
    }
    for (id, xr, l) in [
        ("clip_black", CLIP_BLACK_X, 0u8),
        ("clip_white", CLIP_WHITE_X, 255u8),
    ] {
        let mut el = El::new(
            id,
            "clip_patch",
            "colour",
            (xr.0, STEPS_Y.0, xr.1, STEPS_Y.1),
            2,
        );
        let r = strip_patch_rect(xr, 1, 0);
        el.sample((r.0 + r.2) / 2.0, ymid, [l, l, l], 1);
        v.push(el.extra(json!({"level": l, "outline": 0})));
    }
    let hy = (HALF_Y.0 + HALF_Y.1) / 2.0;
    let mut half = El::new(
        "half_primaries",
        "colour_patches",
        "colour",
        (HALF_X0, HALF_Y.0, HALF_X0 + 6.0 * HALF_W, HALF_Y.1),
        2,
    );
    for i in 0..6 {
        let c = diag::PATCH_COLORS[i].map(|v| if v == 255 { HALF_LEVEL } else { 0 });
        half.sample(HALF_X0 + (i as f64 + 0.5) * HALF_W, hy, c, 1);
    }
    v.push(half.extra(json!({"order": ["R", "G", "B", "C", "M", "Y"], "level": HALF_LEVEL})));
    let mut skin = El::new(
        "skin_tones",
        "colour_patches",
        "colour",
        (SKIN_X0, HALF_Y.0, SKIN_X0 + 3.0 * HALF_W, HALF_Y.1),
        2,
    );
    for (i, c) in SKIN.iter().enumerate() {
        skin.sample(SKIN_X0 + (i as f64 + 0.5) * HALF_W, hy, *c, 1);
    }
    v.push(skin.extra(json!({"names": ["light", "medium", "dark"]})));
    let mut greys = El::new(
        "mid_greys",
        "colour_patches",
        "colour",
        (GREYS_X0, HALF_Y.0, GREYS_X0 + 2.0 * GREY_W, HALF_Y.1),
        2,
    );
    for (i, l) in GREYS.iter().enumerate() {
        greys.sample(GREYS_X0 + (i as f64 + 0.5) * GREY_W, hy, [*l, *l, *l], 1);
    }
    v.push(greys.extra(json!({"levels": GREYS, "note": "118 = 18 % reflectance grey in sRGB"})));
    for (pt, y) in TEXT_LINES {
        let text = text_for(pt);
        let (tw, th, ink) = text_extent(&text, pt);
        v.push(El::new(&format!("text_{pt}pt"), "text", "text", (TEXT_X, y, TEXT_X + tw, y + th), 2).extra(json!({
            "pt": pt, "text": text, "cap_height_mm": th, "glyph_cell_mm": text_unit_mm(pt), "ink_fraction": ink,
            "font": "built-in 5x7 uppercase, cap height 0.72 em",
        })));
    }
    for (i, l) in LABELS.iter().enumerate() {
        let (tw, th, _) = text_extent(l, LABEL_PT);
        let y = LABEL_Y0 + i as f64 * LABEL_PITCH;
        v.push(
            El::new(
                &format!("label_{i}"),
                "text",
                "text",
                (TEXT_X, y, TEXT_X + tw, y + th),
                2,
            )
            .extra(json!({"pt": LABEL_PT, "text": l})),
        );
    }
    v
}

/// Ground-truth description of the page for `dpi` (requested) and layout `version` (1 or 2).
pub fn layout_json(dpi: u32, version: u32) -> Value {
    let (w, h) = image_gen::job_page_pixels(dpi);
    let e = diag::effective_dpi(dpi);
    let k = pxmm(e);
    let mut els = elements_v1();
    if version >= 2 {
        els.extend(elements_v2(dpi));
    }
    json!({
        "schema": SCHEMA,
        "layout_version": if version >= 2 { LAYOUT_VERSION } else { 1 },
        "colour_space": COLOUR_SPACE,
        "colour_note": "8-bit sRGB code values; no ICC conversion happens in the generator",
        "box_mm": [diag::LAYOUT_W_MM, diag::LAYOUT_H_MM],
        "origin": "top-left of the layout box, x right, y down, millimetres",
        "dpi_requested": dpi, "dpi_effective": e,
        "raster_px": [w, h], "raster_mm": [w as f64 / k, h as f64 / k], "px_per_mm": k,
        "banner_reserve_mm": diag::BANNER_RESERVE_MM,
        "qr_payload_layout_field": if version >= 2 { json!("lay=2") } else { Value::Null },
        "elements": els.iter().map(El::json).collect::<Vec<_>>(),
    })
}

/// Rectangles (mm, layout coordinates) where layout v2 may differ from v1: every v2 element.
pub fn v2_regions(dpi: u32) -> Vec<diag::RectMm> {
    elements_v2(dpi)
        .iter()
        .filter(|e| e.drawn && e.id != "page_border")
        .map(|e| e.rect)
        .collect()
}

/// Render the diag page for a layout version (banner and QR included) as RGB.
pub fn render(dpi: u32, page: u32, version: u32) -> anyhow::Result<RgbImage> {
    let pat = if version >= 2 {
        image_gen::PatternId::Diag
    } else {
        image_gen::PatternId::DiagV1
    };
    Ok(image_gen::render_pattern_rgb(pat, dpi, page, None)?.0)
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// PNG bytes with the colour space declared: an `sRGB` chunk (perceptual) and `gAMA` 45455.
pub fn png_bytes(img: &RgbImage) -> anyhow::Result<Vec<u8>> {
    use image::ImageEncoder;
    let mut raw = Vec::new();
    image::codecs::png::PngEncoder::new(&mut raw).write_image(
        img.as_raw(),
        img.width(),
        img.height(),
        image::ExtendedColorType::Rgb8,
    )?;
    // IHDR is the first chunk: 8 byte signature + 4 len + 4 type + 13 data + 4 crc = 33.
    let mut out = raw[..33].to_vec();
    for (kind, data) in [
        (*b"gAMA", 45455u32.to_be_bytes().to_vec()),
        (*b"sRGB", vec![0u8]),
    ] {
        out.extend((data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend(&data);
        out.extend(&body);
        out.extend(crc32(&body).to_be_bytes());
    }
    out.extend(&raw[33..]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image_gen::{luma8, PatternId};

    fn fnv(b: &[u8]) -> u64 {
        b.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &c| {
            (h ^ c as u64).wrapping_mul(0x0000_0100_0000_01b3)
        })
    }

    #[test]
    fn deterministic_and_png_declares_srgb() {
        let a = render(100, 0, 2).unwrap();
        assert_eq!(fnv(a.as_raw()), fnv(render(100, 0, 2).unwrap().as_raw()));
        let png = png_bytes(&a).unwrap();
        assert!(png.windows(4).any(|w| w == b"sRGB"));
        let back = image::load_from_memory(&png).unwrap().to_rgb8();
        assert_eq!(back.as_raw(), a.as_raw());
    }

    #[test]
    fn v2_changes_only_declared_regions_and_never_the_v1_content() {
        for dpi in [100u32, 150, 200] {
            let v1 = render(dpi, 3, 1).unwrap();
            let v2 = render(dpi, 3, 2).unwrap();
            assert_eq!(v1.dimensions(), v2.dimensions());
            let e = diag::effective_dpi(dpi);
            let k = pxmm(e);
            let regions = v2_regions(dpi);
            let (w, h) = v1.dimensions();
            let (wm, hm) = (w as f64 / k, h as f64 / k);
            let mut changed = 0;
            for y in 0..h {
                for x in 0..w {
                    if v1.get_pixel(x, y) == v2.get_pixel(x, y) {
                        continue;
                    }
                    changed += 1;
                    let (mx, my) = ((x as f64 + 0.5) / k, (y as f64 + 0.5) / k);
                    let inside = regions.iter().any(|r| {
                        mx >= r.0 - 0.3 && mx <= r.2 + 0.5 && my >= r.1 - 0.3 && my <= r.3 + 0.5
                    });
                    // Raster corner ticks follow the raster edges.
                    let tick = (mx < 9.5 || mx > wm - 9.5) && (my < 9.5 || my > hm - 9.5);
                    assert!(inside || tick, "dpi {dpi}: v2 altered ({x},{y}) = ({mx:.2},{my:.2}) mm outside every v2 element");
                }
            }
            assert!(changed > 1000, "v2 drew almost nothing ({changed})");
        }
    }

    #[test]
    fn elements_do_not_overlap_the_reserved_v1_regions() {
        let reserved: Vec<(&str, diag::RectMm)> = elements_v1()
            .into_iter()
            .filter(|e| e.id != "qr")
            .map(|e| (Box::leak(e.id.into_boxed_str()) as &str, e.rect))
            .collect();
        for e in elements_v2(100) {
            if !e.drawn || e.kind == "frame" || e.id == "corner_ticks" {
                continue;
            }
            for (rid, r) in &reserved {
                let hit = e.rect.0 < r.2 && e.rect.2 > r.0 && e.rect.1 < r.3 && e.rect.3 > r.1;
                assert!(!hit, "{} overlaps v1 element {rid}", e.id);
            }
            // QR reserve box and the v1 band the existing analyzer profiles.
            assert!(
                !(e.rect.0 < 184.0 && e.rect.2 > 128.0 && e.rect.1 < 264.0 && e.rect.3 > 208.0),
                "{} in QR reserve",
                e.id
            );
            assert!(
                e.rect.2 <= 206.0 && e.rect.3 <= 288.0,
                "{} outside the layout box",
                e.id
            );
        }
    }

    #[test]
    fn frame_lines_stay_clear_of_v1_elements() {
        let p = PRINTABLE;
        let f = FRAME_LINE_MM;
        let lines = [
            (p.0, p.1, p.0 + f, p.3),
            (p.2 - f, p.1, p.2, p.3),
            (p.0, p.1, p.2, p.1 + f),
            (p.0, p.3 - f, p.2, p.3),
        ];
        for el in elements_v1().iter().filter(|e| e.id != "qr") {
            for l in lines {
                let hit = el.rect.0 < l.2 && el.rect.2 > l.0 && el.rect.1 < l.3 && el.rect.3 > l.1;
                assert!(!hit, "frame line {l:?} touches {}", el.id);
            }
        }
    }

    #[test]
    fn layout_samples_match_rendered_pixels() {
        for dpi in [100u32, 150, 200] {
            let img = render(dpi, 0, 2).unwrap();
            let lay = layout_json(dpi, 2);
            let k = lay["px_per_mm"].as_f64().unwrap();
            assert_eq!(lay["raster_px"], json!([img.width(), img.height()]));
            let mut n = 0;
            for el in lay["elements"].as_array().unwrap() {
                for s in el["samples"].as_array().unwrap() {
                    let (x, y) = (s["x_mm"].as_f64().unwrap(), s["y_mm"].as_f64().unwrap());
                    let want: Vec<i64> = s["rgb"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.as_i64().unwrap())
                        .collect();
                    let tol = s["tol"].as_i64().unwrap();
                    let p = img.get_pixel((x * k) as u32, (y * k) as u32);
                    for c in 0..3 {
                        assert!(
                            (p.0[c] as i64 - want[c]).abs() <= tol,
                            "dpi {dpi} {} at ({x:.2},{y:.2}): got {:?}, want {want:?} +-{tol}",
                            el["id"],
                            p.0
                        );
                    }
                    n += 1;
                }
            }
            assert!(n > 200, "only {n} samples");
        }
    }

    #[test]
    fn frames_marks_and_text_render_where_the_layout_says() {
        let dpi = 200;
        let img = render(dpi, 0, 2).unwrap();
        let k = pxmm(dpi);
        let at = |x: f64, y: f64| *img.get_pixel((x * k) as u32, (y * k) as u32);
        // Printable frame: blue on all four edges, at the centre line of each.
        for (x, y) in [(3.2, 120.0), (202.8, 120.0), (100.0, 12.7), (100.0, 284.8)] {
            assert_eq!(at(x, y).0, FRAME_RGB, "frame at ({x},{y})");
        }
        // Page border: black hairline at the very edge of the raster (bottom/left/right).
        assert!(luma8(img.get_pixel(0, 800)) < 10);
        assert!(luma8(img.get_pixel(img.width() - 1, 800)) < 10);
        assert!(luma8(img.get_pixel(800, img.height() - 1)) < 10);
        // Registration cross: ring and arm ink, centre paper-free arm crossing.
        assert!(luma8(&at(27.0 + REG_RING_MM, 278.0)) < 60);
        assert!(luma8(&at(27.0, 278.0)) < 60);
        assert!(luma8(&at(27.0 + 1.5, 278.0 + 1.5)) > 200);
        // Measure bar: ink between its ends, paper just outside; end ticks above it.
        assert!(luma8(&at(100.0, 266.8)) < 40);
        assert!(luma8(&at(4.0, 266.8)) > 200 && luma8(&at(198.0, 266.8)) > 200);
        assert!(luma8(&at(6.0, 265.5)) < 40 && luma8(&at(196.0, 265.5)) < 40);
        // Ruler ticks: a long mm tick is ink at its position, paper half a pitch away.
        let r = &rulers()[0];
        assert!(luma8(&at(r.pos(10) + 0.1, r.b - 3.0)) < 40);
        assert!(luma8(&at(r.pos(10) + 0.5, r.b - 3.0)) > 200);
        // Layout v2 differs from v1 in the QR payload only by the layout field, not in its box size.
        // Text 12 pt has real ink; its measured fraction is in the neighbourhood of the analytic one.
        let (tw, th, ink) = text_extent(&text_for(12), 12);
        let (x0, y0, x1, y1) = (
            px(TEXT_X, dpi),
            px(232.0, dpi),
            px(TEXT_X + tw, dpi),
            px(232.0 + th, dpi),
        );
        let mut dark = 0;
        for y in y0..y1 {
            for x in x0..x1 {
                if luma8(img.get_pixel(x, y)) < 128 {
                    dark += 1;
                }
            }
        }
        let frac = dark as f64 / ((x1 - x0) * (y1 - y0)) as f64;
        assert!((frac - ink).abs() < 0.08, "ink {frac} vs {ink}");
    }

    #[test]
    fn layout_json_is_versioned_and_v1_has_no_v2_elements() {
        let l2 = layout_json(100, 2);
        let l1 = layout_json(100, 1);
        assert_eq!(l2["layout_version"], 2);
        assert_eq!(l1["layout_version"], 1);
        assert_eq!(l2["colour_space"], "sRGB");
        let ids = |l: &Value| {
            l["elements"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| e["id"].as_str().unwrap().to_string())
                .collect::<Vec<_>>()
        };
        let (i1, i2) = (ids(&l1), ids(&l2));
        assert!(i2.len() > i1.len() + 40);
        assert!(i1.iter().all(|i| i2.contains(i)));
        for must in [
            "page_border",
            "printable_frame",
            "sheet_a4",
            "sheet_letter",
            "hue_wheel",
            "ramp_n",
            "siemens_star",
            "text_6pt",
            "ruler_metric_h",
            "ruler_inch_v",
            "measure_bar",
        ] {
            assert!(i2.contains(&must.to_string()), "{must}");
        }
        let mut seen = std::collections::HashSet::new();
        assert!(i2.iter().all(|i| seen.insert(i.clone())), "duplicate ids");
        // Not drawn when they do not fit.
        let a4 = l2["elements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["id"] == "sheet_a4")
            .unwrap();
        assert_eq!(a4["drawn"], false);
        // Same bytes for the same params.
        assert_eq!(l2.to_string(), layout_json(100, 2).to_string());
        let _ = PatternId::Diag;
    }

    /// QR carries the layout version: it differs from v1 only inside the QR box.
    #[test]
    fn qr_is_the_only_pixel_difference_outside_v2_regions_and_carries_lay() {
        let v1 = render(100, 0, 1).unwrap();
        let v2 = render(100, 0, 2).unwrap();
        let k = pxmm(100);
        let mut qr_diff = 0;
        for (x, y, p) in v1.enumerate_pixels() {
            if p != v2.get_pixel(x, y) {
                let (mx, my) = ((x as f64 + 0.5) / k, (y as f64 + 0.5) / k);
                if (128.0..=186.0).contains(&mx) && (206.0..=266.0).contains(&my) {
                    qr_diff += 1;
                }
            }
        }
        assert!(qr_diff > 0, "QR payload should differ (lay=2)");
    }

    /// `FAKESCAN_DIAG_PNG=<path> cargo test write_doc_png` writes the 100 dpi layout-v2 page.
    #[test]
    fn write_doc_png() {
        if let Ok(path) = std::env::var("FAKESCAN_DIAG_PNG") {
            std::fs::write(path, png_bytes(&render(100, 0, 2).unwrap()).unwrap()).unwrap();
        }
    }

    #[test]
    fn hue_helpers_are_consistent() {
        assert_eq!(hsv_to_rgb(0.0, 1.0, 1.0), [255, 0, 0]);
        assert_eq!(hsv_to_rgb(120.0, 1.0, 1.0), [0, 255, 0]);
        assert_eq!(hsv_to_rgb(240.0, 1.0, 1.0), [0, 0, 255]);
        assert_eq!(hsv_to_rgb(60.0, 0.0, 1.0), [255, 255, 255]);
        assert!((wheel_hue_deg(0.0, 1.0) - 90.0).abs() < 1e-9);
        assert_eq!(ramp_level(26.0), 0);
        assert_eq!(ramp_level(126.0), 255);
    }
}
