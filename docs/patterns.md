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
