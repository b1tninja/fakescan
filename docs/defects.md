# Scan defects and color (shipped)

RetrieveImage draws a [pattern](patterns.md), then applies **effects**, then **color**, then encodes the job Format ([formats.md](formats.md)). QR is in pixels for most patterns (omitted for `wia_plain`); JPEG COM markers (`fakescan-page:`) only on JFIF embeds when enabled.

```text
pattern → effects → color → encode (jfif | exif | dib | png | tiff | pdf-a | …)
```

### JPEG COM markers (WIA experiment)

Default **on** (airscan / multi-page order tests). Omit for WIA RetrieveImage accept trials:

```powershell
# control plane (live)
Invoke-RestMethod -Method POST -Uri http://127.0.0.1:3920/v1/jpeg-com `
  -ContentType application/json -Body '{"enabled":false}'

# or env at process start (overrides profile `jpeg_com`)
$env:FAKESCAN_JPEG_COM = '0'
```

`GET /v1/status` → `jpeg_com`. Profile field `jpeg_com` (default true). `POST /v1/reset` restores from profile/env.

### WIA plain encode (`wia_plain`)

COM-off alone still stamps QR + banner at Q95. For a minimal baseline JFIF (wedges only, no COM / QR / banner, Q≈80):

```powershell
Invoke-RestMethod -Method POST -Uri http://127.0.0.1:3920/v1/pattern `
  -ContentType application/json -Body '{"id":"wia_plain"}'
# optional (redundant for this pattern — encode_opts_for forces WIA_PLAIN):
Invoke-RestMethod -Method POST -Uri http://127.0.0.1:3920/v1/jpeg-com `
  -ContentType application/json -Body '{"enabled":false}'
```

`EncodeOpts` knobs: `inject_jpeg_com`, `stamp_qr`, `stamp_banner`, `jpeg_quality`. Pattern `wia_plain` selects `EncodeOpts::WIA_PLAIN` regardless of `jpeg_com`.

JFIF encode (all patterns) uses `jpeg-encoder` toward Brother WSD shape: baseline SOF0, **4:2:0**, APP0 density in dpi, DRI = one MCU row, component ids 1/2/3, then `reshape_jpeg_brother_layout` packs **one DQT + one DHT** and orders APP0→DQT→DRI→SOF→DHT→SOS. Live WIA Transfer on Fake still CancelJob after that change — see [e2e-wia-status.md](e2e-wia-status.md).

Protocol faults (`jam`, `cover_open`, `multi_feed`, `slow`, `jam_after_n`, `empty_mid_job`, `offline`, `offline_after_n`, `multi_feed_after_n`, `abort_retrieve`, …) are separate — see below and [roadmap.md](roadmap.md).

## Set effects

```bash
curl -X POST http://127.0.0.1:3920/v1/effects \
  -H "content-type: application/json" \
  -d '{"preset":"dirty_adf","seed":1}'

curl -X POST http://127.0.0.1:3920/v1/effects \
  -H "content-type: application/json" \
  -d '{"skew_deg":1.2,"hole_punch":3,"seed":42}'

# clear
curl -X POST http://127.0.0.1:3920/v1/effects \
  -H "content-type: application/json" \
  -d '{"clear":true}'
```

`GET /v1/status` → `effects`. `POST /v1/reset` clears effects.

Unset keys **merge** into the current effects. Use `"clear":true` or preset `clean` to wipe.

### Knobs

| Field | Meaning |
|-------|---------|
| `seed` | Deterministic RNG (same seed + page → same pixels) |
| `skew_deg` | Page rotation (degrees); white fill |
| `speckles` | Dust density 0–1 |
| `streaks` | Full-page streak count |
| `streak_axis` | `vertical` (ADF default) or `horizontal` |
| `hole_punch` | Binder holes — darkened rings on the left edge |
| `scratches` | Thin dark polylines (glass scratches) |
| `blur_px` | Box-blur radius |
| `brightness` | Multiplier (1.0 = unchanged) |
| `contrast` | Around mid-gray (1.0 = unchanged) |
| `jpeg_quality` | 1–100 for JPEG embeds; `0` = default 95 |
| `banding` | Horizontal illumination / roller bands |
| `noise` | Additive sensor noise 0–1 |
| `vignette` | Edge darkening 0–1 |
| `margin_crop_px` | Crop then white-pad (autocrop miss) |
| `show_through` | Duplex bleed / ghost 0–1 |
| `moire` | Halftone interference overlay (`true`/`false`) |
| `bilevel_polarity` | Not a defect: bit sense of `BlackAndWhite1` output. `black_is_zero` (default: BMP palette `[black, white]`, TIFF `MinIsBlack`) or `white_is_zero` (`[white, black]`, `MinIsWhite`); the visual is identical. Applies to BMP/DIB and uncompressed TIFF; G4/G3 keep `WhiteIsZero`, PNG gray is always 0=black, JPEG-family BW follows `jfif_bw`. |
| `jfif_bw` | Not a defect: wire answer for `BlackAndWhite1` + JFIF/JPEG/EXIF (JPEG cannot carry 1-bit). `gray8` (default): FinalParameters/ValidateScanTicket say Grayscale8 (Override), BytesPerLine = width, stream is a one-component JPEG whose samples are the thresholded 0/255 values. `rgb24`: legacy Brother-style fallback, kept selectable (RGB24, BytesPerLine = width x 3, 3-component JPEG with bilevel samples; the Brother contract). `reject`: spec-faithful, ValidateScanTicket and CreateScanJob fault `ClientErrorDocumentFormatNotSupported` so the Windows driver falls back to another Format (PNG, TIFF G4...). TIFF/BMP/PNG/DIB BW stay real 1-bit in every mode; the published ScannerConfiguration never changes. Kept across presets, reset by `clear`. Switch: `POST /v1/effects {"jfif_bw":"gray8"\|"rgb24"\|"reject"}` (or `fakescan ctl effects`). |
| `refuse_tickets` | Not a defect: list of rules `{format?, color?, input_source?, resolution?, fault?}`; a ticket is refused when every field present matches (absent = wildcard). `format` is a wire token (`tiff-single-uncompressed`, `jfif`, ...; matched against the requested or resolved Format), `color` `BlackAndWhite1`\|`Grayscale8`\|`RGB24`, `input_source` `Platen`\|`ADF`\|`ADFDuplex`, `resolution` dpi. `fault` defaults to `ClientErrorDocumentFormatNotSupported` (wire `wscn:ClientErrorFormatNotSupported`); also `ClientErrorFormatNotSupported`, `ClientErrorConflictingRequiredParameters`, `ClientErrorNoImagesAvailable`. Applied to both ValidateScanTicket and CreateScanJob; recorded in `GET /v1/tickets` (`fault:<subcode>` / `refused:<subcode>`). Independent of `jfif_bw=reject` (either may fault). Shown in `GET /v1/status` effects; kept across presets, reset by `clear`; `[]` or `null` clears; a malformed rule is HTTP 400. Example: `POST /v1/effects {"refuse_tickets":[{"format":"tiff-single-uncompressed","color":"BlackAndWhite1"}]}`. |
| `jfif_gray` | Not a defect: how a `Grayscale8` + JFIF/JPEG/EXIF ticket is answered (live-switchable via `POST /v1/effects`; never alters the published ScannerConfiguration; `clear`/reset restore the default). `gray8` (**default**): consistent gray, ColorProcessing Grayscale8, BytesPerLine = width, genuine 1-component JPEG (EXIF keeps its APP1); live-proven on Windows 11 (2026-10-12). `rgb24` (legacy opt-in): ColorProcessing coerced to RGB24 (Override), BytesPerLine = width x 3, 3-component JPEG; **known to be rejected by the Windows inbox driver** (CreateScanJob then no RetrieveImage), kept for tests of the old coercion. `gray8_rgb_stride`: gray token and JPEG, BytesPerLine = width x 3 (also accepted live). `gray8_tiff`: Format answered `tiff-single-uncompressed` (Override) with an 8-bit gray TIFF (rejected live, like `rgb24`; needs TIFF in the profile). |

### Presets

| Preset | Sets |
|--------|------|
| `clean` | All off |
| `skewed` | `skew_deg ≈ 1.5` |
| `dirty_adf` | Vertical streaks + speckles |
| `punched` | `hole_punch=3` + slight skew |
| `dirty_glass` | Speckles + scratches + light horizontal streak |
| `dim` | Lower brightness / contrast |
| `harsh_jpeg` | Low `jpeg_quality` + slight blur |
| `office_bad` | Skew + dust + streaks + scratches + banding + noise + vignette + harsh JPEG |

## ColorProcessing

CreateScanJob / ValidateScanTicket read `<wscn:ColorProcessing>`:

| Ticket value | Result |
|--------------|--------|
| `RGB24` (default) | Color |
| `Grayscale8` / `Grayscale` / `Gray8` | Luma (R=G=B) |
| `BW1` / `BlackAndWhite1` | Threshold bilevel |

GetScannerElements advertises ColorEntry values from the profile `colors` allow-list (WSD tokens: `RGB24`, `Grayscale8`, `BlackAndWhite1` — not the shorthand `BW1`). Empty `colors` → Brother-shaped BW → Gray → RGB.

## Compose recipes

```bash
# Dirty ADF on page-order bars
curl -X POST http://127.0.0.1:3920/v1/pattern -H "content-type: application/json" -d '{"id":"page_id"}'
curl -X POST http://127.0.0.1:3920/v1/effects -H "content-type: application/json" -d '{"preset":"office_bad","seed":1}'
curl -X POST http://127.0.0.1:3920/v1/pages -H "content-type: application/json" -d '{"count":3}'
```

## Protocol faults

```bash
# Immediate jam / cover
curl -X POST http://127.0.0.1:3920/v1/fault -H "content-type: application/json" \
  -d '{"jam":true}'

# Jam after 2 successful RetrieveImage transfers on a job
curl -X POST http://127.0.0.1:3920/v1/fault -H "content-type: application/json" \
  -d '{"jam_after_n":2}'

# Feeder appears empty after 2 transfers (NoImagesAvailable; no jam)
curl -X POST http://127.0.0.1:3920/v1/fault -H "content-type: application/json" \
  -d '{"empty_mid_job":2}'
```

| Field | Meaning |
|-------|---------|
| `jam` / `cover_open` | Immediate ScannerStatus reasons |
| `multi_feed` | Immediate `MultipleFeedError` (double-pick) |
| `lamp_error` | Immediate `LampError` + refuse job ops |
| `busy` | Status `Processing`; refuse Subscribe/Create/Retrieve |
| `offline` | Sticky offline (`AttentionRequired` + NotAcceptingJobs) |
| `ms` | Same as `POST /v1/slow` `ms` (pre-retrieve sleep) |
| `bps` (on `/v1/slow` only) | Throttle RetrieveImage MTOM send (bytes/sec). `0` clears. Keeps `Content-Length` — do not use HTTP chunked framing (wsdscdrv hangs). |
| `jam_after_n` | After N pages, RetrieveImage faults with SOAP subcode `wscn:ClientErrorMediaJam` (0 = off) |
| `cover_open_after_n` | After N pages, RetrieveImage faults with `wscn:ClientErrorCoverOpen`, sticky `CoverOpen` in ScannerStatus (0 = off). Sugar for a fault-plan entry |
| `multi_feed_after_n` | After N pages, RetrieveImage faults with `wscn:ClientErrorMultipleFeed` (0 = off) |
| `empty_mid_job` | After N pages, RetrieveImage faults with SOAP subcode `wscn:ClientErrorNoImagesAvailable` (0 = off) |
| `offline_after_n` | After N pages, go offline and refuse further Retrieve/Create (0 = off) |
| `abort_retrieve` | Cut every RetrieveImage MTOM mid-send (stream error; full `Content-Length`) |
| `abort_retrieve_after_n` | After N pages, abort MTOM mid-send and arm sticky `abort_retrieve` (0 = off) |
| `abort_retrieve_bytes` | Bytes to send before abort (`0` = half the assembled MTOM body) |
| `lamp_warmup_ms` | Extra delay on the first RetrieveImage of each job |
| `empty_feeder_brother` | Reproduces the real Brother with an EMPTY feeder. Published status stays `Idle`/`None` with no conditions (no `InputTrayEmpty`); a feeder/ADF `CreateScanJob` with 0 sheets loaded is refused with the `busy` fault (`ServerErrorNotAcceptingJobs`) only after `empty_feeder_delay_ms` (async sleep; other requests keep being served; Windows sees BUSY 0x80210006). Sheets loaded or glass jobs: unchanged. Unarmed: the old quick generic "ADF selected but feeder empty" failure. Cleared by `reset`. Published capabilities are never touched. Exposed in `GET /v1/status` as `empty_feeder_brother`, and as a toggle in the web UI fault panel. |
| `empty_feeder_delay_ms` | Delay for `empty_feeder_brother` (default 10000; `reset` restores the default; tests use a few ms) |

Arm: `POST /v1/fault {"empty_feeder_brother":true,"empty_feeder_delay_ms":10000}` (or `fakescan ctl fault`). The code only takes effect in a daemon restarted from the build that contains it; the lead restarts the live daemon.

Python / PowerShell:

```bash
fakescan-lab fault --jam-after-n 2
# or: .\scripts\fakescan.ps1 fault "{\"jam_after_n\":2}"
```

Hard page echo (upload → exact RetrieveImage) is separate: [lab-harness.md](lab-harness.md) (`POST /v1/loopback`). Effects and JPEG COM synthesis do **not** apply while loopback is active (exact echo).

## Fault plan: interrupt a scan mid-scan or between pages

One mechanism for every interruption. `jam_after_n`, `offline_after_n`, `multi_feed_after_n`,
`empty_mid_job` and `abort_retrieve*` keep working unchanged (they show up as read-only `legacy` rows
in `fault_plan`); `cover_open_after_n` is plain sugar that inserts a plan entry.

```bash
# Cover opens after 2 pages (between pages)
curl -X POST http://127.0.0.1:3920/v1/faults/plan -H "content-type: application/json" \
  -d '{"faults":[{"kind":"cover_open","when":{"after_pages":2}}]}'
# Cover opens while page 2 is streaming: the body is cut at 50 %, page 2 is NOT delivered
  -d '{"faults":[{"kind":"cover_open","when":{"page":2,"fraction":0.5}}]}'
# Live: applies now, also to a job that is already running
  -d '{"kind":"jam","when":"now"}'
curl http://127.0.0.1:3920/v1/faults/plan            # list (also in /v1/status "fault_plan")
curl -X DELETE http://127.0.0.1:3920/v1/faults/plan  # clear the plan only; /v1/reset clears everything
```

Entry: `{kind, when, sticky?, one_shot?, job?, pages?}`.

| `kind` | WS-Scan fault on the retrieve that fails | Condition left in ScannerStatus (sticky) | Default sticky |
|--------|------------------------------------------|------------------------------------------|----------------|
| `jam` | `wscn:ClientErrorMediaJam` | Stopped, `MediaJam` (Id 1, ADF) | yes |
| `cover_open` | `wscn:ClientErrorCoverOpen` | Stopped, `CoverOpen` (Id 2) | yes |
| `multi_feed` | `wscn:ClientErrorMultipleFeed` | Stopped, `MultipleFeedError` (Id 3) | yes |
| `lamp_error` | `wscn:ServerErrorNotAcceptingJobs` | Stopped, `LampError` (Id 4) | yes |
| `interlock_open` | `wscn:ClientErrorInterlockOpen` | Stopped, `InterlockOpen` (Id 6) | yes |
| `offline` | `wscn:ServerErrorNotAcceptingJobs` | Stopped, `AttentionRequired` | yes |
| `empty` | `wscn:ClientErrorNoImagesAvailable` | none; job ends `Completed` / `JobCompletedWithErrors` | no |
| `busy` | `wscn:ServerErrorNotAcceptingJobs` | none; one refusal | no |
| `abort` | none (transport only: the stream is cut) | none | no |

Fault subcode choice: `ScanServiceFaultValuesType` in `WDPScan.xsd` has no cover / door / jam value
([reference/faults.md](reference/faults.md)). `ClientErrorMediaJam`, `ClientErrorCoverOpen`, `ClientErrorMultipleFeed`
and `ClientErrorInterlockOpen` are the lab/driver extension family this repo already used for the sticky
(pre-flight) conditions, so a mid-scan cover open reuses `wscn:ClientErrorCoverOpen` plus a `CoverOpen`
condition in ScannerStatus (the same pair `cover_open: true` produces before a scan). Nothing was invented.
A non-sticky entry raises the condition and clears it at once (ConditionHistory + NotifyTo see both) and fails
only that retrieve. A sticky fault marks the open job `ScannerStopped` (same terminal handling as `jam_after_n`:
the job stays open until it times out or is cancelled).

`when`:

| `when` | Meaning on the wire |
|--------|--------------------|
| `{"after_pages": N}` | between pages: the (N+1)-th RetrieveImage faults (SOAP 500) |
| `{"page": P, "fraction": F}` or `{"page": P, "bytes": B}` | mid page: RetrieveImage for page P answers 200 with the full `Content-Length`, sends F of the MTOM body (or B bytes), then resets the connection. The page is not counted as delivered (job `page_index` stays P-1). The condition is set when the cut starts. |
| `{"after_ms": T, "from": "create"\|"first_retrieve"}` | timed: evaluated at every protocol point (RetrieveImage, GetScannerElements, `/v1/status`, ctl); also cuts a stream that is in flight when its deadline passes |
| `"now"` | applies at once, also to a running job: sticky kinds set the condition immediately; transient kinds fail the next RetrieveImage; a page that is streaming is cut at the next 8 KiB chunk |

In-flight cutting needs a chunked send; fakescan streams the body in 8 KiB chunks when `bps` is set, a cut is
planned, or any plan entry is pending. Without those a page leaves in one write in milliseconds, so a `now` fault
lands on the next RetrieveImage (set `slow {"bps": N}` to leave a window to cut).

`one_shot` (default true) consumes the entry after it fires; `one_shot:false` fires again on every match (for
example every job's page 2). `job` limits to one JobId; `pages:[from,to]` limits to those 1-based page numbers.
`/v1/status` `fault_plan` lists `kind`, `when`, `trigger`, `sticky`, `one_shot`, `fired`, `fired_count`,
`last_fired`; firing is logged to `/v1/activity` as `fault fired: cover_open mid page 2 @ 50% (job 3) (plan #1)`.
Also available as ctl `fault_plan` (`args`: `{faults:[...]}`, a bare entry, or `{clear:true}`), per device under
`/d/{id}/v1/faults/plan`, in the `/ui` page ("Inject fault"), and in Python:
`Client.inject_fault(kind, when, sticky=, one_shot=)`, `Client.fault_plan()`, `Client.clear_fault_plan()`,
`Client.set_fault(cover_open_after_n=N, interlock_open=True)`.

Page numbers in the plan count along the loaded feeder stack: a stack scanned as several
one-page jobs (the Windows WIA pattern, one `ImagesToTransfer=1` job per Transfer) sees pages 1, 2, 3
across those jobs, so `page: 2` / `after_pages: 1` fire on the second job. For the first job on a stack,
and for glass, this equals the per-job page number.

## Encoder and feeder fixes from the comparison runs

What the harness comparison against the real Brother / Windows decoders found, and what changed. None of
this changes what the device publishes (ScannerConfiguration, formats, dpi lists, colours, page ranges).

| Defect | Fix | Tests |
|---|---|---|
| `tiff-*-jpeg-tn2` wrote `Photometric=RGB` (262=2) around a 4:2:0 YCbCr JPEG; TN2 forbids it, libtiff refuses it and WIC decoded wrong colours | colour pages: `Photometric=YCbCr` (6), `SamplesPerPixel 3`, `YCbCrSubSampling` (530) `[2,2]` = the stream's luma H,V, `YCbCrPositioning` (531) centred; gray/bw pages: one-component stream, `BlackIsZero`, `SamplesPerPixel 1`. Byte layout is deterministic | `image_gen::container_tests::tn2_*` (tags, an independent `tiff`-crate decode, determinism) |
| A job with `ImagesToTransfer=1` emptied the whole feeder, so the next Transfer failed with `ADF selected but feeder empty` and mid-job fault-plan entries never fired | a job consumes only the sheets it asked for (hardware duplex: `ceil(faces/2)`); `0`/absent takes all that are left; over-asking clamps to what is left. The rest stay loaded for the next CreateScanJob and later jobs render the next sheet (`ScanJob.sheet_base`), not sheet 0 again. Fault-plan `page`/`after_pages` count along the stack (`ScanJob::stack_position`). `reverse` order uses the loaded stack size | `state::tests::single_page_adf_jobs_consume_one_sheet_each`, `adf_all_and_overask_take_only_what_is_left`, `fault_plan_page_counts_across_one_page_jobs` |
| PDF `MediaBox` was the pixel size; gray/bw used `DeviceRGB` | `MediaBox` and the image `cm` are `pixels * 72 / dpi` points; gray and bw use a one-component JPEG with `/DeviceGray` | `container_tests::pdf_mediabox_*` |
| XPS pages rendered at the wrong scale (MuPDF grid MAD 5-21 vs 0.5 for PDF) | `FixedPage` Width/Height, the geometry, `Viewbox` and `Viewport` are `pixels * 96 / dpi` (XPS user space is 1/96 inch; the JPEG carries the same dpi); gray/bw use one-component JPEG. Checked with PyMuPDF 1.28: page 593.25 x 823.5 pt at 200 dpi, grid MAD vs the PDF 0.32 | `container_tests::xps_page_*` |
| Resolution metadata missing or constant | JFIF APP0 density (inch, real dpi, already), PNG `pHYs`, BMP `biX/YPelsPerMeter` (all BMPs), TIFF `XResolution`/`YResolution`/`ResolutionUnit=inch` in every flavour (G4/G3 were fixed at 200) | `container_tests::every_tiff_flavour_*`, `jfif_density_png_phys_and_bmp_ppm_*` |
| Gray/bw JPEG2000, PDF, XPS, TIFF-JPEG were 3-component | one component whenever the ticket colour is Grayscale8 or BlackAndWhite1. JFIF/EXIF still follow the `jfif_gray` effect: Gray8 is one component (unchanged); BlackAndWhite1 JFIF/EXIF follows `jfif_bw` (default gray8: one component; `rgb24` keeps three) | `container_tests::gray_and_bw_jpeg2k_*`, `jfif_gray_default_*` |
| 1-bpp BMP was bottom-up; the Brother serves top-down | 1-bpp BMP has a negative `biHeight` (top-down, `BI_RGB`, palette `[black, white]` or `[white, black]` per `bilevel_polarity`); 8-bpp and 24-bpp stay bottom-up | `bilevel_tests::bmp_1bpp_*`, `bmp_8bpp_*`, `container_tests::bmp_24bpp_*` |
| `lamp_warmup_delays_first_image_only` flaked | a 40 ms warm-up is the same order as a debug-build page encode under parallel load; the test compared two wall-clock times. Now a 400 ms warm-up with a bound well below it | the test itself |
