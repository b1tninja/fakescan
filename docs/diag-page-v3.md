# Diag page, layout v3 (the default `diag`)

![layout v3, A4 at 100 dpi](diag-page-v3.png)

Layout v3 is a **parametric generator**, not a picture. No page is stored anywhere: the raster, the layout JSON, the
vector primitives and the QR payload are all produced by one deterministic function of the page parameters
(`fakescan::diag_v3`, Python `fakescan.diag`). The PNGs in this directory are rendered examples only; no test reads them.

Why a new layout: v1 / v2 are a fixed 206 x 288 mm box, crowded, and written for one device. v3 keeps what v2 proved
(frame, registration crosses, rulers, a measure bar, resolution and tone targets) and adds what the
[printer test page survey](#survey-and-what-v3-covers) ranked highest, for any sheet size, so the same page serves a scanner (sheet
size and dpi of the device, 4 MP cap) and a printer (PDF at the exact physical size, see "Using it from fakeprint").

## Parameters (`PageSpec`)

| Parameter | Meaning |
|-----------|---------|
| `paper` | `A0`..`A6`, `B4`, `B5` (ISO), `JIS-B4`, `JIS-B5`, `LETTER`, `LEGAL`, `TABLOID`, `LEDGER`, `EXECUTIVE`, `STATEMENT`, `FOLIO`, or custom `210x297` (mm) |
| `orientation` | `portrait` (default) or `landscape` |
| `width_mm`, `height_mm` | custom sheet in mm (overrides `paper` for the size) |
| `width_px`, `height_px` | exact raster size (the scanner path uses this: the device's pixels) |
| `dpi` | pixels per inch; pixel size = `round(mm * dpi / 25.4)` |
| `margin_mm` | printable-area margin (default 4); the blue frame sits at the margin |
| `bleed_mm` | > 0 adds flush edge bars and records the bleed box (outside the raster, `drawn: false` by nature) |
| `page`, `copy`, `total` | 0-based page index, copy index, pages in the job (0 = unknown) |
| `side` | `front` / `back` (duplex face), omit for simplex |
| `job` | short job tag (A-Z, 0-9, `-`, `_`, 16 chars) |
| `colour` | `rgb` (default) or `gray` (luma replicated; layout samples are converted) |
| `banner_px` | height of a tester banner the caller stamps over the top (the scanner does): the frame and content start below it |

The same parameters give the same bytes. A QR in the page carries them, so a decoder can regenerate the expected page:

```
fakescan|v1|page=2|pat=diag|side=front|lay=3|paper=A4|px=816x1144|dpi=100|m=4|b=0|copy=1|of=4|bn=44|g=0|job=AB-1
```

`lay=3` marks the layout version (v1 pages carry no `lay`, v2 carries `lay=2`); `PageSpec::from_qr_payload` /
`fakescan.diag.diag_spec_from_qr` invert it. (The scanner does not add `job=` or `client=`: the default page must be
byte-stable across connections.)

## How it scales

The page is a stack of rows inside the printable frame (printable frame minus a 1.4 mm pad). Block boxes (colour bars,
patch grids, wheels, geometry) scale by one factor `s` (0.55 to 2.0, the largest that fits; A4 at 100 dpi gets about 1.5,
A5 about 1.0, A3 the maximum 2.0). Detail that is physical stays physical: line-pair periods, line weights, text
point sizes, tick pitch, ruler and measure-bar length (the longest multiple of 10 mm that fits). If a row does not fit
at the minimum factor, optional blocks are dropped first (grid, halftone, text-on-colour, pangram), then whole rows by
priority; the top / bottom bands and the QR row are mandatory. Everything dropped stays in the layout with
`drawn: false` and a `reason`; `scale` is recorded on every element. Leftover height is spread between rows.

Landscape sheets use the same rows at the wider width; custom sizes work the same way.

## Elements

| Group | Element ids | What it shows / reveals | Machine check |
|-------|-------------|-------------------------|---------------|
| Print marks | `page_border`, `corner_ticks`, `edge_tick_*`, `printable_frame` (blue, 0.4 mm), `outline_a6..a3` (dashed grey media outlines anchored top-left, drawn only when the medium is smaller than the sheet in both directions), `bleed_probes` (when `bleed_mm` > 0) | cropping, margins, centring, scaling to media | frame edge positions, clipped edges; outline samples |
| Fiducials | `reg_tl/tr/bl/br` (ring + cross), `marker_tl/tr/bl/br` (4x4-bit markers, own dictionary `fakescan-4x4-4`, not OpenCV ArUco ids; codes are not rotation symmetric), `qr` | affine fit (scale x/y, rotation, shear, offset), pose, rotation of a marker | cross centroids, marker rotation, QR payload |
| Orientation | `orient_tl/tr/bl/br`: asymmetric glyphs F, R, G, J per corner; the **back** side mirrors each glyph and rotates the assignment (J, G, R, F) | rotate 180, mirror, flip long / short edge, wrong duplex side | glyph bitmap in the layout; the analyzer tests 4 transforms |
| Page order | `page_number` (big digits of page+1), `order_bars` (page+1 filled bars of 16, the old `page_id` cue), `side_marker` (front inks the left half, back the right, plus label), `page_stamp` (page / total, copy, job) | multi-page order, duplex pairing, copy counting | digit decode, bar count, side read |
| Colour | `ebu_bars` (EBU 100/0/75/0), `smpte_castellation`, `colorchecker24` (24 sRGB values), `ramp_r/g/b/n` (black to primary), `ramp_c/m/y/k` (paper to the declared sRGB full tint of cyan (0,174,239), magenta (236,0,140), yellow (255,242,0), black (35,31,32)), `hue_wheel`, `rich_black` (10,10,10) and `k100` patches | channel order / swap, hue, saturation, chroma, tone curve | ΔE76 per patch, channel permutation, monotonic ramps |
| Tone | `near_black`, `near_white`, `clip` (0 / 255), `flat_10/25/50/75/90` (ink %), `halftone_ladder` (45 degree dot screens at 10/25/50/75/90 % over solid references) | clipping, banding, streaking, uniformity, mottle, dither / moire | flat-field stdev, row and column ranges, halftone mean |
| Resolution | `siemens_star`, `slant_edge_a/b` (+5 / -5 deg), `res_lines_v/h` (periods 2 .. 0.2 mm), `zone_plate` (up to 4 cycles/mm at the half-width), `fine_grid`, `line_ladder_pos/neg` (0.25 .. 2 pt, positive and reverse) | MTF cut-off, resampling kernel, aliasing, thin-line dropout | modulation per period, thinnest visible line |
| Geometry | `concentric_circles`, `circle_in_square`, `hexagon` | aspect ratio, shear | ellipse fit (analyzer reads the layout's centres / radii) |
| Scale | `measure_bar` (exactly N mm, end ticks), `ruler_metric` (mm ticks, numbered per cm), `ruler_inch` (1/16 in ticks, numbered per inch) | x and y scale at 100 % | tick regression, bar length |
| Text | `text_12pt/10pt/8pt/6pt`, `pangram`, `latin_sample` (accents composed on the 5x7 pixel font: A O U E N C A O ss), `reverse_text`, `text_on_colour` (6 pt on yellow, cyan, red, blue), `legend` | font rendering, small text, reverse text, text on colour | ink fraction vs the analytic value, contrast |

The built-in 5x7 font is uppercase Latin: no TrueType font is embedded, so there is no real Unicode sample (see
"What is still missing").

## Layout JSON (ground truth) and vector primitives

`fakescan.diag.diag_page_layout(dpi, paper=..., ...)` / `fakescan::diag_v3::build_layout(&spec).json()`:

* header: `layout_version` 3, `colour_space` `sRGB`, `spec` (the full parameters), `raster_px`, `px_per_mm`, `scale`, `frame_mm`,
  `content_mm`, `bleed_box_mm`, `banner_reserve_mm`, `qr_payload`, `notes`;
* `elements[]`: `id`, `kind`, `group`, `rect_mm`, `size_mm`, `since_layout` (3), `drawn`, `scale`, `reason` (when not drawn),
  `expected` (element-specific: palettes, periods, bitmaps, tick pitch, ...), `samples` (`x_mm`, `y_mm`, `rgb`, `tol`:
  the code value expected at that point; the Rust and Python tests render the page at several sizes and dpis and check
  every sample);
* `primitives[]`: the drawing in paint order, millimetres, page coordinates, each with `id` (the element) and `vector`.
  Vector ops: `rect`, `polygon`, `line` (optionally dashed), `circle`, `text` (5x7 pixel font; `pt`, mirror flag), `grating`
  (bar patch), `ticks` (ruler ticks with tiers). Raster-only ops (`vector: false`): `ramp` (gradient), `hue_wheel`,
  `siemens_star`, `zone_plate`, `halftone`, `qr` (carries the payload). `fakescan.diag.diag_page_primitives(layout)` returns them.

Pixel `i` covers `[i / k, (i + 1) / k)` mm, `k = dpi / 25.4`; edges are anti-aliased by exact area coverage, so hairlines
render grey at low dpi (samples on hairlines use a large tolerance). Colours are 8-bit sRGB code values; no ICC
conversion happens. PNGs carry `sRGB`, `gAMA` and a `pHYs` chunk with the physical size.

## API

Rust: `fakescan::diag_v3::{PageSpec, build_layout, rasterize, render, png_bytes, paper_size_mm}`
(`PageSpec::from_paper("A4", landscape, dpi)`, `from_mm`, `from_px`, `from_json`, `qr_payload`, `from_qr_payload`).

Python (needs the native module):

```python
from fakescan import diag
lay = diag.diag_page_layout(300, paper="LETTER", page=1, total=3, side="back", job="t1")   # dict
img = diag.render_diag_page(300, paper="LETTER", page=1, total=3, side="back", job="t1")   # PIL image
png = diag.render_diag_png(300, paper="A4", orientation="landscape")                       # bytes, sRGB + pHYs
prims = diag.diag_page_primitives(lay)                                                     # vector description
spec = diag.diag_spec_from_qr(lay["qr_payload"])                                           # regenerate from the QR
rep = diag.analyze_diag_page(scan, lay)    # v3 layouts dispatch to fakescan.diag3.analyze_v3
```

`version=2` / `version=1` select the fixed layouts (their analyzer is unchanged).

### Analyzer report (`fakescan.diag3.analyze_v3`)

Driven only by the layout. `geometry`: affine fit from the four registration crosses (`px_per_mm_x/y`, `dpi_x/y`,
`scale_*_vs_nominal`, `aspect_x_over_y`, `rotation_deg`, `skew_deg`, `origin_offset_mm`, residual). `elements[<id>]`:
`colorchecker24` (mean / max ΔE76, worst patch, `channel_order` such as `bgr`), ramps (max error, monotonic), flat fields
(mean, error, stdev, row / column range), markers (rotation), rulers (`px_per_mm`, `pitch_error_pct`), `measure_bar`
(length error mm / %), `printable_frame` (per-edge error, clipped edges), `res_lines_*` (modulation, last resolvable
period), `line_ladder_*` (thinnest visible pt), `halftone_ladder` (mean error), `zone_plate` (contrast by radius), text
(ink fraction), other sampled elements (RGB error, ΔE). `elements["orientation"]` (verdict `upright`, `rot180`, `flip_h`,
`flip_v` from the four corner glyphs), `elements["page_order"]` (page number read, bar count, side read). `summary` has the
headline figures. At identity the scale error is < 0.5 %, rotation < 0.05 deg, ΔE76 < 1; injected 0.97 x scale, 0.5 deg
rotation, BGR swap, 180 deg rotation and mirrors are recovered in `tests/test_diag_v3.py`.

## Using it from fakeprint (vector PDF)

`diag_page_layout(...)` plus `diag_page_primitives(...)` give everything needed to draw the page at the exact physical size
in a PDF: draw the `vector: true` primitives with a PDF writer (mm to points: x 72 / 25.4, y flipped), and crop the
`rect_mm` of each raster-only element out of `render_diag_page(dpi, ...)` at a high dpi (or draw a PDF shading for `ramp`
and a QR from `payload`). The same layout JSON is the analysis ground truth after the print-scan loop.

## Versions, pins, compatibility

| Pattern | Page | Pin |
|---------|------|-----|
| `diag` | layout v3 (default) | `diag_v3::tests::pinned_hashes` (A4 / Letter at 100 dpi, A3 at 50) |
| `diag_v2` | layout v2, `lay=2` | `diag_v2_is_byte_identical_and_still_selectable` (100 and 200 dpi) |
| `diag_v1` | layout v1, no `lay` | `diag_v1_is_byte_identical_to_the_pre_v2_page` |
| `page_id` | numbered bars (old default) | selectable; `--default-pattern page_id` |

* v3 is **not** v2 plus additions: the elements are placed by the generator, so the v1 fiducials, position array and
  correlation strips (which `pdfcraft_fakescan.pattern_analyzer` blind-decodes) are not on the page. Loops that depend on
  that analyzer must select `diag_v2`; see "Consumers" below.
* The multi-page order cue of `page_id` (numbered bars + a banner) lives on v3 as `order_bars`, `page_number`, `page_stamp`
  and the QR (`page=`); the JPEG COM `fakescan-page:N` marker is unchanged.

## Consumers (what must change)

* `pdfcraft-fakescan` (`print_scan_loop.py` line ~121, `scalingmatrix.py` ~143 / ~157, `scanloop.py` ~167 / ~413) and
  `pdfcraft-fakeprint` (`printloop.py` ~90) call `set_pattern("diag")` and read it with the blind `pattern_analyzer`
  (v1 layout). They must select **`diag_v2`** (the old `diag`, byte-identical) or be ported to a v3 reader.
* `pdfcraft-fakeprint/printloop.py` ~611 calls `fakescan.diag.diag_page_layout(dpi=...)`, whose default is now version 3:
  pass `version=2` there, or use v3 with `paper` / `margin_mm` and the v3 analyzer.
* `POST /v1/loopback` (docs in pdfcraft-fakescan) resets to the default pattern, now `diag`.
* Nothing in those packages decodes the numbered bars of `page_id`; page order in the harness comes from the JPEG COM
  marker and the QR.

## Survey and what v3 covers

Ranked findings and sources: `pdfcraft-notes/print/test-page-survey.md`. Implemented from its P1 / P2 list: ColorChecker 24,
EBU / SMPTE bars, orientation glyphs and duplex markers, 4x4-bit corner markers (own dictionary), zone plate, flat fields at
5 tones, line-weight ladder (positive and reverse), CMYK-ish separation ramps and a rich-black patch, halftone ladder,
text on colour, reverse text, accent sample, page / copy stamp, dashed media outlines, bleed probes. Earlier layouts already
had slanted edges, Siemens star, line pairs, hue wheel, grey ramps, near-black / near-white, rulers, measure bar.

## What is still missing

* Real Unicode text (no embedded font; only an accent set on the pixel font).
* OpenCV-compatible ArUco / AprilTag ids and a DataMatrix (the markers are a private 4x4 dictionary; the QR is the
  interoperable fiducial).
* USAF-1951 bars, an OECF / gamma grey ring, transparency swatches, a dot-gain / ink-limit set, ghosting / show-through
  and paper-skew tests (these need physical media or a different output model, not an sRGB raster).
* `total` is not passed by the scanner (the page stamp reads "PAGE n" there); set it through `PageSpec` for print jobs.
* Colour mode `gray` replicates luma; real 1-bit / CMYK output modes are applied downstream by the encoder.
