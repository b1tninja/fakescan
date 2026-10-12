# WSD Scan Service operations & events

Source: `schema/WSDScannerService.wsdl` (Scan Service Definition V1.0).

You cannot extend this operation list inside the same WSDL `portType`; vendor ops need a new portType ([Learn — operation elements](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/wsd-scan-service-operation-elements)).

## Request / response operations

| Operation | Purpose | fakescan |
|-----------|---------|----------|
| **GetScannerElements** | Root description / configuration / status / default ticket | **yes** |
| **ValidateScanTicket** | Pre-check a `ScanTicket`; returns `ValidationInfo` | **yes** (see [validate-vs-create.md](validate-vs-create.md)) |
| **CreateScanJob** | Start job; returns `JobId`, `JobToken`, `ImageInformation`, `DocumentFinalParameters` | **yes** |
| **RetrieveImage** | Pull image (MTOM / XOP); repeat until `ClientErrorNoImagesAvailable` | **yes** |
| **CancelJob** | Abort by `JobId` | **yes** |
| **GetActiveJobs** | Active job table | **yes** (JobStateReasons + ScansCompleted) |
| **GetJobElements** | Per-job elements | **yes** (JobStatus + Documents/DocumentFinalParameters) |
| **GetJobHistory** | Completed jobs | **yes** (Completed / Aborted / Canceled; cap 32) |

### CreateScanJob modes ([Learn](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/createscanjobrequest))

1. **Client-pull (app acquire)** — body is essentially `ScanTicket` only. WIA / pdfcraft / airscan.
2. **Device-push** — also `ScanIdentifier` + `DestinationToken` from `ScanAvailableEvent` subscription. fakescan: Subscribe registers DestinationTokens and stores the `NotifyTo` Address; `POST /v1/ctl` `push_scan` mints a ScanIdentifier and best-effort POSTs `ScanAvailableEvent` SOAP to that sink (ctl JSON still returns the ids if the sink is down); Create validates and consumes the pair.

### CreateScanJobResponse timing ([Learn](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/createscanjobresponse))

Client should send `RetrieveImage` within **60 seconds** or the service should abort with `JobStateReason` = `JobTimedOut`. Same timeout between successive RetrieveImage calls on multi-image jobs.

fakescan: default idle timeout **60s** (`FAKESCAN_JOB_TIMEOUT_SECS`, `World::set_job_timeout`, exposed as `status.job_timeout_ms`). Late `RetrieveImage` / `GetJobElements` abort the job (`Aborted` + `JobTimedOut`) and fault with SOAP subcode `wscn:OperationFailed` (reason text names `JobTimedOut`). Abandoned jobs are swept on the next `CreateScanJob` / status.

## Events (WSDL outbound)

| Event | Purpose | fakescan |
|-------|---------|----------|
| ScanAvailableEvent | Device-button / push scan | **yes** — `ctl push_scan` mints ids and POSTs to Subscribe NotifyTo (best-effort) |
| ScannerElementsChangeEvent | Config/description change | **done** — NotifyTo on profile/pattern/pages/feeder/effects/scenario; ElementChanges: Description → full Configuration → DefaultScanTicket |
| ScannerStatusSummaryEvent | Status summary | **done** — NotifyTo on `/v1/fault` deltas + Create/Retrieve/Cancel; State + Reasons mirror GetScannerElements |
| ScannerStatusConditionEvent | Condition raised | **done** — DeviceCondition (@Id/Time/Name/Component/Severity Critical|Warning|Informational) Ids 1–8; offline/Paused/LampWarming skipped (id=0) |
| ScannerStatusConditionClearedEvent | Condition cleared | **done** — DeviceConditionCleared (ConditionId + ConditionClearTime) |
| JobStatusEvent | Job progress | **done** — every successful RetrieveImage; NotifyTo when Subscribe registered a sink |
| JobEndStateEvent | Job ended | **done** — last Retrieve (`pages_left==0`), CancelJob (active), JobTimedOut on Retrieve; NotifyTo POST when sink set |

Inbox WIA also uses **WS-Eventing `Subscribe`** on the same HTTP endpoint (not a Scan Service op). Subscribe stores the NotifyTo and destination tokens; Unsubscribe clears them. Push is a no-op until a NotifyTo is registered ([specs/ws-eventing.md](../specs/ws-eventing.md), [coverage.md](../coverage.md)).

## Typical client-pull sequence

```text
Subscribe                 (WS-Eventing; Windows)
GetScannerElements        (often repeated)
ValidateScanTicket        (often ×N while WIA props settle)
CreateScanJob             (ScanTicket; Format often MustHonor)
RetrieveImage             (MTOM) …
CancelJob                 (optional / on failure)
```
