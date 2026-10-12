# What fakescan can test for pdfcraft

pdfcraft’s WIA path is hard to CI without hardware. fakescan’s job is to reproduce the **surfaces and quirks** that code already special-cases.

## Surfaces

| Surface | How pdfcraft sees it | Status | Unlocks |
|---------|----------------------|--------|---------|
| WSD Scan Service | `connection: wsd`, thin caps, no optical | SOAP done; Windows install unverified | network list/dump/acquire, format/dpi refusals, feeder pages |
| WIA USD (future) | `connection: usb`, rich caps, optical | not started | sticky `FEED_READY`, high dpi, bmp-only, STOP_FEEDER |
| Control plane | HTTP `/v1` | done + scenarios | script feeder/pages/faults from pytest/cargo |

## Scenario matrix (pdfcraft behaviors)

| # | Behavior under test | Needs | fakescan knobs |
|---|---------------------|-------|----------------|
| 1 | List shows `usb` vs `wsd` | Windows install / USD | dual profiles |
| 2 | Optical null on WSD, non-null on USB | caps publish | `optical` in profile |
| 3 | Refuse dpi/format not in lists | ValidateScanTicket + CreateScanJob | profile + `refuse_unknown` |
| 4 | Glass with sticky feeder ready must not pull ADF | FEED_READY + empty stack | `sticky_ready` + `pages=0` |
| 5 | Auto + empty ADF → glass fallback | sticky ready + empty error | scenario `sticky_empty` |
| 6 | Feeder `--pages N` stops after N | ImagesToTransfer / job pages | `/v1/pages` + job limit |
| 7 | Cancel mid-feeder (STOP_FEEDER) | slow page transfer + CancelJob | `slow_retrieve_ms` |
| 8 | Manual duplex / `--reverse` | two stacks of known images | fixture dirs `fronts/` `backs/` |
| 9 | Multi-page ADF order | page index in bitmaps | synthetic page markers |
| 10 | Jam / cover open | ScannerStatus reasons | `jam` / `cover_open` faults |
| 11 | Capability-aware UI (hide missing controls) | published lists differ | `brother-usb` vs `brother-wsd` |
| 12 | Dump must not crash on WSD props | live WSD device | install fakescan as WSD |

## Implementation priority

1. **Control scenarios** (no Windows install) — faults, fixtures, ticket validation  
2. **Live WSD install** — so pdfcraft-cli `scan devices` sees fakescan  
3. **WIA USD** — USB-like path for sticky feeder / STOP_FEEDER without a Brother  

Items 1 are implemented in-tree; 2–3 remain.
