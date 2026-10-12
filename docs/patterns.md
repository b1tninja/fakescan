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

`GET /v1/status` includes `image_pattern`. `POST /v1/reset` and profile reload restore `page_id`.

| Id | Purpose |
|----|---------|
| `page_id` | Numbered bars + center block (multi-page order); **default** |
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
