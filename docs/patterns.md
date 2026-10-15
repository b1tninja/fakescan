# Synthetic image patterns

RetrieveImage returns the job’s Format ([formats.md](formats.md)). For `jfif`, the blob also carries COM comment `fakescan-page:N` (0-based page index). The **pixel layout** is chosen by the control-plane pattern id (default `page_id`). Every page also draws a **top banner** (`1/7 PAGE ORDER  P0  -`) so a person can see which step they are on without decoding the QR.

After the pattern (and QR), optional **[effects](defects.md)** (skew, speckles, streaks, hole punches) and **ColorProcessing** (RGB24 / Gray8) run. More charts (`testpage`, `nozzle`, `info`, `anac`, …) are listed in [roadmap.md](roadmap.md).

## Tester progression (recommended)

Walk these in order. Each step sets pattern + feeder knobs and puts the check text on `GET /v1/status`.

```bash
curl http://127.0.0.1:3920/v1/scenarios
curl -X POST http://127.0.0.1:3920/v1/scenario \
  -H "content-type: application/json" \
  -d '{"id":"order"}'
# acquire… then:
curl -X POST http://127.0.0.1:3920/v1/scenario -H "content-type: application/json" -d '{"id":"qr"}'
# … tone → color → orient → crop → duplex
```

| Step | Id | Pattern | Check |
|------|----|---------|-------|
| 1/7 | `order` | `page_id` | Multi-page ADF: indices 0,1,2… (QR `page=` / COM) |
| 2/7 | `qr` | `qr` | Decode QR (zbar/zxing/rqrr) — no OCR |
| 3/7 | `tone` | `wedges` | Gray steps visible; threshold should drop steps |
| 4/7 | `color` | `patches` | RGB separable only in color modes |
| 5/7 | `orient` | `slant_f` | Big F stays top-left under rotate/deskew |
| 6/7 | `crop` | `corners` | All four L-marks present on full page |
| 7/7 | `duplex` | `duplex_pair` | Fronts pass primed; then reverse backs (below) |

Status fields while a scenario is active: `scenario`, `scenario_step`, `scenario_of`, `scenario_check`, `scenario_next`. `POST /v1/pattern` clears the scenario (pattern only).

## Select a pattern

```bash
curl -X POST http://127.0.0.1:3920/v1/pattern \
  -H "content-type: application/json" \
  -d '{"id":"wedges"}'
```

`GET /v1/status` includes `image_pattern`. `POST /v1/reset` and profile reload restore the default pattern, which is `diag` (the parametric test page, [diag-page-v3.md](diag-page-v3.md)). To keep the old numbered-bars page as the default start the daemon with `--default-pattern page_id` (or `FAKESCAN_DEFAULT_PATTERN=page_id`); any pattern id works there. `POST /v1/pattern` still selects per device at runtime.

| Id | Purpose |
|----|---------|
| `diag` | **Default.** Parametric diagnostic page, layout v3 revision 1 (print marks, colour bars, ColorChecker, ramps, flat fields, resolution targets, orientation / duplex / page-order markers, OpenCV-compatible ArUco markers, chessboard / ChArUco / circle-grid boards, dual-unit rulers, QR with the page parameters); scales to the sheet. [diag-page-v3.md](diag-page-v3.md) |
| `page_id` | Numbered bars + center block (multi-page order); the previous default, see `--default-pattern` |
| `wedges` | Gray step wedges (brightness / contrast / threshold) |
| `patches` | R / G / B + gray patches (color mode) |
| `slant_f` | Large asymmetric F in the top-left (orientation) |
| `corners` | L-marks in four corners + center cross (crop / region) |
| `duplex_pair` | Even → blue front, odd → red back; also COM `fakescan-side:front\|back` |
| `qr` | Large centered QR (decode robustness) |
| `wia_plain` | WIA accept trial: gray wedges only — **no** banner, QR, or COM; baseline JPEG Q≈80 (`EncodeOpts::WIA_PLAIN`) |
| `info` | Server diagnostics: profile, UUID, DPI, format, job id, client IP, timestamp. QR carries `client=<ip>` when caller is known. |
| `anac` / `whoami` | ANAC-style client IP readback: caller's `ip:port` in large text, centred. QR carries `client=<ip>`. |
| `slant_edge` | ~5° slanted edge (ISO 16067-1 style; no MTF math) — dark left / light right with narrow gradient transition; use to check for edge fringing or softening |
| `textladder` | Text size ladder — the phrase "SCAN TEST 12345 ABCDE" rendered at seven scales (8× → 1×); the smallest readable row indicates effective resolution |
| `testpage` | All-in-one composite (see below) |
| `cmyk` | CMYK bars + tint ladders — four columns (C, M, Y, K) with solid 100 % bar (labelled), eight tint steps fading to light, and a bottom overprint strip (C+M, M+Y, C+Y, C+M+Y); RGB-approximated |
| `nozzle` | Nozzle-check bands — 7 colour channels (C, M, Y, K, R, G, B) as horizontal line groups in the top half and vertical line groups in the bottom half; a missing or broken stripe indicates a channel gap |
| `align` | Alignment / registration crosses — centre accent crosshair, corner registration crosses with open-square surrounds, mid-edge marks, and tick marks at regular intervals along all four edges |
| `grid` | Margin frames + scale rules — inner 0.5-inch margin frame, light inch-grid, and ruler ticks (inch + quarter-inch) along all four edges; page is letter (8.5 × 11 in) so tick spacing scales with DPI |
| `lines` | Resolution line ladder — six coarse-to-fine horizontal line groups (left half) and the same six frequencies as vertical lines (right half); half-periods 32, 16, 8, 4, 3, 2 px so effective lpi scales with DPI |
| `diag_v2` / `diag_v1` | **Self-describing diagnostic page** for blind analysis of image-handling mistakes (flip / mirror / rotate / invert / BGR / bit order / stride shear / scale / crop / threshold); laid out in millimetres — see "diag layout" below |

## diag layout

> **`diag` is now layout v3** ([diag-page-v3.md](diag-page-v3.md)), a different, parametric page. The fixed-size layouts below stay available byte for byte: `diag_v2` and `diag_v1` (the blind `pdfcraft_fakescan.pattern_analyzer` reads these). **Layout v2** (`diag_v2`) adds print marks, rulers, resolution / colour targets and geometry on top of everything below, which is unchanged (`diag_v1` renders the old page byte for byte). The QR payload gains `|lay=2`. See [diag-page-v2.md](diag-page-v2.md), the ground-truth layout JSON and the Python analyzer (`fakescan.diag`).

`diag` is a byte-order-mark for pixels: any plausible handling mistake leaves a signature a blind
analyzer can read from the image alone (analyzer: `pdfcraft_fakescan.pattern_analyzer`, see
pdfcraft-fakescan `docs/pattern-analyzer.md`). The layout is in **millimetres, top-left anchored**
(`px = round(mm * dpi / 25.4)`, dpi is capped to 150 when the 4 MP limit applies, i.e. a 300 dpi
request renders a 150 dpi page). Layout box 206 x 288 mm; constants live in `image_gen::diag`.
Everything is deterministic: the same page index gives identical bytes (the page index only changes the QR).
Nothing the device publishes (ScannerConfiguration, formats, dpi, colours) is affected.

| Feature | Position (mm) | Answers |
|---------|---------------|---------|
| Banner (tester banner, stamped as usual) | y 0-12 | - |
| Corner fiducials: comb with 1 / 2 / 3 / 4 teeth at TL / TR / BL / BR, 16 mm, spine on the left | (4,14) (186,14) (4,268) (186,268) | rotation 0/90/180/270, mirror-x, flip-y (distinct under all 8 symmetries) |
| Barker-13 x3, chip 4 mm, horizontal (8 mm tall) and vertical (8 mm wide) | h (24,14); v (4,34) | shift/crop (peak position), scale (peak spacing 52 mm), polarity (peak sign), mirror (reversed template) |
| m-sequence 63 (h: x^6+x+1, v: x^6+x^5+x^2+x+1), chip 2.5 mm | h (24,24); v (13,34) | same as above, more robust to blur/JPEG |
| Polarity wedge: 1 mm black outline, black ref 13 mm, 16 gray steps (17*k) x 7 mm, paper-white ref 13 mm | (26,34)-(166,50) | polarity, gamma/threshold, bit depth |
| Colour patches R,G,B,C,M,Y 20x12 mm | (26,52) | RGB<->BGR (R<->B swap), colour loss |
| Bit-order comb: motif 0xD0 (ink at pixels 0,1,3 of every byte), 8 rows, byte-aligned start, length 8n+5 px | y 67 | reversed bit order in packed bytes (pixel-exact: lost by scaling) |
| 1-px vertical bars, Golomb-ruler positions x 4 px (+ 1-px border at the page edge) | (26,76)-(96) | stride/shear px per row (pixel-exact) |
| Raw bit strip: 128 x 1 mm chips (Barker-13 preamble + xorshift32 PRBS seed 0xD1A60001, MSB) | (26,98) | raw bit error rate, no ECC |
| 2-D position array: 36 x 36 cells of 2 mm, every 4x4 window unique (xorshift32 seed 0xA55A1234, DFS) | (26,108) | absolute position, crop, scale, rotation, flip |
| Zone plate: 40 mm square, f(r)=1.2*r/20 cycles/mm | centre (128,128) | resampling, aliasing/moire, x/y scale |
| QR (payload contract unchanged; bottom-right corner anchored at) | (184,264), ~44-52 mm | page identity; ECC hides small corruption, hence the raw strip |

Features are >= 3 px at 100 dpi except the deliberately pixel-exact ones (comb, 1-px bars, border), which
are 1 px by design and do not survive resampling; use the Barker / m-sequence strips, position array and
zone plate for scale. At 100 dpi a 1-bit threshold keeps fiducials, wedge ends, strips, position array
and QR; mid-gray wedge steps collapse and the zone plate becomes a binary chirp.

## testpage composition

`testpage` divides the page into four horizontal bands:

| Band | Height | Content |
|------|--------|---------|
| Top | 45 % | Left 38 %: gray step wedges (black → white) · Right 62 %: ~5° slant edge |
| Patches | 12 % | R / G / B / gray cells (four equal columns) |
| Ladder | 33 % | Text size ladder at three scales: 5×, 3×, 1× |
| Corners | 10 % | L-corner marks at the four corners of the band + a centre cross |

All sub-regions are drawn with primitive `fill_rect` / `draw_glyph` calls; no recursive JPEG encode. The top banner and corner QR code are stamped by the normal render path.

## Machine-readable IDs (no OCR)

Every pattern stamps a **QR code** (corner, or full-page for `qr`) whose text is:

```text
fakescan|v1|page=0|pat=page_id|side=front
```

| Field | Meaning |
|-------|---------|
| `page` | 0-based RetrieveImage index |
| `pat` | pattern id |
| `side` | `front`, `back`, or `none` |

Decode with any QR library (zbar, zxing, `rqrr`) — no OCR. JPEG COM `fakescan-page:N` (and optional `fakescan-side:`) remains as a second channel that does not need a vision stack.

### Manual duplex stacks (`side` / `order`)

```bash
# Fronts pass (sheet 0..N-1, all tagged front / blue)
curl -X POST http://127.0.0.1:3920/v1/pages \
  -H "content-type: application/json" \
  -d '{"count":3,"side":"front"}'

# Backs pass after flip (RetrieveImage sheet order 2,1,0 tagged back / red)
curl -X POST http://127.0.0.1:3920/v1/pages \
  -H "content-type: application/json" \
  -d '{"count":3,"side":"back","order":"reverse"}'
```

Setting `side` selects `duplex_pair` automatically. Omitting `side` clears forced tagging (parity-based even/odd duplex_pair still applies if that pattern is active). See [test-matrix.md](test-matrix.md) row 8 for the pdfcraft `--reverse` recipe.

Brightness / contrast / threshold from ScanTicket are **not** applied: GetScannerElements advertises them unsupported and CreateScanJob does not parse those fields.
