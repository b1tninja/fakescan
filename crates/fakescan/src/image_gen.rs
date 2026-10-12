//! Synthetic page bitmaps for RetrieveImage.

use anyhow::{bail, Result};
use image::{ImageBuffer, Rgb, RgbImage};

/// Letter size in 1/1000 inch (WSD InputMediaSize units).
pub const LETTER_W_THOU: u32 = 8500;
pub const LETTER_H_THOU: u32 = 11000;

pub fn page_pixels(dpi: u32) -> (u32, u32) {
    let w = (LETTER_W_THOU * dpi) / 1000;
    let h = (LETTER_H_THOU * dpi) / 1000;
    (w.max(1), h.max(1))
}

/// Encode a JPEG with a page number banner (RGB24).
pub fn jpeg_page(dpi: u32, page_index: u32) -> Result<Vec<u8>> {
    let (w, h) = page_pixels(dpi);
    // Cap for CI memory; still looks like a letter page at lower dpi.
    let (w, h) = if w * h > 4_000_000 {
        page_pixels(150)
    } else {
        (w, h)
    };

    let mut img: RgbImage = ImageBuffer::from_pixel(w, h, Rgb([245, 245, 240]));
    let ink = Rgb([20, 40, 80]);
    let accent = Rgb([180, 60, 40]);

    // Border
    for x in 0..w {
        img.put_pixel(x, 0, ink);
        img.put_pixel(x, h - 1, ink);
    }
    for y in 0..h {
        img.put_pixel(0, y, ink);
        img.put_pixel(w - 1, y, ink);
    }

    // Horizontal rules + page marker blocks
    let band = (h / 20).max(4);
    for y in (band * 2)..(band * 3) {
        for x in (w / 10)..(w * 9 / 10) {
            img.put_pixel(x, y, accent);
        }
    }
    let mark = 8 + (page_index % 7) * 12;
    for y in (h / 2).saturating_sub(mark)..(h / 2 + mark).min(h) {
        for x in (w / 2).saturating_sub(mark)..(w / 2 + mark).min(w) {
            img.put_pixel(x, y, ink);
        }
    }

    let mut out = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut out);
    img.write_to(&mut cursor, image::ImageFormat::Jpeg)
        .map_err(|e| anyhow::anyhow!("jpeg encode: {e}"))?;
    if out.is_empty() {
        bail!("empty jpeg");
    }
    Ok(out)
}
