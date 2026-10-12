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

JFIF encode (all patterns) uses `jpeg-encoder` toward Brother WSD shape: baseline SOF0, **4:2:0**, APP0 density in dpi, DRI = one MCU row, component ids 1/2/3. Live WIA Transfer on Fake still CancelJob after that change — see [e2e-wia-status.md](e2e-wia-status.md).

Protocol faults (`jam`, `cover_open`, `slow`, `jam_after_n`, `empty_mid_job`) are separate — see below and [roadmap.md](roadmap.md) for remaining mid-job knobs (`multi_feed`, …).

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

GetScannerElements advertises `RGB24`, `Grayscale8`, and `BlackAndWhite1` (WSD ColorEntry; not the shorthand `BW1`).

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
| `ms` | Same as `POST /v1/slow` |
| `jam_after_n` | Trip MediaJam when `page_index >= N` (0 = off) |
| `empty_mid_job` | Return NoImagesAvailable when `page_index >= N` (0 = off) |
| `lamp_warmup_ms` | Extra delay on the first RetrieveImage of each job |

Still later (roadmap): `multi_feed`, `abort_retrieve`.

Python / PowerShell:

```bash
fakescan-lab fault --jam-after-n 2
# or: .\scripts\fakescan.ps1 fault "{\"jam_after_n\":2}"
```

Hard page echo (upload → exact RetrieveImage) is separate: [lab-harness.md](lab-harness.md) (`POST /v1/loopback`). Effects and JPEG COM synthesis do **not** apply while loopback is active (exact echo).
