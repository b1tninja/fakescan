# ValidateScanTicket vs CreateScanJob

These are **different operations**. Mixing their failure modes cost us the lossless WIA path.

## ValidateScanTicket

Sources: [Request](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/validatescanticketrequest), [Response](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/validatescanticketresponse), [ValidTicket](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/validticket), [ImageInformation](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/imageinformation).

| Fact | Detail |
|------|--------|
| Purpose | Ask whether a future ScanTicket is acceptable |
| Success shape | SOAP **200** + `ValidateScanTicketResponse` / `ValidationInfo` |
| Validity flag | `ValidTicket` true/false — **not** the FormatNotSupported SOAP fault |
| Geometry | `ImageInformation` / `MediaFrontImageInfo` (`PixelsPerLine`, `NumberOfLines`, `BytesPerLine`) |
| Invalid example | Learn shows `ValidTicket=false`, zeroed ImageInfo, and optional `ValidScanTicket` with a **corrected** ticket — fakescan emits that rewrite when `ValidTicket` is actually false (e.g. bad dpi under `refuse_unknown`); format soft-accept stays `true` and omits the rewrite |
| Named Validate fault | `ClientErrorConflictingRequiredParameters` (MustHonor conflicts) |

WIA Connect / property settle often calls Validate **many times** before any CreateScanJob.

## CreateScanJob

Sources: [Request](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/createscanjobrequest), [Response](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/createscanjobresponse).

| Fact | Detail |
|------|--------|
| Purpose | Actually start the job |
| Refusal shape | SOAP **Fault** with `wscn:` subcode (see [faults.md](faults.md)) |
| Format refusal | Triggers **wsdscdrv format fallback** ([format.md](format.md)) |
| Success | `JobId`, `JobToken`, `ImageInformation`, `DocumentFinalParameters` |
| Next step | RetrieveImage within ~60 s or `JobTimedOut` |

WIA may also CreateScanJob with `JobName` like “Validating scan ticket for current WIA item properties” — still a real CreateScanJob on the wire.

## What went wrong in lab (2026-10-12)

```text
Profile omits jfif (wia-lossless)
    │
    ├─ ValidateScanTicket(jfif) → ValidTicket=false + ImageInfo
    │     WIA may still CreateScanJob, or stall without Format fallback
    │
    └─ CreateScanJob(jfif) → generic invalid-ticket fault
          └── no PNG/EXIF/TIFF retry

Fix path that worked:
    Validate: soft-accept format miss (ValidTicket=true) so WIA proceeds
    Create:   ClientErrorDocumentFormatNotSupported / FormatNotSupported
              → driver retries → dib / png / tiff-single-uncompressed
```

### Rules of thumb

1. **Do not** expect Validate’s `ValidTicket=false` to run the Format fallback algorithm — that algorithm is documented on **Create** format faults.
2. **Do** put the named format fault on **CreateScanJob** when refusing Format.
3. **Do** keep CreateScanJob FinalParams + ImageInformation coherent with the format you will Retrieve (BPL=0 for compressed per MS; Brother JFIF quirk noted separately).
4. App format ≠ wire format: pdfcraft `--format bmp` → WIA DIB → wire `dib`; `--format jpeg` → wire `jfif`.

fakescan implementation notes (lab adaptations, tracked in [coverage.md](../coverage.md) — not claimed as MS-required):

- Validate soft-accept for DocumentFormatNotSupported (`ValidTicket=true`) so WIA still reaches Create.
- When Validate returns `ValidTicket=false`, ImageInformation is zeroed and `ValidScanTicket` carries a corrected ticket (nearest profile dpi, preferred format, JobDescription kept when present).
