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
| 1 | List shows `usb` vs `wsd` | Windows install / USD | dual profiles | SOAP profiles automated; WIA Connect still fails (see below) |
| 2 | Optical null on WSD, non-null on USB | caps publish | `optical` in profile | **automated** (`test_local_matrix.py`) |
| 3 | Refuse dpi/format not in lists | ValidateScanTicket + CreateScanJob | profile + `refuse_unknown` | **automated** |
| 4 | Glass with sticky feeder ready must not pull ADF | FEED_READY + empty stack | `sticky_ready` + `pages=0` | **automated** (platen job leaves the tray) |
| 5 | Auto + empty ADF → glass fallback | sticky ready + empty error | `sticky_empty` scenario + pdfcraft `acquire` retries glass | **automated** (`sticky_empty` on `brother-usb`: ADF Create faults; Platen OK; tray stays sticky) |
| 6 | Feeder `--pages N` stops after N | ImagesToTransfer / job pages | `/v1/pages` + job limit | **automated** |
| 7 | Cancel mid-feeder (STOP_FEEDER) | slow page transfer + CancelJob | `slow_retrieve_ms` | **automated** (delay + CancelJob drops the job) |
| 8 | Manual duplex / `--reverse` | two stacks of known images | `/v1/pages` `{side,order}` + pattern `duplex_pair` | **automated** on SOAP; pdfcraft-cli still blocked on WIA Connect |
| 8b | Hardware ADF duplex | `ADFSupportsDuplex` + `ADFBack`; `InputSource` ADFDuplex | `feeder.duplex` in profile | **automated** (front then back per sheet; glass-only profiles refuse ADFDuplex) |
| 9 | Multi-page ADF order | page index in bitmaps | synthetic page markers | **automated** (COM `fakescan-page:N`) |
| 9c | Hard loopback (byte-identical echo) | uploaded JFIF store | `POST /v1/loopback` + Python client | **automated** |
| 9b | Brightness / color / orientation / crop cues | known pixel layouts | `POST /v1/pattern` (`wedges`, `patches`, `slant_f`, `corners`) | **automated** (four layouts differ; Gray8 drops chroma) |
| 10 | Jam / cover open | ScannerStatus reasons | `jam` / `cover_open` faults | **automated** on SOAP; WIA `handling_status` blocked on Connect |
| 10b | Mid-job jam / empty | RetrieveImage SOAP subcode `wscn:ClientErrorMediaJam` / `wscn:ClientErrorNoImagesAvailable` after N pages | `jam_after_n` / `empty_mid_job` | **automated** on SOAP; pdfcraft keeps the jam sentence after pages (`after_transfer`) |
| 10c | Offline / mid-scan drop | Status `AttentionRequired`; Subscribe/Create/Retrieve `ServerErrorNotAcceptingJobs` | `offline` / `offline_after_n` | **automated** on SOAP + local matrix; sticky offline until `reset`; pdfcraft mid-scan → stayed-online / offline sentences; ignored WIA labs |
| 10d | Multi-feed / double-pick | Status `MultipleFeedError`; mid-job RetrieveImage `ClientErrorMultipleFeed` | `multi_feed` / `multi_feed_after_n` | **automated** on SOAP + unit; pdfcraft preflight / `after_transfer` → “more than one sheet”; ignored WIA lab |
| 10e | Mid-MTOM abort | RetrieveImage HTTP body stream errors after N bytes (`Content-Length` still full) | `abort_retrieve` / `abort_retrieve_after_n` / `abort_retrieve_bytes` | **automated** on SOAP + unit; pdfcraft → interrupted / stayed-online; ignored WIA lab |
| 10f | Lamp error / busy | Status `LampError` / `Processing`; Subscribe+Create refuse | `lamp_error` / `busy` | **automated** on unit + SOAP + local matrix; pdfcraft lamp preflight / busy sentence; ignored WIA labs |
| 11 | Capability-aware UI (hide missing controls) | published lists differ | `brother-usb` vs `brother-wsd` | **automated** (connection, optical, formats) |
| 12 | Dump must not crash on WSD props | live WSD device | install fakescan as WSD | blocked: WIA Connect fails before dump |

## Sources, trays, and duplex (rows 4 / 8 / 8b)

Scanners do **not** have printer-style input trays. fakescan models the usual two **sources** (and glass-only profiles for one):

| Source | Profile | How to arm |
|--------|---------|------------|
| Glass (platen) | `sources` includes `glass` | CreateScanJob `InputSource` Platen; `glass-wsd` / `glass-jfif` are glass-only |
| ADF feeder | `sources` includes `feeder` | `/v1/pages` + `InputSource` ADF / Feeder |
| Hardware duplex | `feeder.duplex: true` | `ADFSupportsDuplex=1` + `ADFBack`; `InputSource` **ADFDuplex** → 2 faces per sheet (front then back) |
| Manual duplex | any feeder profile | two loads with `side`/`order` (below) — pdfcraft `--reverse` |

Printer multi-tray labs live in **fakeprint** (`InputBins` / Tray1), not here ([siblings.md](siblings.md)).

### Hardware duplex (row 8b)

```bash
# brother-wsd / brother-usb / wia-rich advertise duplex
curl -X POST http://127.0.0.1:3920/v1/pages -H "content-type: application/json" -d '{"count":2}'
# CreateScanJob with <wscn:InputSource>ADFDuplex</wscn:InputSource>
# RetrieveImage ×4 → front, back, front, back (COM fakescan-side + duplex_pair art)
```

`ImagesToTransfer` counts **faces**. Sheets in the feeder × 2 = faces available.

## Manual duplex / pdfcraft `--reverse` (row 8)

pdfcraft’s `--reverse` is **manual duplex** when the feeder has no hardware duplex bit (or the app chooses two-pass): scan fronts first, flip the whole stack (last sheet on top), scan backs (they arrive last-sheet-first), then interleave. fakescan models that with two feeder loads — no real `fronts/` / `backs/` dirs required.

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

## Local matrix (no Imaging install)

`pytest python/tests/test_local_matrix.py` builds fakescan if needed (honors `CARGO_TARGET_DIR`), starts it on `127.0.0.1` with probe listen off, and walks the rows above that do not need a Windows Imaging node: profiles and optical resolution, ticket refusal, sticky feeder plus a platen job that leaves the tray alone, `sticky_empty` (ADF Create empty fault + Platen OK), empty-ADF refusal, `ImagesToTransfer`, slow retrieve and CancelJob, the seven smoke scenarios (`order` through `duplex`) with `fakescan-page:` markers, manual-duplex reverse backs, hardware ADFDuplex (front/back + glass-only refusal), hard loopback, jam / cover / mid-job empty, offline / lamp_error / busy (reset clears), lamp warmup, a dirty-ADF effect page, and `wia-rich` Format Create+Retrieve (magic + multi-page consume).

Rows 1 and 12 still need WIA Connect. The SOAP / local matrix above stays **unelevated**. `.\scripts\fakescan.ps1 wia-trace` prints the COM HRESULT, the elapsed time, and any WSD calls `GET /v1/trace` recorded during that Connect. Zero calls and `0x80210001` in about 26 ms means Windows failed before ScannerService (sticky offline). Rechecked with Fake installed as "Fake WSD scanner (lite caps)" and fakescan answering `http://192.168.1.70:3920`: Connect failed that way and did not open ScannerService. `GET /v1/activity` and `GET /v1/log` are how you see whether Connect produced WSD lines. `GET /v1/jobs` is the scan-task list. Imaging recovery: `helper remove` (or elevated `remove` / `rebind`), Unpair, and a person re-Add while fakescan stays up unelevated — [windows.md](windows.md#helper--elevated--remove--rebind). A banner walk by eye is in [patterns.md](patterns.md).

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
