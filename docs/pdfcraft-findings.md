# pdfcraft findings from fakescan harness

Living notes from **pdfcraft-fakescan** (`pytest -m pdfcraft_wia`) and fakescan SOAP/local matrix runs. Each item should become a pdfcraft issue, test, or doc fix when confirmed.

## Harness fixes (pdfcraft-fakescan)

| Finding | Fix |
|---------|-----|
| `time.strftime("%f")` invalid on Windows | Use `datetime` microsecond suffix in `driver.py` run dirs |
| ValidateScanTicket soft-accepts unknown format (inbox WSD) | `test_local_matrix` documents MustHonor + Create fault path |
| CancelJob retrieve → `ClientErrorJobCancelled` not "not found" | Test assertion updated |
| E2E feeder cases on glass-only WIA tree | Skip with re-Add hint when `scan devices` reports `feeder: false` |
| `wia_lab` arm feeder without profile | Rust smoke only: `fakescan_arm_feeder` sets `brother-wsd` after reset |
| Glass acquire format fault (`jfif` / `dib` not in profile) | Rust smoke only: `fakescan_arm_glass` sets `glass-wsd` after reset |
| Fault / partial / offline / MTOM matrix | **pdfcraft-fakescan** `pytest -m pdfcraft_wia` (`pdfcraft_fakescan/matrix.py`) |

## pdfcraft fixes (WIA / Fake WSD)

| Issue | Cause | Change |
|-------|--------|--------|
| `property 3088: Invalid pointer` | `WIA_DPS_DOCUMENT_HANDLING_SELECT` (0xC10) on WIA 1.0 / WIA 2.0 root for inbox WSD | Skip WIA 1.0 `select_source` on WSD; soft-write handling select on root; soft IPS/DPS props and DPI on transfer item |
| Fake lists `port: null` in `scan devices` | Driver omits port string | Detect WSD via device name / `WIA_DIP_DEV_NAME` containing `wsd` |
| `the feeder is empty` with sheets on fakescan | WIA root omits `FEED_READY` while WS-Scan has pages | On WSD with `pages > 0`, warn and continue (transfer still fails if truly empty) |
| Stale WIA tree after `POST /v1/profile` | Windows caches GetScannerElements at Add time | Re-Add device or start fakescan on `brother-wsd` before Add; HTTP profile alone may leave glass-only tree |
| Partial pages lost on failed acquire | CLI only saved on `Ok(acquire)` | `ScanError::Incomplete` + CLI writes pages then exits 1 |
| `--pages all` on lab WSD | `IPS_PAGES=0` → one sheet; generic HRESULT after stack | Write ticket cap (`MAX_PAGES`); `explain_transfer_failure(..., pages_requested=0)` treats empty tray as normal end |
| `10b_jam_after_2` message OK, 0 files | Same partial-save gap | Fixed via `Incomplete` (manual: 2 files + jam sentence) |

## E2E matrix status (last run: 2026-10-12)

**CLI:** `D:\temp\fakescan-target-optical\debug\pdfcraft-cli.exe` (present).  
**fakescan:** `:3920` health `ok`, profile `brother-wsd`.  
**Imaging:** Fake WSD re-Added (`FakescanInWia=True`, `feeder: true` in `scan devices`).

| pytest | Result |
|--------|--------|
| fakescan `python`: `-m "not live"` | **59–61 passed**, 2 skipped (SOAP/local matrix) |
| pdfcraft-fakescan `pytest -q` | unit + registry (no WIA) |
| `-m pdfcraft_wia` | Re-run after re-Add; manual `06b_pages_all`: **3 BMPs, exit 0** (`feeder --pages all`) |

**Manual proof (`06b_pages_all`):** reset + `pages=3`, then  
`scan acquire --source feeder --pages all` → `count=3`, exit **0** (validates [pdfcraft WSD pages=all](a775bfd0-86da-4adf-bd56-a1ab3cfe090c) ticket cap + whole-stack end).

| Scenario | SOAP/local | WIA E2E (this run) | Notes / last known failure shape |
|----------|------------|--------------------|----------------------------------|
| `06_pages_n` | pass | blocked | — |
| `06b_pages_all` | pass (pages cap N/A) | **pass** (manual 2026-10-12) | pytest row should match manual; old failure: `feeder clamp? … got 1 page(s)` |
| `10_jam_preflight` | pass | blocked | — |
| `10_cover_preflight` | pass | blocked | — |
| `10d_multi_feed` | pass | blocked | — |
| `10f_lamp` | pass | blocked | — |
| `10b_jam_after_2` | pass | blocked | Expect 2 partial pages + message containing `paper jam`; re-prove after fakescan NotifyTo on mid-job jam (2026-10-12) |
| `10c_offline_after_n` | pass | blocked | Expect 1 partial page + `offline` / `interrupted` / `stayed online` / `online` in stderr |
| `10e_mtom_abort` | pass | blocked | — |
| `devices_jam_bit` | pass | blocked | — |
| `09c_loopback_wia` | pass (`test_loopback_echoes_bytes…`) | skipped (by design) | Glass `pages=1` + minimal loopback JFIF: WIA Download does not finish within 180s; not a SOAP regression |
| `08_reverse_manual` | pass | skipped (by design) | Two-pass manual duplex |

Feeder rows also skip when `scan devices` reports `feeder: false` (stale glass-only WIA tree) even if Fake is present.

## pdfcraft behavior to verify (WIA lab)

| Matrix id | Question |
|-----------|----------|
| `06b_pages_all` | Feeder `--pages all` returns full stack loaded via `/v1/pages` (regression for UI `snap_choices`) |
| `10b_jam_after_2` | Exactly 2 pages on disk then jam sentence (`after_transfer`) |
| `10c_offline_after_n` | One page then offline/interrupted wording |
| `10e_mtom_abort` | Clear interrupted message, no panic |
| `09c_loopback_wia` | Glass loopback: WIA Download timeout (matrix `skip_reason`; SOAP path OK) |

## SOAP vs WIA (row 3)

- **MustHonor=false:** unknown dpi may validate soft (fakescan maps to nearest). pdfcraft WIA may accept settings the UI lists differently.
- **MustHonor=true:** CreateScanJob should fault; pdfcraft should surface `ScanError::Failed` with a readable sentence, not raw HRESULT.

### Mid-job faults (`jam_after_n`, `offline_after_n`)

| Layer | Behavior |
|-------|----------|
| **fakescan SOAP** | After N successful RetrieveImage calls, next Retrieve faults (`ClientErrorMediaJam` / `ServerErrorNotAcceptingJobs`), sets sticky `jam` / `offline`, marks job `ScannerStopped`. |
| **NotifyTo (fixed 2026-10-12)** | On that fault, fakescan now emits `ScannerStatusConditionEvent` (jam) and `ScannerStatusSummaryEvent` (Stopped + `MediaJam` or `AttentionRequired`), same as `POST /v1/fault` — inbox `wsdscdrv` can refresh without a new GetScannerElements poll. |
| **GetScannerElements** | Still reflects sticky faults on the next poll (`ActiveConditions` / reasons); Windows may cache capabilities at **Add** time — re-Add after profile change, not for mid-job status. |
| **pdfcraft WIA acquire** | After `IWiaTransfer::Download`, re-reads `WIA_DPS_DOCUMENT_HANDLING_STATUS` on the root item and maps bit `0x20` → jam sentence via [`explain_transfer_failure`](https://github.com/storytold/pdfcraft/blob/main/crates/scan/src/lib.rs). Mid-job jam should surface if the driver sets the jam bit **or** returns HRESULT / message mapped to paper jam; SOAP fault alone is driver-internal. |
| **WIA shim** | No separate repo: inbox **WSD Scan Driver** (`wsdscdrv`) maps WS-Scan → WIA. fakescan cannot set WIA properties directly; live proof is `pdfcraft_wia` rows `10b_jam_after_2` / `10c_offline_after_n`. |

## Gaps to implement in pdfcraft (candidates)

- **CLI default `--pages 1`** vs UI feeder default “All pages” (`0`) — document or align.
- **busy** fault (`10f` busy knob): add `pdfcraft_e2e_id` when WIA maps `Processing` to busy sentence.
- **empty_mid_job** (`10b`): matrix row `10b_empty_mid` not yet in registry.
- **Progress UI** with fakescan `bps` throttle — manual GUI lab ([lab-harness.md](lab-harness.md)).
- **Image fidelity E2E:** `AcquireOk.fidelity` and `compare_acquired_to_originals` exist but `verify_acquire` does not call them; wire loopback + scenario `qr`/`order` into `pdfcraft_wia` or a new `pdfcraft_fidelity` marker.
- **Automation / MCP:** only `scan_devices` today — no headless `scan_acquire` tool for agents (CLI already has full acquire JSON).

## How to refresh

Use the same `pdfcraft-cli` you built (if `CARGO_TARGET_DIR` is set, point `PDFCRAFT_CLI` at that tree’s `debug\pdfcraft-cli.exe`).

```powershell
$env:PDFCRAFT_CLI = "D:\temp\fakescan-target-optical\debug\pdfcraft-cli.exe"
$env:PDFCRAFT_WIA_LAB = "1"
cd D:\code\pdfcraft-fakescan
python -m pytest -m pdfcraft_wia -q --tb=short
```

Offline harness (no WIA):

```powershell
cd D:\code\fakescan\python
python -m pytest -m "not live" -q
cd D:\code\pdfcraft-fakescan
python -m pytest -q
```
