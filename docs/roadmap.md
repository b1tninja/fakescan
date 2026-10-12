# Roadmap catalog

Ideas collected for a rich fakescan test tool. Status: **shipped** | **next** | **later**.

Pick from this list instead of rediscovering motifs. Implementation notes live with each section.

## Image effects (`POST /v1/effects`)

Target body (core fields shipped; others later):

```json
{
  "seed": 42,
  "speckles": 0.02,
  "streaks": 3,
  "streak_axis": "vertical",
  "scratches": 2,
  "blur_px": 0,
  "skew_deg": 0.8,
  "hole_punch": 3,
  "brightness": 1.0,
  "contrast": 1.0,
  "banding": 0,
  "moire": false,
  "show_through": 0.0,
  "vignette": 0.0,
  "noise": 0.0,
  "jpeg_quality": 85,
  "margin_crop_px": 0,
  "preset": "dirty_adf"
}
```

| Effect | Simulates | Status |
|--------|-----------|--------|
| `speckles` | Dust on glass | **shipped** |
| `streaks` + `streak_axis` | ADF / sheetfed dust streaks | **shipped** |
| `skew_deg` | Misfeed rotation | **shipped** |
| `hole_punch` | Binder holes (dark rings) | **shipped** |
| `scratches` | Glass scratches | **shipped** |
| `blur_px` | Soft focus / motion | **shipped** |
| `brightness` / `contrast` | Exposure | **shipped** |
| `jpeg_quality` | Heavy compression | **shipped** |
| `banding` | Roller / illumination bands | **shipped** |
| `moire` | Halftone interference | **shipped** |
| `show_through` | Duplex bleed | **shipped** |
| `vignette` | Edge falloff | **shipped** |
| `noise` | Sensor noise | **shipped** |
| `margin_crop_px` | Autocrop / guide miss | **shipped** |

| Preset | Status |
|--------|--------|
| `clean`, `skewed`, `dirty_adf`, `punched` | **shipped** |
| `dirty_glass`, `dim`, `harsh_jpeg`, `office_bad` | **shipped** |

Prior art: [bad_scanner](https://github.com/LucidTruthTechnologies/bad_scanner), [scan-simulator](https://github.com/s1mb1o/scan-simulator), [scanners.colortest.page](https://scanners.colortest.page/).

## Color spaces

| ColorProcessing | Status |
|-----------------|--------|
| `RGB24` | **shipped** |
| `Grayscale8` / Gray8 | **shipped** |
| `BW1` / threshold | **shipped** |

## Protocol malfunctions

| Knob | Status |
|------|--------|
| `jam` / `cover_open` → ScannerStatus | **shipped** |
| `slow_retrieve_ms` | **shipped** |
| `jam_after_n` | **shipped** (`POST /v1/fault`) |
| `empty_mid_job` | **shipped** (`POST /v1/fault`) |
| `lamp_warmup_ms` | **shipped** (`POST /v1/fault`; first image only) |
| `multi_feed` | later |
| `abort_retrieve` | later |

## Diagnostic pages

| Pattern | Role | Status |
|---------|------|--------|
| `info` | Server diagnostics on the page | later |
| `anac` | Client IP readback ([ANAC](https://en.wikipedia.org/wiki/Automatic_number_announcement_circuit)-style) | later |

Wiring sketch: `PageContext` + axum `ConnectInfo` on RetrieveImage; multi-line banner glyphs; QR `client=`.

## Printer / scanner test-page patterns

| Motif | Id | Status |
|-------|-----|--------|
| Page order bars | `page_id` | **shipped** |
| Gray wedges | `wedges` | **shipped** |
| RGB patches | `patches` | **shipped** |
| Orientation F | `slant_f` | **shipped** |
| Corner L-marks | `corners` | **shipped** |
| Duplex pair | `duplex_pair` | **shipped** |
| QR focus | `qr` | **shipped** |
| CMYK bars + tint ladders | `cmyk` | later |
| Nozzle-check bands | `nozzle` | later |
| Alignment / registration grid | `align` | later |
| Margin frames + scale rules | `grid` | later |
| All-in-one composite | `testpage` | later |
| Resolution line ladder | `lines` | later |
| ~5° slanted edge (ISO 16067-1 style) | `slant_edge` | later |
| Text size ladder | `textladder` | later |

Not planned: licensed IT8 charts, vendor diagnostic bitmaps, in-process MTF math.

## Windows WSD install

| Item | Status |
|------|--------|
| Hello + SOAP Get + Host/Hosted metadata | **shipped** |
| FD query scripts / `fd-installable` verdict | **shipped** |
| Silent Add device | **not possible** via INF — gate is PnP-X association (person click; FD `Associate` unproven for fakescan) |
| Custom INF for inbox WSD | **not needed** — live node matched CompatibleId → `wsdscdrv.inf`; custom INF only for future WIA USD |
| Live fakescan FD Installable=true | **partial** — FD COM often omits fakescan; WinRT AEP + Printers & scanners Add device **works** (lab 2026-10-11) |
| Live WIA install (Printers & scanners) | **shipped** (person click; `wia` / `swd` / verdict `installed`; can drop if WIA hangs / device goes offline) |
| Presence sticky offline after restart | **open** — PnP OK + Device Get poll, WIA `0x80210005`, no `/ScannerService` POST; InstanceId/MetadataVersion bump + Hello burst shipped, **needs live proof** ([windows-client.md](specs/windows-client.md#presence-sticky-offline--open-questions)) |

See [windows.md](windows.md) and [e2e-wia-status.md](e2e-wia-status.md).

## Lab harness

| Item | Status |
|------|--------|
| Hard loopback store (`POST /v1/loopback`) | **shipped** |
| Python control-plane client (`python/`) | **shipped** ([lab-harness.md](lab-harness.md)) |
| PowerShell `Set-FakescanFault` / pages side-order | **shipped** (full loopback stays Python) |
| pdfcraft-cli optional glue (`fakescan-lab loopback-wia`) | **shipped** |

## Docs hygiene

| Item | Status |
|------|--------|
| Doc index | **shipped** (this pass) |
| Accurate README status | **shipped** (this pass) |
| Encode full WSD Format catalog (except jbig) | **shipped** ([formats.md](formats.md); `wia-rich`) |
| `jbig` (JBIG1) | **not planned** (JBIG-KIT GPL; no MIT/Apache encoder) |
