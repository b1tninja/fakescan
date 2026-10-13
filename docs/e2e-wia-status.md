# WIA E2E live status (WSD path)

**Lab log** (dated run notes). Operator checklist: [runbook-fake-scanner.md](runbook-fake-scanner.md). Scripts / dead ends: [windows.md](windows.md).

Date: 2026-10-11 / updated 2026-10-12 (local). Host: Windows, LAN `192.168.1.70`.

## Run

```text
fakescan.exe --listen 0.0.0.0:3920 --advertise 192.168.1.70 \
  --profiles profiles --profile brother-wsd \
  --device-uuid 00000000-0000-4000-8000-0000000000f2 \
  --probe-listen -v
```

(`dasHost` often owns `:3702`; Hello-only still reaches `aep-pairable`.)

## Results

| Step | Result | Notes |
|------|--------|-------|
| SOAP GetScannerElements | **pass** | `Fake WSD scanner (lite caps)` |
| WS-Discovery Hello (`wsd_hellos_sent`) | **pass** | `on-wire-discoverable` → `aep-pairable` |
| Windows SOAP `Get` after Hello | **pass** | dasHost polls Device endpoint (POST WS-Transfer, not HTTP GET) |
| Function Discovery WSD query (COM) | **variable** | fakescan CAN appear with `Installable=true` (confirmed 2026-10-12). Presence is cache-dependent: FD WSD provider shows a freshly-discovered uninstalled device as Installable; an already-installed device (present SWD node) does **not** appear as Installable. Brother `Installable=true` always — it was already installed. `wsd:Scopes` not the cause (Brother omits them too). |
| WinRT WSD AssociationEndpoint | **pass (candidate)** | `Fake WSD scanner (lite caps)` with `CanPair=true` |
| WinRT `PairAsync` | **fail** | Status=`Failed`; does not create Imaging node. |
| **Association (person Add)** | **install gate** | **Printers & scanners** → Add device. Creates WIA + `SWD\DAFWSDPROVIDER\...` Image; HardwareId `UMB\VEN_FAKE&DEV_WSDSCAN`; driver **inbox** `wsdscdrv.inf`. Custom INF / Docker multicast / registry Enum are **not** the gate. |
| Custom INF / silent install | **not the gate** | Association creates the node; CompatibleId already matches inbox. See [windows.md](windows.md) dead ends. |
| Docker Desktop bridge | **not the gate** | Fine for SOAP/CI; multicast Hello usually fails → no Add candidate. |
| Registry `Enum\SWD\DAFWSDPROVIDER` | **not the gate** | Effect of install, not a create API ([windows-client.md](specs/windows-client.md)). |
| `pnputil /scan-devices` | **dead end** | Rescans PnP tree only; no WSD association. |
| WS-Eventing **Subscribe** | **required + implemented** | WIA open faults offline if Subscribe fails. With `ScanDestinations`, response must include **`DestinationResponses`** ([ws-eventing.md](specs/ws-eventing.md)). |
| Empty feeder status | **fixed** | Must be **Idle/None** — do **not** emit `AttentionRequired` (that means human intervention; WIA treated the device as offline). Verified live: `ScannerState=Idle` / `ScannerStateReason=None`. |
| After fakescan **restart** | **sticky WIA offline** | Imaging node can stay “installed” while WIA Connect fails and **no SOAP** reaches fakescan. Recovery: elevated `pnputil` remove Image + SoftwareDevice, `Disconnect-FakescanAssociationEndpoint`, person **re-Add** while fakescan stays up (stable `--device-uuid` / `--advertise`). `Repair-FakescanScanner` runs remove → Disconnect → printers UI. |
| Stale AEP `IsPaired` after remove | **cleared** | `Disconnect-FakescanAssociationEndpoint` so Add list can show Fake (`IsPaired=False CanPair=True`). |
| `Open-FakescanAddDevice` | **fixed** | Opens **only** `ms-settings:printers` (do not also open Bluetooth — steals focus). |
| `pdfcraft-cli scan devices` | **fail / hang** | Opens **every** WIA device; offline Brother USB (`0x80210005` / hang) aborts before fakescan. |
| WIA Connect Fake (live) | **pass** (2026-10-12, re-Add after ghost-aep clear) | COM Connect OK ~20ms; SOAP ValidateScanTicket ×3. Item `Scan` **props=32**, bed 2550×3510 @300 (empty-item gate cleared). |
| Glass acquire (live) | **pass @ accept** (2026-10-12 optical) | Gate was **full-letter SOF/ImageInformation** (1700×2340 @200). Brother deliverable is **1648×2289 @200** / **2480×3440 @300**. With `job_page_pixels` = optical crop: WIA `Transfer()` OK (~11MB BMP @200 / ~25MB @300), **no CancelJob**. Loopback of `D:\temp\wia-variants` BMPs (re-encoded to optical JFIF) and live Brother wire JPEG all accept. |
| pdfcraft glass acquire | **pass** (2026-10-12 night, RGB) | `pdfcraft-cli scan --source glass --dpi 200 --color rgb --format jpeg\|bmp` → CreateScanJob → RetrieveImage → file (JPEG loopback 316180 B / BMP 1648×2289). pdfcraft skips `TYMED_CALLBACK` on WSD ports (`com.rs`). **`--color gray`:** **pass** (2026-10-12, live proof, see "Gray live proof" below). |
| Progress / slow fill | **pass** (bps throttle) | `POST /v1/slow` `{ "bps": 32000 }` paces MTOM with `Content-Length` (~10 s vs ~130 ms). WIA STATUS callbacks climb (`PDFCRAFT_SCAN_STATUS_LOG=1`). `ms` alone = pre-retrieve sleep only. See [lab-harness.md](lab-harness.md). |
| Wire `dib` / raw BMP | **pass (WIA + SOAP)** | MS: `ClientErrorDocumentFormatNotSupported` → driver retries PNG…→uncompressed TIFF. Profile `wia-lossless` (no jfif) + that fault → WIA tickets `dib`, RetrieveImage 11 MB BM @200; pdfcraft `--format bmp` OK. SOAP: `fakescan-lab acquire-dib`. See [formats.md](formats.md). |
| `Remove-FakescanScanner` | **pass** (elevated) | Non-elevated: Access denied. Elevated pnputil cleared Image + parent. Brother untouched. |
| Unit/integration (`cargo test -p fakescan`) | **pass** | |
| `docker compose build` | **pass** | Docker ≠ Windows discover/install (multicast). |

## What works without install

- Control plane `/v1` (pages, feeder, slow, fault/jam/cover)
- SOAP acquire path (airscan / integration tests)
- Discovery Hello + Bye on shutdown
- Scripts: `discoverable`, `fd`, `aep`, `swd`, `e2e`, `remove`, `install` assist

## Gray live proof (2026-10-12, Windows 11)

Closes the open item "gray Create live re-proof". pdfcraft `--color gray`, feeder, 2 sheets, 100 dpi through the real WIA stack, switching the `jfif_gray` effect live (`POST /v1/effects`) on one daemon build:

| `jfif_gray` | Result |
|-------------|--------|
| `rgb24` (old coercion: FinalParams RGB24, BPL width×3, 3-component JPEG) | **fail**: "The feeder has no paper"; daemon sees CreateScanJob, one more GetScannerElements, no RetrieveImage |
| `gray8` | **pass**: 2 pages, 8-bpp BMP 816x1144, RetrieveImage ×2 |
| `gray8_rgb_stride` | **pass**: 8 bpp, ×2 |
| `gray8_tiff` | **fail**, like `rgb24` |

Root cause: Create answered a Grayscale8 ticket with ColorProcessing RGB24 (Override); wsdscdrv rejects the mismatch and gives up. Fix: `gray8` is now the default (Grayscale8, BytesPerLine = width, genuine 1-component JPEG); `rgb24` stays as an opt-in legacy mode. Published capabilities unchanged. Details: [defects.md](defects.md).

## Live Connect SOAP (2026-10-12)

With fakescan native `--advertise 192.168.1.70:3920`, device already Associated:

```text
Subscribe … destinations=5
Subscribe … destinations=0 (×4)
GSE sca:ScannerDescription → Ticket → Configuration → Status (Idle)
ValidateScanTicket ×N  (ValidationInfo + ImageInformation)
→ WIA Connect OK (items=1)
```

Acquire (glass / ShowTransfer JPEG GUID `{B96B3CAE-0728-11D3-9D7B-0000F81EF32E}`):

```text
Connect OK (~15–22ms)
ValidateScanTicket … CreateScanJob (Platen, jfif, dpi 200 → 1700×2200)
GSE ScannerStatus → Processing
RetrieveImage → MTOM image/jpeg ~203KB (SOF dims match ImageInformation)
  ~30s quiet (no further ScannerService SOAP)
CancelJob (JobId only) → COM 0x80004005 / pdfcraft Unspecified
```

### RetrieveImage request shape (live wsdscdrv)

Captured from `-v` (`info!` body dump) → `D:\temp\fakescan-retrieve-request.xml`:

- NS: `sca` = `http://schemas.microsoft.com/windows/2006/08/wdp/scan` (not 2006/01).
- Body: `JobId` + `JobToken` + **`DocumentDescription`/`DocumentName`** = `Scanned image file for the WSD Scan Driver`.
- Header: `To` ScannerService, `Action` …/RetrieveImage, `ReplyTo` anonymous, `From` client UUID.
- **CancelJob** ~30s after MTOM ok: `JobId` only. Cancel is now **idempotent** (no 500 if job already completed).

### MTOM packaging deltas tried (still CancelJob)

`mtom_response` + RetrieveImage branch in `crates/fakescan/src/wsd/mod.rs` vs [MS RetrieveImageResponse sample](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/retrieveimageresponse). Live dump: `D:\temp\fakescan-retrieve.mtom`.

| Delta | Result |
|-------|--------|
| Bare UUID boundary (no `uuid:` prefix); `@uuid` Content-IDs | still CancelJob |
| `MIME-Version: 1.0`; `xmlns:xop` on Envelope (not Include) | still CancelJob |
| Outer CT `startinfo=` (MS) + `start-info=` (RFC); type-before-charset on root part | still CancelJob |
| Root part CTE `8bit`; image `Content-Type: image/jpeg;`; SOAP Infoset CRLF | still CancelJob |
| Action `…/RetrieveImage` (MS sample) vs `…/RetrieveImageResponse` | same ~30s CancelJob |
| Explicit `Content-Length` on MTOM response | same ~30s CancelJob |

Dumped MTOM checks clean: complete JFIF (`FFD8`…`FFD9`), SOF 1700×2200, no boundary collision inside JPEG, closing `--boundary--` present.

### JPEG COM / format A/B (live, 2026-10-11 evening)

Setup: fakescan rebuilt (`/v1/jpeg-com` in binary) + restarted on `192.168.1.70:3920`, `brother-wsd`, pattern `wedges`, glass, dpi 200, pages=1, **`jpeg_com=false`**. WIA Connect Fake **yes** (~21ms); device `Fake WSD scanner (lite caps)`. No MTOM packaging changes.

| Case | Client | Result | Elapsed | CancelJob? | Output |
|------|--------|--------|---------|------------|--------|
| **A** `jpeg_com=false` + JPEG GUID | WIA.Transfer | `0x80004005` | **~30.1s** | **yes** (`active_jobs` 1→0 @~30s) | none |
| **A′** pdfcraft `--format jpeg` `--device` lite caps | pdfcraft-cli | Unspecified | **~149ms** | no | none (`active_jobs=1`) |
| **B** pdfcraft `--format bmp` | pdfcraft-cli | Unspecified | **~90ms** | no | none (`active_jobs=1`) |
| **B′** BMP GUID (`dib`) | WIA.Transfer | `0x80004005` | **~34ms** | no | none (`active_jobs=1`) |
| **C** pdfcraft `--format tiff` | pdfcraft-cli | Unspecified | **~92ms** | no | none (`active_jobs=1`) |
| **C′** TIFF GUID | WIA.Transfer | `0x80004005` | **~28ms** | no | none (`active_jobs=1`) |

Earlier control (same night): `jpeg_com=true` + JPEG Transfer also CancelJob **~30.1s** — COM markers are **not** the accept gate.

**Verdict:** With `jpeg_com=false`, JPEG still takes RetrieveImage → **~30s CancelJob**; BMP/TIFF (and all pdfcraft formats) fail **early** (~28–150ms) after CreateScanJob, no CancelJob. No output files. Do not churn `mtom_response` further on this evidence.

**Encode follow-up (code, 2026-10-11):** pattern `wia_plain` + `EncodeOpts::WIA_PLAIN` — baseline SOF0, no COM/APP1, no QR/banner, Q≈80 (`POST /v1/pattern` `{"id":"wia_plain"}`). Unit tests cover marker absence.

### `wia_plain` live JPEG A/B (2026-10-11 night)

Setup: fakescan rebuilt with `wia_plain` / `EncodeOpts::WIA_PLAIN`, restarted on `192.168.1.70:3920`, `brother-wsd` `…000f2`. `POST /v1/pattern` `{"id":"wia_plain"}` → **200**; `/v1/status` `image_pattern=wia_plain` (`jpeg_com=false`). Device `Fake WSD scanner (lite caps)`. No MTOM packaging changes.

| Case | Client | Result | Elapsed | CancelJob? | Output |
|------|--------|--------|---------|------------|--------|
| **D** `wia_plain` + JPEG GUID | WIA.Transfer / ShowTransfer | `0x80004005` | **~30.08s** (repeat **~30.06s**) | **yes** (`active_jobs` 1→0 @~30.1s) | none |
| **D′** pdfcraft `--format jpeg` `--device` lite caps | pdfcraft-cli | Unspecified | **~144ms** | no | none (`active_jobs=1`) |

**Verdict:** Minimal encode (`wia_plain`) does **not** clear the accept gate — same RetrieveImage → **~30s CancelJob** as wedges / `jpeg_com=false`. Synthetic COM/APP1/QR/banner are ruled out as the sole reject cause.

### Brother vs fakescan JPEG bitstream (2026-10-11 night)

Samples (same host). Brother via WIA.Transfer JPEG GUID `{B96B3CAE-0728-11D3-9D7B-0000F81EF32E}` (WIA may re-encode). Fake from `wia_plain` encode (matches live RetrieveImage byte length @200).

| Path | Notes |
|------|-------|
| `D:\temp\brother-wia.jpg` | Brother MFC-9330CDW WSD, ShowTransfer OK ~9.9s, **33620 B**, 100 dpi, SOF 816×1144 |
| `D:\temp\fakescan-wia-100dpi.jpg` | `wia_plain` after reshape @100, **28811 B**, SOF 850×1100 |
| `D:\temp\fakescan-wia.jpg` | `wia_plain` after reshape @200, **86316 B**, SOF 1700×2200 (= live `image_bytes`) |
| Prior MTOM extract | Pre-fix: 189620 B @200, 4:4:4, dens 1×1, no DRI |

| Field | Brother | fakescan **pre-fix** | fakescan **after reshape** |
|-------|---------|------------------------|----------------------------|
| Marker order | APP0→DQT→DRI→SOF0→DHT→SOS | APP0→SOF→DQT×2→DHT×4→(DRI)→SOS | **APP0→DQT→DRI→SOF0→DHT→SOS** |
| DQT / DHT | 1 packed DQT (2 tbl); 1 packed DHT (4) | split | **packed** |
| APP0/JFIF | v1.2 units=1 dens=dpi | units=0 dens=1×1 | **units=1 dens=dpi** |
| SOF0 sampling | 4:2:0 ids 1/2/3 | 4:4:4 | **4:2:0 ids 1/2/3** |
| DRI | MCU row (51 @816) | none | MCU row (54 @850 / 107 @1700) |
| Dims @100 | 816×1144 (crop) | — | 850×1100 (letter) — content delta |

ImageInformation (fakescan Validate/Create): `PixelsPerLine`/`NumberOfLines` from `page_pixels(dpi)` (letter); SOF dims match. Format `jfif`. **No `mtom_response` edits.**

**Code:** `encode_jpeg` — `jpeg-encoder` `F_2_2` + dpi APP0 + DRI MCU row + 1-based component ids, then `reshape_jpeg_brother_layout` (pack DQT/DHT, reorder). Tests: `jfif_encode_matches_brother_structure`, `reshape_jpeg_packs_dqt_dht_and_reorders`.

#### Live Fake WIA.Transfer after reshape (**E**)

| Case | Client | Result | Elapsed | SOAP | Output |
|------|--------|--------|---------|------|--------|
| **E** reshape + JPEG GUID | WIA.ShowTransfer | Unspecified / `0x80004005` | **~30.2–34.4s** | CreateScanJob jfif dpi=200 → **RetrieveImage MTOM `image_bytes=86316`** → ~30s quiet → fail (CancelJob path) | none |

**Verdict:** Brother-matching JFIF **segment order + packed tables** (plus prior 4:2:0 / APP0 dens / DRI / `wia_plain`) does **not** clear the accept gate.

**Next hypothesis:** ticket / job completion (`ImageInformation`, `DocumentFinalParameters`, ScannerStatus after RetrieveImage), Brother-like crop size, or a non-header accept check — not more MIME or marker-order churn.

### ImageInformation `BytesPerLine` for JFIF (**F**, 2026-10-11 night)

**Hypothesis (strongest of remaining):** after JPEG decode, wsdscdrv checks Create/Validate `ImageInformation` against the attachment. MS [BytesPerLine](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/bytesperline) remarks: valid only for **uncompressed** formats; if Format is compressed, return **0**. fakescan was returning `width×3` (RGB24 stride) for `jfif`.

(Schema has no BitsPerPixel / Compression / ColorSpace children — those names are not WSD elements. The concrete delta is `BytesPerLine`.)

**Code (no MTOM churn):** `image_gen::bytes_per_line` / `format_is_compressed`; ValidateScanTicket + CreateScanJob use it. Unit + `airscan_platen_flow` assert `<wscn:BytesPerLine>0</wscn:BytesPerLine>` for jfif. Uncompressed `dib` / `tiff-single-uncompressed` still get color stride.

**Live SOAP (binary up, `brother-wsd`):** CreateScanJob jfif @100 dpi → **`BytesPerLine=0`**, `PixelsPerLine=850`. Pattern `wia_plain`, `jpeg_com=false`.

#### Live Fake WIA.Transfer after BPL=0 (**F**)

| Case | Client | Result | Elapsed | Notes |
|------|--------|--------|---------|-------|
| **F** BPL=0 + JPEG GUID | WIA.Transfer / ShowTransfer | **blocked @ Connect** | ~12–20ms | Person Add restored Fake (`FakescanInWia=True`, PnP OK). Connect → **`0x80210001`**. Trace: Subscribe + GSE + ValidateScanTicket **succeed**; COM still fails; **no** CreateScanJob / RetrieveImage. Brother Connect OK. ShowAcquireImage same `0x80210001` (~12s). |

**Packing reconfirm (this session):** `reshape_jpeg_brother_layout` + tests `jfif_encode_matches_brother_structure` / `reshape_jpeg_packs_dqt_dht_and_reorders` **pass**. Before→after markers: `D8,E0,C0,DB×2,C4×4,DD,DA` → `D8,E0,DB,DD,C0,C4,DA`. fakescan Hidden on `192.168.1.70:3920` `…000f2`; fakeprint `:3921` not killed. No `mtom_response` churn.

**Verdict:** BPL=0 is live on the wire (SOAP-proven). **WIA.Transfer accept for F still unknown** — re-Add got past uninstall, but Connect COM fails despite healthy Subscribe/Validate. Do **not** churn MTOM / JFIF packing on this evidence. Next: clear Connect `0x80210001` (elevated stisvc / remove+re-Add with fakescan **not** restarted mid-session), then **F Transfer**; if still CancelJob ~30s → Gray8 / 816×1144 / Brother RetrieveImage capture.

Fixes that moved the needle earlier (Connect path): envelope `xmlns:sca`; ColorEntry `BlackAndWhite1`; `ScalingRangeSupported`; Validate `ImageInformation`; Status `Processing` while `active_jobs>0`; fuller `DocumentFinalParameters`.

## Current gate (2026-10-12 late night — autoduck ticket A/B)

**Brother baseline (same host, WIA Automation):** Connect OK `items=1`, child name=`Scan`, **props=32**. Not separate Flatbed/Feeder bags. Historical Fake Connect OK also reported `items=1` — match that shape, not empty named stubs.

**Empty Flatbed dump (letter-ticket install):** Fake root ~39 props; **Flatbed props=0**, **Feeder props=0**; `bed=None`; glass formats empty. Children exist by name only.

**glass-only / jfif-only A/B:** `glass-wsd` (ADF omitted) still left Flatbed props=0. `glass-jfif` re-Add tipped into sticky offline (`0x80210005`) — separate mode; rebind required.

**Autoduck ticket A/B (negative):** re-Add with `DocumentSizeAutoDetect` ticket + Scaling 50–200 still left **Flatbed/Feeder props=0**, root 39 props, COM Connect FAIL. Autoduck alone is not the empty-item fix.

**Brother live GSE** (`http://192.168.1.71/…/ScannerService` → `D:\temp\brother-gse-config.xml`): booleans **0/1**; ContentType **Auto only**; CompressionQuality **100–100**; Scaling **100–100**; Brightness/Contrast supported **1**; formats `tiff-single-uncompressed` then `jfif` (**no dib**); platen max **8500×11700**; min **556**; color order BW→Gray→RGB; DefaultScanTicket is a **full** document (InputMediaSize 8500×11700, Exposure, Scaling, Rotation, ScanRegion, dpi 300). Brother WIA Automation: `items=1` name=`Scan` props=32.

**Brother-shaped Config/Ticket — empty item cleared (2026-10-12):** after person Add of `D:\temp\fakescan-target-brother-shape\release\fakescan.exe` on `…000f2` / `brother-wsd` (tiff+jfif, 0/1 DeviceSettings, full DefaultScanTicket with ScanRegion/Exposure, platen max 8500×11700):

| Check | Result |
|-------|--------|
| WIA Automation Connect | **OK** `items=1` name=`Scan` **props=32** (matches Brother) |
| pdfcraft dump | root 39; **Flatbed props=44**; **Feeder props=51**; glass formats bmp/jpeg/tiff; dpi 100/200/300 |
| pdfcraft glass acquire | **pass @ RGB** — see Results table. Earlier “partial” was accept-gate / gray ticket, not TYMED. |

**Rebind notes:** stale AEP Unpair AccessDenied (even elevated — pairing permission, not UAC). `stisvc` + DAS already tried for sticky Connect and failed. **fdPHost** was never bounced for the ghost AEP; `rebind` now does fdPHost then DAS on AccessDenied (`bounce-fd`; never dasHost). `Test-FakescanLooksLike` matches `DAFWSD…000fN` only (not fakeprint IPP). Helper `unpair` / `bounce-fd` added.

**Subscribe is not missing on Add:** Windows sends Subscribe **destinations=5** then destinations=0 ×4, then GSE Description → DefaultScanTicket → Configuration → Status. Later Connects that only do Status + Validate×2 and fail `0x80210001` are a different failure from empty property bags.

**Failure modes** (do not conflate):

| Mode | Symptom | What does *not* fix it | Recovery |
|------|---------|------------------------|----------|
| **Empty installed item** (**this gate**) | Fake listed; Flatbed child exists by name; property reads `0x80070057`; root has no bed size; pdfcraft stops before Download / CreateScanJob | Cable / “reconnect” of a healthy device (reuses the installed item; does **not** rebuild empty properties) | Person **re-Add** (remove Image + Add while fakescan stays up). One re-Add is required once. |
| **Sticky offline** | After fakescan **restart**: Connect `0x80210005`, PnP often OK, **no** `/ScannerService` SOAP | `stisvc` **and** DeviceAssociationService (tried); hoping Hello/Bye clears it | Elevated remove + Disconnect + person re-Add (`Repair-FakescanScanner` / `fakescan.ps1 rebind`). Keep process up during Add. |
| **Ghost AEP** | PnP gone, AEP `IsPaired=True`, Add list empty, Unpair AccessDenied | bouncing `stisvc`; bouncing **dasHost** (UDP 3702) | `bounce-fd` (fdPHost then DAS) after Unpair; PairTool `/disassociate` if present; last resort new `--device-uuid` |
| **Reconnect (healthy MFP)** | Device was healthy; Windows reopens the same Imaging node | — | Reuses the installed item. Useful when properties were already populated; **not** a substitute for re-Add on an empty Flatbed. |

| Client | Result |
|--------|--------|
| `WIA.DeviceManager.Connect` | Generic WIA error in ~60ms. SOAP: ScannerStatus + ValidateScanTicket ×2 (`valid=true`, jfif, 200 dpi, 1700×2200, `BytesPerLine=0`). No CreateScanJob. Validate alone does not mean the WIA item has bed size. |
| pdfcraft `scanners()` | Both Brother and Fake open. Fake port is the `…000f2` ScannerService URN. |
| pdfcraft glass acquire | WIA 1.0 select `0x80210001` (logged, not fatal). Flatbed item reads return `0x80070057`. No bed size → empty-item message (re-Add while on). No Download, no CreateScanJob. ~0.46s. Not sticky-offline (`0x80210005`). |

The scanner is advertising a letter platen on SOAP. The installed Imaging children are empty (no extent / format / dpi). Re-Add with letter `InputMediaSize` already done on PID 71568 — empty Flatbed remains. Earlier RetrieveImage → ~30s CancelJob was from a previous install when Connect still populated items.

## Remaining gap (P0 for full E2E)

1. ~~**Empty installed item**~~ — **cleared** by Brother-shaped ScannerConfiguration + DefaultScanTicket (see above).
2. ~~**Glass acquire `0x100c = 128`**~~ — **not a fakescan bug.** Prop is `WIA_IPA_TYMED` / `TYMED_CALLBACK` (128). Vista+ WIA ([compatibility layer](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/wia-compatibility-layer-data-transfers-overview)) stops distinguishing CALLBACK vs FILE for modern drivers (`STI_VERSION_3`); only `TYMED_FILE` / `TYMED_MULTIPAGE_FILE` remain. Inbox WSD still advertises callback and rejects the write (`0x80070057`). pdfcraft tries callback, then falls back to file + `IWiaTransfer::Download` (`crates/scan/src/com.rs`). Remaining live fail after that is **`0x80004005` / RetrieveImage CancelJob**.
3. ~~**RetrieveImage accept (P0)**~~ — **cleared 2026-10-12 night.** Root cause: MTOM SOAP envelope used XOP xmlns `http://www.w3.org/2003/12/xop/include`; Brother live uses **`2004/08`**. With `2004/08`, WIA.Transfer JPEG OK @100/200/300 (~30–50ms), no CancelJob. Confirmed by replaying Brother JPEG bytes (still CancelJob under 2003/12; OK under 2004/08). Secondary: ImageInformation/SOF use Brother optical crop (`optical_page_pixels`: 816×1144 / 1648×2289 / 2480×3440) while WIA extent stays full letter — matches Brother Transfer output. Lab: `FAKESCAN_RETRIEVE_FILE` overrides RetrieveImage body.
4. ~~**Rebind / ghost AEP**~~ — **cleared 2026-10-12:** never `Restart-Service -Force` on DeviceAssociationService (StopPending wedges Add/AEP). Bounce **fdPHost** only; Start DAS if Stopped. Unstick StopPending by killing **lone** DAS svchost then Start. Keep **`--probe-listen`**.
5. Docker is for SOAP/CI; Windows WIA E2E stays on native `--advertise`.

### Re-install path (empty item or sticky offline)

```text
# keep fakescan.exe running with the same --advertise / --device-uuid
.\scripts\fakescan.ps1 elevated   # then remove, or:
pnputil /remove-device "SWD\DAFWSDPROVIDER\urn:uuid:00000000-0000-4000-8000-0000000000f2\..."
.\scripts\fakescan.ps1 aep          # expect Fake WSD scanner, CanPair=True
.\scripts\fakescan.ps1 discoverable # verdict aep-pairable
.\scripts\fakescan.ps1 install -BaseUrl http://192.168.1.70:3920 -WaitSeconds 120
# person: Printers & scanners → Add device next to Fake WSD scanner
.\scripts\fakescan.ps1 wia
# -v logs should show Connect → Subscribe (DestinationResponses) → GetScannerElements
pdfcraft-cli scan --out DIR --yes --device "Fake WSD scanner" --source glass --pages 1 --dpi 100 --format png
```

## FD / PnP-X Associate investigation (2026-10-12)

### What was checked

**Transfer Get metadata (live SOAP GET on `/WebServices/Device`):**

All required PnP-X fields are present and tested:

| Field | Value | Where |
|-------|-------|-------|
| `pnpx:DeviceCategory` | `Scanners` | `ThisModel` section |
| `df:DeviceCategory` | `Imaging.Scanner` | `ThisModel` section |
| `pnpx:HardwareId` | `VEN_FAKE&DEV_WSDSCAN` | `Hosted` section |
| `pnpx:CompatibleId` | `…/2006/08/wdp/scan/ScannerServiceType` | `Hosted` section |

Unit test `transfer_get_includes_pnpx_fields` (in `wsd_scan_flow.rs`) asserts all four. No `wsd:Scopes` change needed — Brother omits Scopes and still appears in FD.

**Function Discovery (COM) live test results:**

| Query | Result |
|-------|--------|
| Baseline `Get-FakescanFunctionDiscovery` with device already installed | `FakescanPresent=True, Installable=true` but for a **stale loopback instance** (UUID `e11e`) from a prior run; the installed `f2` instance does not appear as Installable (already installed) |
| `Get-FakescanFunctionDiscovery` repeated 30 min later | `FakescanPresent=False` — stale `e11e` evicted from FD cache |
| `FdAssoc.AssociateFakescan` (both categories) | `0 instances found` — live query finds only devices currently broadcasting Hellos; stale cache entries don't satisfy live re-discovery |

**Verdict on FD listing:**
- Transfer Get metadata is correct; `pnpx:DeviceCategory` and HardwareId/CompatibleId are already present.
- An **uninstalled** fakescan (no `SWD\DAFWSDPROVIDER\…f2` node) that is actively sending Hellos **will** appear in FD with `Installable=true`. The installed device (`f2`) is not shown as Installable because Windows considers it already associated.
- FD results are volatile (WSD provider cache); `aep-pairable` from WinRT is more reliable as a pre-install signal.
- `wsd:Scopes` in Hello are **not** the cause — not added; no gap vs Brother.

### IPNPXDeviceAssociation::Associate helper

**Added (2026-10-12):**

- `Fakescan.FunctionDiscovery.cs` — new COM interop: `IPNPXAssociationCallback`, `AssocCallback`, `IPNPXDeviceAssociation`, `AssocResult`, `AssocNotify`, `FdAssoc.AssociateFakescan()`.
- `Fakescan.psm1` — `Invoke-FakescanPnpxAssociate` function (exported).
- `fakescan.ps1` — `associate` command.

**Interface details (`pnpxassoc.h`):**
- `SID_PNPXDeviceAssociation = {656A3BB3-ECC0-43FD-8477-4AE0404A96CD}` (same GUID as PnpX property format)
- `IID_IPNPXDeviceAssociation = {AEE3D3B8-2BA0-4B5C-ACF5-FDAE6C86AAE4}`
- Obtained via `IFunctionInstance::QueryService(SID, IID)` on a live FD instance.
- `Associate(hwnd, callback)` — starts the OS install flow; callback (`IPNPXAssociationCallback::OnAssociated`) fires on completion.
- **Requires elevation** — returns `0x80070005` (`E_ACCESSDENIED`) without admin rights.

**Live test (2026-10-12, device already installed):**
- `Invoke-FakescanPnpxAssociate` correctly returned `0 instances` (no uninstalled fakescan in live FD) and reported `FakescanInWiaAfter=True` (already installed via prior person Add).
- Non-elevated warning fires correctly; `Elevated=False`.

**Unproven path (device not yet installed):**
Associate can only fire when FD has a live uninstalled instance. Use this sequence:

```powershell
# 1. fakescan running with --advertise <LAN-IP> --probe-listen (LAN Hello visible)
.\scripts\fakescan.ps1 discoverable    # confirm fd-installable or aep-pairable

# 2. If device already installed, remove it first (elevated):
.\scripts\fakescan.ps1 elevated
# in elevated shell:
Remove-FakescanScanner                 # removes Image + SoftwareDevice nodes (not Brother)

# 3. From same elevated shell, re-query FD and try Associate:
.\scripts\fakescan.ps1 associate
# expects: FD finds Fake WSD scanner with Installable=true, Associate call fires
# then: .\scripts\fakescan.ps1 wia  -> FakescanInWia=True (if it works)

# If Associate fails or FD shows empty:
.\scripts\fakescan.ps1 install -BaseUrl http://192.168.1.70:3920 -WaitSeconds 120
# (person Add device fallback)
```

**Remaining gap:** Whether `Associate` actually creates the Imaging node (without person click) is **unproven** on this lab. The call chain is correct; the COM interop is wired; the test requires an uninstalled-but-FD-visible device and an elevated shell. Log `Associate hr` from `$r.Results` — `0x00000000` + callback `hr=0` = success; `0x80004005`/`0x80070005` = still needs person.

**ensure integration (2026-10-12):** `Start-FakescanEnsure` → `Complete-FakescanPnpIfNeeded`: if Fake is already in WIA, marks `added.pnp=true` without Settings; if elevated and not in WIA, tries `Invoke-FakescanPnpxAssociate` before attention. Proving Associate still needs an elevated remove+associate cycle (do not run mid-scan — sticky WIA risk).

### Associate prove run (2026-10-12)

**Verdict: blocked-on-UAC** — current agent session is not elevated; Remove-FakescanScanner and Invoke-FakescanPnpxAssociate both require admin rights that the non-interactive agent shell cannot obtain.

**Live state checked (pre-prove):**

| Check | Result |
|-------|--------|
| `GET /health` (127.0.0.1:3920) | **200 ok** |
| `GET /v1/status` | `device_uuid=000f2`, `wsd_hellos_sent=1721`, `profile=brother-wsd`, `active_jobs=0` |
| `Get-FakescanWiaDevices` | `FakescanInWia=True`, `FakescanPnpOnly=False`, `OnlineCapable=True`, `WiaConnectable=True`, `StickyPresenceHint=False` |
| WinRT AEP | `Fake WSD scanner (lite caps)` — `IsPaired=True`, `CanPair=True` (no uninstalled pairable candidate, so `CanPairFakescan=False`) |
| FD WSD provider | `FakescanPresent=False` — installed device (`f2`) does not appear as Installable (as expected) |
| `Remove-FakescanScanner -WhatIf` | Would remove **3 nodes**: Image (`SCANNERSERVICE`), SoftwareDevice (`f2`), parent SoftwareDevice (`f2/URI:…`) — **Brother untouched** |

**Live elevated prove (2026-10-12):** `prove-associate.ps1` removed 3 Fake PnP nodes, then FD stayed empty for the full wait (`FunctionDiscoveryHit=False`, `AssocInstances=0`). Associate never fired. Afterward WIA/PnP still showed `Fake WSD scanner (lite caps)` (`OnlineCapable=True`) — residual or immediate rediscovery as installed, not Associate. First script build wrongly printed `RESULT: PROVED` from `FakescanInWia` alone; verdict now requires Associate to have fired on an FD instance, and Step 4 waits for WIA/PnP clear.

**Later same day (post-clear):** `FakescanInWia=False` / `FakescanInPnp=False` (clean slate), discoverable `on-wire-discoverable`. Restarted with `--probe-listen`: fakescan binds `:3702` (alongside svchost), `wsd_probes_answered` rises (Probes answered), Hellos advance — but FD still has **0** fakescan hits (Brother on `.71` Installable only). Same-host advertise stays FD-invisible for Associate. `ensure` now passes `--probe-listen` by default (registry `probe_listen=false` to opt out). Associate remains **BLOCKED-ON-FD** on this host — use person Add (`fakescan.ps1 install`) or prove from another LAN machine.

**One-shot prove script:** `scripts/prove-associate.ps1` — elevated; `RESULT: PROVED` only when Associate fired and WIA is present afterward.

```powershell
.\scripts\fakescan.ps1 elevated
# then in the elevated window:
.\scripts\prove-associate.ps1 -BaseUrl http://192.168.1.70:3920
```

**Expected HRs (when FD has an Installable hit):**

| HR | Meaning |
|----|---------|
| `QueryServiceHr=0x00000000` | `IFunctionInstance::QueryService` got `IPNPXDeviceAssociation` — interface present |
| `QueryServiceHr=0x80004002` | `E_NOINTERFACE` — FD instance does not expose `IPNPXDeviceAssociation` |
| `AssociateHr=0x00000000` + `CallbackFired=True` + `Succeeded=True` | **proved** — OS install flow completed without person click |
| `AssociateHr=0x00000000` + `CallbackFired=False` | associate started, callback never fired (timeout); check WIA manually |
| `AssociateHr=0x80004005` (`E_FAIL`) | OS rejected non-interactive Add (likely needs person); Associate path blocked |
| `AssociateHr=0x80070005` (`E_ACCESSDENIED`) | not elevated (should not happen with `-RunAsAdministrator`) |
| `AssociateHr=0x80070490` | device not found in PnP; FD instance stale |
| `AssociateHr=0x8007139F` (`E_ALREADY_EXISTS`) | device was already installed; WIA should be True |
| 0 FD instances | **BLOCKED-ON-FD** — not proved, even if `FakescanInWia=True` |

**Sticky WIA risk (documented):**
- Remove may leave or immediately restore Imaging + SoftwareDevice nodes; while they remain, FD does not expose Installable and Associate cannot fire.
- fakescan keeps running with the same `--device-uuid=000f2 --advertise 192.168.1.70 --port 3920`. Do **not** restart it between remove and prove.
- If Associate fails or FD finds 0 instances, recovery is: `.\scripts\fakescan.ps1 install -BaseUrl http://192.168.1.70:3920 -WaitSeconds 120` (person Add device).
- fakeprint (`:3921`) is never touched by `Remove-FakescanScanner` (name/UUID filter is Fake-only).

## Matrix (from this run)

| # | Status |
|---|--------|
| 1 usb vs wsd list | blocked — `scan devices` trips on offline Brother USB; acquire by name |
| 2 optical null on WSD | ready in profile; unverified live for fakescan |
| 3 refuse unknown dpi/format | enabled + tested |
| 4–7 sticky / pages / slow cancel | control plane ready |
| 8 duplex reverse fixtures | enabled (`side`/`order` + `scenario duplex`) |
| 9 page markers / QR / progression | enabled ([patterns.md](patterns.md)) |
| 10 jam / cover | enabled |
| 11 capability UI profiles | ready |
| 12 dump on WSD fakescan | blocked — dump also opens all devices |
| USD | deferred (custom INF belongs here, not silent Add) |
