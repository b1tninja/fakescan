# WSD Scan Service faults

Sources: [`schema/WDPScan.xsd`](schema/WDPScan.xsd) `ScanServiceFaultValuesType`, [common error codes](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/common-wsd-scan-service-operation-error-codes), [error reporting](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/wsd-scan-service-operation-error-reporting), [CreateScanJobRequest](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/createscanjobrequest).

## Envelope

SOAP 1.2 fault; `wsa:Action` = `http://schemas.xmlsoap.org/ws/2004/08/addressing/fault` (or https in Learn samples). Subcode is a QName under `wscn:` (or `wsa:` / `wprt:` for a few commons).

fakescan: `soap::fault_with_code` + HTTP 500.

## Catalog from Scan Service XSD

| Subcode | Typical use | fakescan |
|---------|-------------|----------|
| `wscn:InvalidArgs` | Bad/missing args | **yes** (push Create with only one of ScanIdentifier/DestinationToken) |
| `wscn:OperationFailed` | State prevents op; JobTimedOut after idle Create/Retrieve | **yes** (JobTimedOut) / partial |
| `wscn:ServerErrorTemporaryError` | Retry later | **yes** (`POST /v1/fault` `{ "temporary": true }` / ctl `fault`) |
| `wscn:ServerErrorInternalError` | Permanent internal | **yes** (`POST /v1/fault` `{ "internal": true }` / ctl `fault`) |
| `wscn:ServerErrorNotAcceptingJobs` | Offline / busy / lamp / buffers | **yes** |
| `wscn:ClientErrorJobCancelled` | Job cancelled | **yes** — Retrieve/GetJobElements after Cancel (history-only JobId) |
| `wscn:ClientErrorJobIdNotFound` | Unknown JobId | **yes** (approx) |
| `wscn:ClientErrorFormatNotSupported` | Bad Format on Create | **done** (XSD outbound; Document* + phrase still matched inbound) |
| `wscn:ClientErrorInvalidScanIdentifier` | Push-scan id | **done** |
| `wscn:ClientErrorInvalidDestinationToken` | Push-scan token | **done** |
| `wscn:ClientErrorNoImagesAvailable` | Empty feeder / no more pages | **yes** |
| `wscn:ClientErrorInvalidJobToken` | Bad JobToken on Retrieve | **done** |
| `wscn:ClientErrorConflictingRequiredParameters` | MustHonor conflict (Validate SOAP + Create) | **done** (≥2 MustHonor=true misses) |

## Learn-only / driver extensions (not in XSD enum)

| Name | Where documented | fakescan |
|------|------------------|----------|
| `ClientErrorDocumentFormatNotSupported` | Format element remarks (WIA fallback) | **yes** (inbound alias only; outbound is XSD FormatNotSupported) |
| `ClientErrorMediaJam` | Lab / status `MediaJam` | **yes** |
| `ClientErrorMultipleFeed` | Lab / status `MultipleFeedError` | **yes** |
| `ClientErrorCoverOpen` | Lab / status `CoverOpen` (no cover/door value exists in the XSD enum; same family as MediaJam). Pre-flight (`cover_open`), `cover_open_after_n`, fault plan `cover_open` | **yes** |
| `ClientErrorInterlockOpen` | Lab / status `InterlockOpen` | **yes** |
| `wsa:ActionNotSupported` | Common codes | via “action not implemented” |
| `wprt:ServerErrorTemporaryError` | Common codes (print NS typo in Learn) | no |

## Scanner / job reason keywords (status, not SOAP faults)

From `WellKnownValues.xsd`. Per-token **done** / **partial** / **gap** / **in flight** and ctl triggers: [coverage.md § Scanner / job state matrix](../coverage.md#scanner--job-state-matrix). Catalog arrays + ratchet test: `wsd/status_matrix.rs`.

**ScannerStateReason:** AttentionRequired, Calibrating, CoverOpen, InterlockOpen, InternalStorageFull, LampError, LampWarming, MediaJam, MultipleFeedError, None, Paused.

**ConditionSeverity** (`ConditionSeverityBaseType` in `Scanner.xsd`): Informational, Warning, Critical. Jam/cover/multi/lamp DeviceConditions (ActiveConditions, ConditionHistory, Condition NotifyTo) emit `Critical`. Do not use `Error` (invalid; fixed to `Critical`).

**JobStateReason:** InvalidScanTicket, CompressionError, DocumentFormatError, JobCanceledAtDevice, JobCompletedSuccessfully, JobCompletedWithErrors, JobCompletedWithWarnings, JobScanning, JobScanningAndTransfering, JobTimedOut, JobTransfering, None, ScannerStopped.

(Note Learn’s JobStateReason page spelling sometimes differs: Transferring vs Transfering.)
