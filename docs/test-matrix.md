# What fakescan can test for pdfcraft

pdfcraft’s WIA path is hard to CI without hardware. fakescan’s job is to reproduce the **surfaces and quirks** that code already special-cases.

## Surfaces

| Surface | How pdfcraft sees it | Status | Unlocks |
|---------|----------------------|--------|---------|
| WSD Scan Service | `connection: wsd`, thin caps, no optical | SOAP done; Windows install unverified | network list/dump/acquire, format/dpi refusals, feeder pages |
| WIA USD (future) | `connection: usb`, rich caps, optical | not started | sticky `FEED_READY`, high dpi, bmp-only, STOP_FEEDER |
| Control plane | HTTP `/v1` | done + scenarios + effects | script feeder/pages/faults/effects from pytest/cargo |

## Scenario matrix (pdfcraft behaviors)

| # | Behavior under test | Needs | fakescan knobs | Status |
|---|---------------------|-------|----------------|--------|
| 1 | List shows `usb` vs `wsd` | Windows install / USD | dual profiles | profiles ready; Windows TBD |
| 2 | Optical null on WSD, non-null on USB | caps publish | `optical` in profile | ready |
| 3 | Refuse dpi/format not in lists | ValidateScanTicket + CreateScanJob | profile + `refuse_unknown` | **enabled** (`refuse_unknown: true` on brother-wsd/usb; CreateScanJob refuses) |
| 4 | Glass with sticky feeder ready must not pull ADF | FEED_READY + empty stack | `sticky_ready` + `pages=0` | ready |
| 5 | Auto + empty ADF → glass fallback | sticky ready + empty error | scenario `sticky_empty` | ready |
| 6 | Feeder `--pages N` stops after N | ImagesToTransfer / job pages | `/v1/pages` + job limit | ready |
| 7 | Cancel mid-feeder (STOP_FEEDER) | slow page transfer + CancelJob | `slow_retrieve_ms` | **enabled** (`POST /v1/slow`, `/v1/faults` `{ms}`) |
| 8 | Manual duplex / `--reverse` | two stacks of known images | `/v1/pages` `{side,order}` + pattern `duplex_pair` | **enabled** (synthetic front/back stacks; see below) |
| 9 | Multi-page ADF order | page index in bitmaps | synthetic page markers | **enabled** (JPEG COM `fakescan-page:N` + visual bars; see [patterns.md](patterns.md)) |
| 9c | Hard loopback (byte-identical echo) | uploaded JFIF store | `POST /v1/loopback` + Python client | **enabled** ([lab-harness.md](lab-harness.md)) |
| 9b | Brightness / color / orientation / crop cues | known pixel layouts | `POST /v1/pattern` (`wedges`, `patches`, `slant_f`, `corners`) | **enabled** ([patterns.md](patterns.md)) |
| 10 | Jam / cover open | ScannerStatus reasons | `jam` / `cover_open` faults | **enabled** (`POST /v1/fault` `/v1/condition`; Status JSON; ScannerStatus reads World) |
| 10b | Mid-job jam / empty | RetrieveImage fault after N pages | `jam_after_n` / `empty_mid_job` | **enabled** (`POST /v1/fault`; see [defects.md](defects.md)) |
| 11 | Capability-aware UI (hide missing controls) | published lists differ | `brother-usb` vs `brother-wsd` | ready |
| 12 | Dump must not crash on WSD props | live WSD device | install fakescan as WSD | Windows TBD |

## Manual duplex / pdfcraft `--reverse` (row 8)

pdfcraft’s `--reverse` is **manual duplex** when the feeder has no hardware duplex bit: scan fronts first, flip the whole stack (last sheet on top), scan backs (they arrive last-sheet-first), then interleave. fakescan models that with two feeder loads — no real `fronts/` / `backs/` dirs required.

### Control plane

| Knob | Path / field | Effect |
|------|--------------|--------|
| Pattern | `POST /v1/pattern` `{"id":"duplex_pair"}` | Even → blue/front, odd → red/back (auto side COM) |
| Stack + side | `POST /v1/pages` `{"count":N,"side":"front"\|"back"}` | Forces that face on every RetrieveImage; auto-selects `duplex_pair` |
| Flip order | `POST /v1/pages` `...,"order":"reverse"` | Sheet indices come out last-first (backs as scanned after flip) |
| Status | `GET /v1/status` | `page_side`, `page_order`, `image_pattern`, `pages_remaining` |

JPEG COM markers: `fakescan-page:N` (sheet index) and `fakescan-side:front|back`. Plain `{"count":N}` still only sets the feeder count (existing behavior).

### Unit / SOAP (no WIA)

`World::apply_pages` + RetrieveImage: load fronts → retrieve → load backs with `order=reverse` → retrieve → reverse backs and zip with fronts (same as `interleave_manual_duplex`). Covered by `apply_pages_front_then_reverse_backs_for_manual_duplex`.

### Live pdfcraft once WIA install works

```bash
# Pass 1 — fronts
curl -X POST http://127.0.0.1:3920/v1/pages \
  -H "content-type: application/json" \
  -d '{"count":3,"side":"front"}'
pdfcraft-cli scan --device fakescan --out DIR --pages 3 --yes

# Pass 2 — flip stack, backs last-sheet-first
curl -X POST http://127.0.0.1:3920/v1/pages \
  -H "content-type: application/json" \
  -d '{"count":3,"side":"back","order":"reverse"}'
pdfcraft-cli scan --device fakescan --out DIR --reverse --yes
```

Expect interleaved pages F0,B0,F1,B1,F2,B2 (verify with COM markers or blue/red faces). Blocked today on Windows Imaging install of fakescan ([e2e-wia-status.md](e2e-wia-status.md)).

## Human smoke path (patterns)

Without a full matrix run, walk `GET /v1/scenarios` → `POST /v1/scenario` for `order` → `qr` → `tone` → `color` → `orient` → `crop` → `duplex`. Banner + QR + `scenario_check` on status tell the tester what to look for. Details: [patterns.md](patterns.md).

Optional defect stress (any pattern):

```bash
curl -X POST http://127.0.0.1:3920/v1/effects \
  -H "content-type: application/json" -d '{"preset":"dirty_adf","seed":1}'
# or: skewed / punched — see defects.md
```

Gray8: put `<wscn:ColorProcessing>Grayscale8</wscn:ColorProcessing>` in the ScanTicket (CreateScanJob).

## Implementation priority

1. **Control scenarios** (no Windows install) — faults, fixtures, ticket validation  
2. **Live WSD install** — so pdfcraft-cli `scan devices` sees fakescan  
3. **WIA USD** — USB-like path for sticky feeder / STOP_FEEDER without a Brother  

Items 1 are implemented in-tree; 2–3 remain.
