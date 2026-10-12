# WS-Scan / WDP Scan Service (Microsoft)

The **scanner application protocol**: SOAP operations on the hosted Scan Service endpoint (fakescan: `/ScannerService`).

## Spec status

| Question | Answer |
|----------|--------|
| IETF RFC? | **No** |
| Owner | Microsoft (Windows Rally / WSD) |
| Canonical doc | **Scan Service Definition V1.0 for Web Services on Devices** (Word + WSDL/XSD zip, dated 2012-02-09) |
| Companion | **Scan Device Definition V1.0** (device-level types / categories) |
| Download hub | [Implementing Web Services on Devices for Printing](https://learn.microsoft.com/en-us/windows-hardware/design/whitepapers/implementing-web-services-on-devices-for-printing) |
| HTML element catalog | [WSD Scan Service Operation Elements](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/wsd-scan-service-operation-elements) |
| Distilled rules + XSD | [docs/reference/](../reference/README.md) · coverage [coverage.md](../coverage.md) |

Windows calls the inbox consumer path **WDP (Windows Device Protocol) for scanners**; the XML namespace still says `wdp/scan`.

## Namespaces

| URI | Role |
|-----|------|
| `http://schemas.microsoft.com/windows/2006/01/wdp/scan` | Canonical WSDScan / WDK examples (`SCAN_NS` in fakescan) |
| `http://schemas.microsoft.com/windows/2006/08/wdp/scan` | Variant used by **sane-airscan** and often by live Windows clients (`SCAN_NS_AIRSCAN`) |

fakescan echoes the request’s scan namespace on responses. Action URIs look like:

```text
{scan_ns}/GetScannerElements
{scan_ns}/CreateScanJob
{scan_ns}/RetrieveImage
…
```

SOAP 1.2 + WS-Addressing (`wsa:Action`, `wsa:MessageID`, `wsa:To`, `wsa:RelatesTo`).

## Required operations (Scan Service)

Client sends `XxxRequest`; service returns `XxxResponse` or a SOAP fault with `wscn:` subcodes.

| Operation | Purpose |
|-----------|---------|
| **GetScannerElements** | Capabilities / status / description / default ticket fragments |
| **ValidateScanTicket** | Accept or reject a ticket before job create |
| **CreateScanJob** | Start a job; returns `JobId`, `JobToken`, image geometry |
| **RetrieveImage** | Pull one image (usually **MTOM** / XOP); repeat until empty |
| **CancelJob** | Abort |
| **GetActiveJobs** / **GetJobElements** / **GetJobHistory** | Job tables |

Element-level docs (examples + faults):  
[GetScannerElementsRequest](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/getscannerelementsrequest),  
[CreateScanJobRequest](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/createscanjobrequest),  
[RetrieveImageRequest](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/retrieveimagerequest).

### GetScannerElements

Client lists QNames under `RequestedElements` / `Name`, e.g.:

- `wscn:ScannerDescription`
- `wscn:ScannerConfiguration`
- `wscn:ScannerStatus`
- `wscn:DefaultScanTicket`

Unknown names should come back as `ElementData` with `Valid="false"` (not a hard fault for the whole request).

### CreateScanJob

Two initiation modes in the spec:

1. **Client-pull (app acquire)** — body is essentially a `ScanTicket` only. This is what WIA / pdfcraft / airscan use for glass/ADF acquire.
2. **Device-push (ScanAvailableEvent)** — also carries `ScanIdentifier` + `DestinationToken` from a prior WS-Eventing subscription to scanner-button events. fakescan does not need this for pdfcraft E2E.

`MustHonor` on ticket fields: if true and unsupported → reject; if false → substitute/ignore.

Important faults: `wscn:ClientErrorNoImagesAvailable`, `wscn:ClientErrorMediaJam` (mid-job `jam_after_n`), `ClientErrorFormatNotSupported`, `ServerErrorNotAcceptingJobs`, …

### RetrieveImage

Requires `JobId` + `JobToken` from CreateScanJob. Response body is `ScanData` with an `xop:Include` pointing at an MTOM MIME part (`jfif`, `png`, `tiff-…`, etc.). Faults when no pages left: `wscn:ClientErrorNoImagesAvailable`. Mid-job jam (`jam_after_n`): SOAP subcode `wscn:ClientErrorMediaJam`.

Image formats catalog for fakescan: [formats.md](../formats.md).

**MTOM / wsdscdrv accept:** packaging A/B vs MS sample exhausted (still CancelJob); JPEG payload next — [mtom-retrieveimage.md](mtom-retrieveimage.md) · [e2e-wia-status.md](../e2e-wia-status.md).

## Typical client-pull sequence

```text
GetScannerElements  (optional, often first)
ValidateScanTicket  (optional)
CreateScanJob       (ScanTicket)
loop:
  RetrieveImage     (MTOM) until NoImagesAvailable
CancelJob           (optional cleanup)
```

On Windows, **WS-Eventing Subscribe** usually happens *before* or around open — see [ws-eventing.md](ws-eventing.md). That is **not** a Scan Service operation; it shares the same HTTP endpoint.

## fakescan mapping

| Spec piece | fakescan |
|------------|----------|
| Scan Service Address | `http://{advertise}:{port}/ScannerService` |
| Dispatcher | `crates/fakescan/src/wsd/mod.rs` |
| Elements / ticket encode | `wsd/elements.rs`, profile JSON |
| Page bitmaps | control API `/v1/pages` + patterns/effects |

## Related but different

**Print** Service Definition (WS-Print v1.x / v2.0) lives on the same download page — do not implement print ops here.
