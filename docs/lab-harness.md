# Lab harness (Python)

The WIA / pdfcraft scan harness lives **in fakescan**, not in pdfcraft. Print labs use the sibling **[fakeprint](../../fakeprint)** ([siblings.md](siblings.md)) — capture-assert on `:3921`, not this loopback store.

| Layer | Where | Role |
|-------|--------|------|
| Server | Rust `fakescan` binary | WSD + control plane + hard loopback store |
| Harness | `python/` package (`pip install -e ./python`) | Arm `/v1/*`, SOAP acquire, digest/byte expect |
| Windows install | `scripts/Fakescan.psm1` | Discovery, Add device, helper/elevated Imaging remove, rebind (person required; SOAP matrix unelevated) |
| App under test | pdfcraft-cli (optional) | Real WIA path — **not** a fakescan dependency |

pdfcraft must not take a fakescan (or this package) dependency for normal builds.

## Soft vs hard ground truth

| Mode | How | Expect |
|------|-----|--------|
| Soft | Synthetic patterns + JPEG COM / QR | Markers / visual cues |
| **Hard loopback** | `POST /v1/loopback` with uploaded JFIF bytes | Acquired bytes **==** original |

Hard loopback is the “echo this page back” store: upload → CreateScanJob / WIA acquire → compare.

Modes are mutually exclusive: `POST /v1/loopback` clears pattern/side/scenario; `POST /v1/pages` (with or without side), `pattern`, or `scenario` clears the loopback store. Effects do **not** alter loopback JFIF bytes (exact echo).

```bash
# terminal 1
cargo run -p fakescan -- --profiles profiles --profile brother-wsd

# terminal 2
pip install -e "./python[dev]"
# any local JFIGs — CLI resets before arming
fakescan-lab loopback-soap python/fixtures/a.jpg python/fixtures/b.jpg
# or via PowerShell CLI:
.\scripts\fakescan.ps1 lab loopback-soap python/fixtures/a.jpg python/fixtures/b.jpg
cd python && pytest -m live
# full local matrix (starts fakescan; no Imaging install):
cd python && pytest tests/test_local_matrix.py
```


## Control plane additions

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/v1/loopback` | count + digests (`len:first8hex`) |
| `POST` | `/v1/loopback` | `{"pages":["base64…",…]}` — replace store; empty clears |
| `DELETE` | `/v1/loopback` | clear store |

`GET /v1/status` also exposes `loopback_pages` and `loopback_digests`.

JFIF RetrieveImage returns **exact** uploaded bytes. Multi-page Formats are refused for loopback — use `jfif` for byte-identical compare.

Read-only views of the same process. These routes are in this tree and show up after the next start. An older binary still on `:3920` answers 404 for `/v1/jobs`, `/v1/activity`, and `/v1/log`.

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/v1/jobs` | `{jobs:[{id, state, format, source, dpi, color, pages_left, page_index, width_px, height_px}]}`. `state` is `Pending`, `Processing`, or `Completed`. Job tokens omitted. |
| `GET` | `/v1/activity` | `?after=SEQ` optional. `{seq, events:[{seq, kind, peer, action, outcome, ms, detail}]}`. `kind` is `wsd`, `job`, or `ctl`. Ring of the last 64; `after` returns only newer events. `GET /v1/trace` is this JSON. |
| `GET` | `/v1/log` | `?after=SEQ` optional. The same events as `text/plain`, one line each: `{seq} {kind} {action} {outcome} {ms}ms {detail}`. |

`POST /v1/ctl` accepts cmds `jobs` and `activity` (`activity` args `after` optional).

```powershell
.\scripts\fakescan.ps1 jobs
.\scripts\fakescan.ps1 activity
.\scripts\fakescan.ps1 log
.\scripts\fakescan.ps1 wia-trace
```

`wia-trace` connects only the Fake WIA device, prints the COM HRESULT and elapsed ms, and diffs `/v1/trace` (or `/v1/activity`) across that Connect. Rechecked with Fake installed as "Fake WSD scanner (lite caps)" and fakescan answering `http://192.168.1.70:3920`: Connect failed in about 26 ms with HRESULT `0x80210001` and no ScannerService call.

## Python API

```python
from fakescan import Client

with Client("http://127.0.0.1:3920") as c:
    c.reset()
    c.set_loopback_files(["front0.jpg", "front1.jpg", "front2.jpg"])
    # … WIA / pdfcraft-cli acquire …
    c.expect_loopback_files(["front0.jpg", "front1.jpg", "front2.jpg"])

# Soft / fault paths stay separate from loopback:
with Client() as c:
    c.reset()
    c.set_pages(3, side="front")   # clears any prior loopback
    c.set_fault(jam_after_n=2)
```

SOAP-only (CI, no WIA): `from fakescan.soap import acquire_jfif_pages` or `fakescan-lab loopback-soap`.

### Faults (jam / cover / empty / offline)

The SOAP column is what `pytest python/tests/test_live_faults.py -m live` checks. The pdfcraft / WIA column is what that app is written to show; a live Imaging pass is still open ([test-matrix.md](test-matrix.md) rows 10, 10b, and 10c).

| Knob | SOAP | pdfcraft / WIA (app expect) |
|------|------|------------------------------|
| `jam` / `cover_open` | `GetScannerElements` ScannerStateReason `MediaJam` / `CoverOpen` | `scan devices` → `handling_status.jam` / `cover_open`; UI banner |
| `multi_feed` | Status `MultipleFeedError` + `Stopped` | `handling_status.multiple_feed`; “The feeder took more than one sheet.” |
| `lamp_error` | Status `LampError` + `Stopped`; job ops refuse | “The scanner lamp is off.” (often `0x80210011`) |
| `busy` | Status `Processing` + reason `None`; Subscribe/Create/Retrieve refuse | “The scanner is busy…” (`0x80210006`) when WIA maps it; else offline/not-accepting |
| `temporary` | Create/Retrieve/Subscribe refuse `wscn:ServerErrorTemporaryError` | retry-later / temporary failure (WIA mapping varies) |
| `internal` | Create/Retrieve/Subscribe refuse `wscn:ServerErrorInternalError` | permanent internal failure (WIA mapping varies) |
| `offline` | Status `AttentionRequired` + Subscribe/Create/Retrieve refuse `ServerErrorNotAcceptingJobs` | Connect / acquire → offline-style sentence (often `0x80210005`) |
| `jam_after_n=N` | N pages OK, then RetrieveImage SOAP subcode `wscn:ClientErrorMediaJam` | acquire fails: “The scanner has a paper jam.” (pages already streamed stay in progress) |
| `multi_feed_after_n=N` | N pages OK, then RetrieveImage `wscn:ClientErrorMultipleFeed` + sticky `multi_feed` | mid-job double-feed sentence (pages already streamed stay in progress) |
| `offline_after_n=N` | N pages OK, then RetrieveImage `ServerErrorNotAcceptingJobs` and sticky `offline` | mid-scan drop → “stopped… stayed online” / offline; later Connect stays offline until `reset` |
| `empty_mid_job=N` | N pages OK, then SOAP subcode `wscn:ClientErrorNoImagesAvailable` | feeder empty / short stack |
| `lamp_warmup_ms` / `ms` | first RetrieveImage delayed | busy / cancel-mid-transfer labs |
| `abort_retrieve` / `abort_retrieve_after_n` / `abort_retrieve_bytes` | MTOM body starts, then stream errors (drop mid-page) | Download fail → interrupted / stayed-online sentence; pair with `bps` for progress UI |
| `POST /v1/slow` `{ "bps": N }` | MTOM body paced at N bytes/sec (8 KiB chunks; still has `Content-Length`) | pdfcraft UI mid-page `%` / `bytes_received` (WSD Download STATUS); `ms` alone only stretches indeterminate wait |

### Progress UI lab (Fake WSD, 2026-10-12)

```powershell
# ~10 s fill for a ~316 KiB optical JFIF @ 32 KiB/s
Invoke-RestMethod -Method Post -Uri http://127.0.0.1:3920/v1/ctl `
  -ContentType 'application/json' -Body '{"cmd":"slow","args":{"bps":32000}}'
# arm optical loopback, then:
#   pdfcraft GUI glass scan --color rgb   → bar should climb
#   or CLI with STATUS trace:
$env:PDFCRAFT_SCAN_STATUS_LOG = '1'
pdfcraft-cli scan --out D:\temp\out --yes --source glass --dpi 200 --color rgb `
  --format jpeg --device 'Fake WSD scanner (lite caps)'
# clear: {"cmd":"slow","args":{"bps":0}}  or reset
```

Live: throttled Transfer ~10 s vs ~130 ms unthrottled; `IWiaTransfer` STATUS fired ~40 times (`percent` 0…40 then 100, `transferred` climbing to image size). CLI has no progress token; the GUI `ProgressToken` path is what paints the bar.

**Progress accuracy (pdfcraft):** inbox WSD’s `lPercentComplete` is not of the JFIF stream (peaks ~40 at full `ulTransferredBytes`, then snaps to 100). pdfcraft ignores mid values on WSD ports, derives bar fill from transferred bytes (soft ceiling until the terminal 100), and does not use the uncompressed BMP size as `bytes_expected` for that path.

```bash
fakescan-lab fault --jam-after-n 2
# live SOAP (no WIA):
pytest python/tests/test_live_faults.py -m live
```

## Raw BMP / lossless (no JFIF)

**WIA path (documented inbox fallback):** advertise no `jfif`, return `ClientErrorDocumentFormatNotSupported` when asked for it — driver retries PNG → … → uncompressed TIFF / dib ([MS Format](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/format)).

```bash
fakescan-lab --base-url http://127.0.0.1:3920 profile wia-lossless
# pdfcraft --format bmp → CreateScanJob Format=dib (lossless wire BMP)
# (inbox may first try jfif → FormatNotSupported → png/tiff/dib; Validate soft-accepts the miss)
fakescan-lab --base-url http://127.0.0.1:3920 profile brother-wsd
# pdfcraft --format jpeg → CreateScanJob Format=jfif (Brother-shaped). --format bmp asks dib and fails on this profile.
```

**SOAP path:**

```bash
fakescan-lab --base-url http://127.0.0.1:3920 acquire-dib --out D:/temp/platen-dib.bmp --dpi 200
# restores the previous profile by default (--restore-profile '' leaves accept-dib)
```

Python: `from fakescan.soap import acquire_dib_platen`.

## Optional pdfcraft (WIA)

Needs Fake listed in Imaging / `pdfcraft-cli scan devices`. WIA usually **re-encodes** JPEG, so compare defaults to `auto` (exact, else Pillow pixels). Use `loopback-soap` for hard byte identity.

```bash
fakescan-lab loopback-wia page0.jpg page1.jpg \
  --pdfcraft-cli D:/code/pdfcraft/target/release/pdfcraft-cli.exe
# arm only (no CLI):
fakescan-lab loopback-wia page0.jpg --arm-only
```

Never add pdfcraft as a Cargo/Python dependency of fakescan — pass `--pdfcraft-cli` or PATH only in the lab.
