# Spec coverage map

Maps **official WS-Scan / WSD** surface ([reference/](reference/README.md)) → fakescan. Status: **done** | **partial** | **stub** | **gap** | **n/a**.

Update this when closing a reference item; keep [roadmap.md](roadmap.md) for product ideas (effects, patterns) that are not schema requirements.

## Operations ([reference/operations.md](reference/operations.md))

| Item | Status | Notes |
|------|--------|-------|
| GetScannerElements | **done** | Description / Configuration / Status / DefaultScanTicket; ColorEntry from profile `colors`; ActiveConditions + ConditionHistory (cleared sticky faults); unknown → Valid=false |
| ValidateScanTicket | **done** | ImageInformation + ValidTicket; format miss soft-accepted (lab, inbox fallback); ValidScanTicket rewrite when ValidTicket=false; ConflictingRequiredParameters → SOAP fault |
| CreateScanJob (client-pull) | **done** | JobId/Token, ImageInformation, DocumentFinalParameters |
| CreateScanJob (device-push) | **done** | Validates ScanIdentifier + DestinationToken; consumes one-shot id |
| RetrieveImage + MTOM | **done** | WIA JPEG/dib Transfer OK (XOP `2004/08`, Brother optical crop); mid-MTOM abort + `retrieve_bps` throttle; accept-gate cleared 2026-10-12 ([e2e-wia-status.md](e2e-wia-status.md)). Packaging + SOF↔ImageInformation locked by unit tests (`build_mtom`, airscan flow). Gray+JFIF FinalParams RGB24 + BPL×3 **code-locked** (flow tests); live gray re-proof + sticky offline remain live-only ([mtom-retrieveimage.md](specs/mtom-retrieveimage.md)) |
| CancelJob | **done** | |
| GetActiveJobs | **done** | JobSummary: StateReasons + ScansCompleted |
| GetJobElements | **done** | JobStatus (Reasons, ScansCompleted, Images*); Documents/DocumentFinalParameters (Format, dpi, pages_left) |
| GetJobHistory | **done** | Completed / Aborted / Canceled rows (cap 32) |
| ScanAvailableEvent + push flow | **done** | Subscribe stores DestinationToken + NotifyTo; `ctl push_scan` mints ScanIdentifier and POSTs ScanAvailableEvent best-effort |
| Other Scanner*/Job* events | **done** | JobStatus every Retrieve + JobEnd; Condition DeviceCondition/Cleared (wall-clock Severity); StatusSummary on fault + Create/Retrieve/Cancel; ElementsChange Description→Configuration→Ticket; NotifyTo no-op when unset |
| WS-Eventing Subscribe | **done** | Accept, DestinationResponses, tokens, NotifyTo, Identifier/Expires PT\|`P1D`\|`P1DT…`\|UTC Z/`+00:00`, Renew/GetStatus/Unsubscribe Identifier check, Filter/@Dialect logged. SubscriptionManager EPR = **ScannerService** Address + Identifier (locked in flow tests). Single active subscription — re-Subscribe replaces (**n/a** concurrent). Filter body not evaluated (ignore unknown dialects) |

## Format & encoding ([reference/format.md](reference/format.md))

| Item | Status | Notes |
|------|--------|-------|
| Advertise FormatValue list from profile | **done** | |
| Encode jfif / dib / png / tiff-* / jpeg2k / pdf-a / xps | **done** | See formats.md; jbig **n/a** |
| CreateScanJob format fault → WIA fallback | **done** | Documented + proven (`wia-lossless`) |
| Emit `ClientErrorFormatNotSupported` (XSD name) | **done** | XSD outbound; Document* + "format … not in profile" still matched inbound |
| BytesPerLine=0 for compressed (MS rule) | **done** | Default Brother JFIF `width×3`; profile `bytes_per_line_ms=true` → MS 0 for compressed (both intentional) |
| Multi-page Format one-shot Retrieve | **done** | tiff-multi / pdf-a / xps |
| FinalParameters Override/UsedDefault | **done** | Format/Resolution Override on MustHonor=false substitute; UsedDefault when omitted; ColorProcessing Override when JFIF coerces RGB24; other UsedDefaults unchanged |

## MustHonor ([reference/must-honor.md](reference/must-honor.md))

| Item | Status | Notes |
|------|--------|-------|
| Honor MustHonor on dpi / color / source / region | **done** | dpi + InputSource + ScanRegion* + ColorProcessing (profile `colors` allow-list) |
| MustHonor Format (XSD + live WIA) | **done** | extract_tag + MustHonor attr; Create hard-faults when true |
| Substitute when MustHonor=false | **done** | Unsupported dpi → nearest; format → preferred_format; color → first profile color / RGB24 (absent = false) |
| Conflicting MustHonor → named fault | **done** | ≥2 MustHonor=true misses (Resolution/Format/ColorProcessing/InputSource/ScanRegion) → `ClientErrorConflictingRequiredParameters` (Validate SOAP fault + Create) |

## Validate vs Create ([reference/validate-vs-create.md](reference/validate-vs-create.md))

| Item | Status | Notes |
|------|--------|-------|
| Named format fault on Create only | **done** | |
| Validate soft-accept format miss | **done** | Lab adaptation — required for inbox fallback today |
| ValidScanTicket corrected ticket on invalid | **done** | When ValidTicket=false: zero ImageInfo + corrected dpi/format; format soft-accept still skips rewrite |
| JobTimedOut after 60s without Retrieve | **done** | Default 60s (`FAKESCAN_JOB_TIMEOUT_SECS` / `set_job_timeout`); Create→Retrieve + between Retrieves; SOAP `OperationFailed` + `JobStateReason=JobTimedOut`; sweep on Create/status |

## Faults ([reference/faults.md](reference/faults.md))

| Item | Status |
|------|--------|
| NoImagesAvailable / NotAcceptingJobs / Format* / JobIdNotFound | **done** |
| MediaJam / MultipleFeed (lab extensions) | **done** |
| InvalidJobToken / InvalidScanIdentifier / DestinationToken | **done** |
| InvalidArgs / OperationFailed / Temporary / Internal | **done** (InvalidArgs; OperationFailed for JobTimedOut; Temporary/Internal via `/v1/fault` `temporary`/`internal`) |
| ScannerStatus reasons (CoverOpen, LampError, …) | **done** — via `/v1/fault`; ActiveConditions DeviceCondition Id 1–9 |

## Discovery / DPWS ([specs/](specs/README.md))

| Item | Status |
|------|--------|
| Hello / ProbeMatch / Resolve | **done** |
| Device Get metadata Host + Hosted ScannerService | **done** |
| Both 2006/01 and 2006/08 scan NS | **done** |

## Scanner / job state matrix

Honest snapshot of `FaultBody` / `scanner_state_reasons` / `job_state` / ActiveConditions at time of writing. Catalog tokens locked in `wsd/status_matrix.rs`. Status: **done** | **partial** | **gap** | **in flight**.

### ScannerState

| Token | Status | When fakescan uses it |
|-------|--------|------------------------|
| Idle | **done** | No sticky fault / calibrating / paused / warming and no active job / `busy` |
| Processing | **done** | Active job (`scanning`), `busy`, `calibrating`, `paused`, or `lamp_warming` |
| Stopped | **done** | jam / cover / multi / lamp_error / offline / interlock / storage_full / lamp_warning |

### ScannerStateReason (`WellKnownValues.xsd`)

| Token | Status | How to trigger |
|-------|--------|----------------|
| None | **done** | Default idle; also `busy` alone (Processing + None) |
| MediaJam | **done** | ctl / `POST /v1/fault` `jam: true` (or `jam_after_n`) |
| CoverOpen | **done** | `cover_open: true` |
| MultipleFeedError | **done** | `multi_feed: true` (or `multi_feed_after_n`) |
| LampError | **done** | `lamp_error: true` |
| AttentionRequired | **done** | `offline: true` or `lamp_warning: true` (reason only — no DeviceCondition Id for offline) |
| Calibrating | **done** | `calibrating: true` |
| InterlockOpen | **done** | `interlock` / `interlock_open: true` |
| InternalStorageFull | **done** | `storage_full: true` |
| LampWarming | **done** | `lamp_warming: true` (also during `lamp_warmup_ms` sleep) |
| Paused | **done** | `paused: true` (reason only; Processing) |

### ConditionName (`Scanner.xsd` ActiveConditions / history)

| Token | Status | Id / notes |
|-------|--------|------------|
| MediaJam | **done** | Id **1**, ADF, Critical |
| CoverOpen | **done** | Id **2**, Platen, Critical |
| MultipleFeedError | **done** | Id **3**, ADF, Critical |
| LampError | **done** | Id **4**, Platen, Critical |
| Calibrating | **done** | Id **5**, Platen, Informational |
| InterlockOpen | **done** | Id **6**, MediaPath, Critical |
| InternalStorageFull | **done** | Id **7**, Platen, Critical |
| LampWarning | **done** | Id **8**, Platen, Warning; pairs with reason `AttentionRequired` |
| InputTrayEmpty | **done** | Id **9**, ADF, Warning — derived when profile has feeder, `!feeder_ready`, `pages_remaining==0` (no AttentionRequired) |

### JobState

| Token | Status | Notes |
|-------|--------|-------|
| Pending | **done** | CreateScanJob, `page_index == 0` |
| Processing | **done** | After first successful Retrieve (`page_index > 0`) |
| Completed | **done** | `pages_left == 0` |
| Aborted | **done** | JobTimedOut tombstone |
| Canceled | **done** | CancelJob → GetJobHistory |
| Creating | **n/a** | Sync CreateScanJob — never observable via GetJobElements |
| Pending-Held | **n/a** | No hold / resource-wait model |
| Started | **n/a** | Pending covers post-Create wait (WIA) |
| Terminating | **n/a** | Cancel removes the job before a Terminating poll window |

### JobStateReason

| Token | Status | Notes |
|-------|--------|-------|
| None | **done** | Idle default before Create (history / empty) |
| JobScanningAndTransfering | **done** | Set on each successful Retrieve while pages remain |
| JobCompletedSuccessfully | **done** | Last page Retrieve (no LampWarning) |
| JobTimedOut | **done** | Idle Create→Retrieve / between Retrieves |
| JobCanceledAtDevice | **done** | CancelJob history (`ClientErrorJobCancelled` on later Retrieve) |
| JobScanning | **done** | Set on Create (Pending) until first Retrieve |
| JobTransfering | **done** | Mid-job Retrieve while `retrieve_bps > 0` (throttled transfer) |
| JobCompletedWithErrors | **done** | Last page Retrieve while sticky `abort_retrieve` (mid-MTOM cut armed) |
| JobCompletedWithWarnings | **done** | Last page Retrieve while sticky `lamp_warning` |
| InvalidScanTicket | **n/a** | Ticket miss → SOAP fault before a job exists |
| CompressionError | **done** | Encode failure on Retrieve marks the job then faults |
| DocumentFormatError | **done** | Loopback re-encode miss on Retrieve marks the job then faults |
| ScannerStopped | **done** | Sticky jam/cover/multi/interlock/storage (or mid-job `*_after_n`) marks open jobs; Create/Retrieve refuse ClientError* |

### ConditionSeverity

| Token | Status | Notes |
|-------|--------|-------|
| Critical | **done** | ActiveConditions + ConditionHistory + Condition NotifyTo (jam/cover/multi/lamp/interlock/storage) |
| Warning | **done** | LampWarning Id 8 + InputTrayEmpty Id 9 (ActiveConditions, history, NotifyTo) |
| Informational | **done** | Calibrating Id 5 (ActiveConditions, history, NotifyTo) |

`Error` is **not** in the XSD (`ConditionSeverityBaseType`); an earlier emitter used it — fixed to `Critical`.

## Highest-value gaps (roadmap input)

Matrix and ops coverage rows are **done** or intentional **n/a**. Live-only leftovers (gray Create live re-proof, sticky offline after restart, Associate without person Add) stay in [e2e-wia-status.md](e2e-wia-status.md) / [windows-client.md](specs/windows-client.md), not schema gaps.

Done this round: Subscribe → **done**; gray+JFIF FinalParams/ValidScanTicket RGB24 coerce + Validate BPL×3 flow test; Hosted Get = Device Get (same dispatcher); `scan_schema` lab allowances; RetrieveImage+MTOM; job-reason + Condition fanout.

## How to extend this map

1. Open the Learn or XSD item in [reference/sources.md](reference/sources.md).
2. Add/adjust a row here with status.
3. If it is a product idea (effects, patterns), put it in [roadmap.md](roadmap.md) instead.
