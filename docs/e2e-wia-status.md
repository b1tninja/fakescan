# WIA E2E live status (WSD path)

**Lab log** (dated run notes). Operator checklist: [runbook-fake-scanner.md](runbook-fake-scanner.md). Scripts / dead ends: [windows.md](windows.md).

Date: 2026-10-11 (local). Host: Windows, LAN `192.168.1.70`.

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
| Function Discovery WSD query (COM) | **partial** | Brother `Installable=true` works. fakescan often **absent** from FD even when AEP lists it. |
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
| WIA Connect Fake (live) | **pass** (2026-10-12) | COM Connect OK after `xmlns:sca`, `ScalingRangeSupported`, `BlackAndWhite1`, Validate `ImageInformation`. Was `0x80210005` / then `0x80210001`. |
| Glass acquire (live) | **partial** | Connect OK → CreateScanJob → **RetrieveImage MTOM JFIF** → **~30s** → CancelJob → COM **0x80004005**. JFIF now Brother-shaped (`D8,E0,DB,DD,C0,C4,DA`, packed DQT/DHT, 4:2:0, dens=dpi, DRI); live **E** still CancelJob @~30s (`image_bytes=86316`). Next: ImageInformation / final params / size — not more markers. |
| `Remove-FakescanScanner` | **pass** (elevated) | Non-elevated: Access denied. Elevated pnputil cleared Image + parent. Brother untouched. |
| Unit/integration (`cargo test -p fakescan`) | **pass** | |
| `docker compose build` | **pass** | Docker ≠ Windows discover/install (multicast). |

## What works without install

- Control plane `/v1` (pages, feeder, slow, fault/jam/cover)
- SOAP acquire path (airscan / integration tests)
- Discovery Hello + Bye on shutdown
- Scripts: `discoverable`, `fd`, `aep`, `swd`, `e2e`, `remove`, `install` assist

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

Fixes that moved the needle earlier (Connect path): envelope `xmlns:sca`; ColorEntry `BlackAndWhite1`; `ScalingRangeSupported`; Validate `ImageInformation`; Status `Processing` while `active_jobs>0`; fuller `DocumentFinalParameters`.

## Remaining gap (P0 for full E2E)

1. **RetrieveImage accept** — MTOM OK; `jpeg_com=false`, `wia_plain`, **and Brother JFIF reshape** still CancelJob ~30s (WIA.Transfer JPEG). Next: ticket / status / size vs attachment — not more MIME/marker A/B.
2. **Acquire by name** — `--device` lite caps / `"Fake WSD scanner"` reaches CreateScanJob but fails ~90–150ms; empty `--device` hits Brother. Do not use `scan devices` while Brother USB is offline.
3. Sticky offline after fakescan restart / long CancelJob storms still needs elevated **remove + Disconnect + re-Add** if it returns (`Repair-FakescanScanner` / `fakescan.ps1 rebind`). Keep `--advertise` on LAN IP (loopback-only fakescan is not Add-able).
4. Docker is for SOAP/CI; Windows WIA E2E stays on native `--advertise`.

### Re-install path (after drop / sticky offline)

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
