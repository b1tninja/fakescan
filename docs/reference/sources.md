# Authoritative sources

Crawled / mirrored for fakescan reference (2026-10-12).

## Primary package (WSDL + XSD)

| Artifact | Location |
|----------|----------|
| **Scan Service Definition V1.0** | [ScanService.zip](https://download.microsoft.com/download/9/C/5/9C5B2167-8017-4BAE-9FDE-D599BAC8184A/ScanService.zip) (1.5 MB; 2012-02-09) |
| Hub page | [Implementing Web Services on Devices for Printing](https://learn.microsoft.com/en-us/windows-hardware/design/whitepapers/implementing-web-services-on-devices-for-printing) (hosts Print + Scan zips) |
| Companion | [ScanDevice.zip](https://download.microsoft.com/download/9/c/5/9c5b2167-8017-4bae-9fde-d599bac8184a/ScanDevice.zip) — device-level types |

Fetched locally by `scripts/Get-WsdSpecs.ps1` (not checked in; schemas only, not the Word spec):

```text
docs/reference/schema/
  WSDScannerService.wsdl
  WDPScan.xsd  Common.xsd  Document.xsd  Job.xsd  Scanner.xsd  WellKnownValues.xsd
```

Microsoft copyright on the XSD headers. The zip stays under `docs/reference/upstream/` after `scripts/Get-WsdSpecs.ps1`; the Word document (`WSDScanService.doc`) is left inside that zip.

### Namespace in the package

Target namespace in the XSD/WSDL:

```text
http://schemas.microsoft.com/windows/2006/08/wdp/scan
```

Learn HTML samples and older WDK examples often use:

```text
http://schemas.microsoft.com/windows/2006/01/wdp/scan
```

fakescan accepts either and echoes the request namespace ([specs/ws-scan.md](../specs/ws-scan.md)).

## Learn HTML (element catalog)

| Page | URL |
|------|-----|
| WSD reference hub | https://learn.microsoft.com/en-us/windows-hardware/drivers/image/web-services-on-devices-reference |
| Operation elements | https://learn.microsoft.com/en-us/windows-hardware/drivers/image/wsd-scan-service-operation-elements |
| Job / document elements | https://learn.microsoft.com/en-us/windows-hardware/drivers/image/wsd-scan-service-job-and-document-elements |
| Error reporting | https://learn.microsoft.com/en-us/windows-hardware/drivers/image/wsd-scan-service-operation-error-reporting |
| Common faults | https://learn.microsoft.com/en-us/windows-hardware/drivers/image/common-wsd-scan-service-operation-error-codes |
| **Format** (+ WIA fallback) | https://learn.microsoft.com/en-us/windows-hardware/drivers/image/format |
| CreateScanJobRequest | https://learn.microsoft.com/en-us/windows-hardware/drivers/image/createscanjobrequest |
| CreateScanJobResponse | https://learn.microsoft.com/en-us/windows-hardware/drivers/image/createscanjobresponse |
| ValidateScanTicketRequest | https://learn.microsoft.com/en-us/windows-hardware/drivers/image/validatescanticketrequest |
| ValidateScanTicketResponse | https://learn.microsoft.com/en-us/windows-hardware/drivers/image/validatescanticketresponse |
| DocumentFinalParameters | https://learn.microsoft.com/en-us/windows-hardware/drivers/image/documentfinalparameters |
| BytesPerLine | https://learn.microsoft.com/en-us/windows-hardware/drivers/image/bytesperline |
| ImageInformation | https://learn.microsoft.com/en-us/windows-hardware/drivers/image/imageinformation |
| RetrieveImageResponse | https://learn.microsoft.com/en-us/windows-hardware/drivers/image/retrieveimageresponse |

Note: some older TOC index pages (`scan-service--ws-scan`, document-element indexes) were emptied or removed in 2025 doc curation; element pages above remain.

## Related (not WS-Scan schema)

| Spec | Role |
|------|------|
| OASIS DPWS 1.1 | Device vs hosted service |
| OASIS WS-Discovery | UDP 3702 |
| W3C WS-Transfer / WS-Eventing | Device `Get`, Subscribe |
| UWP `ImageScannerFormat` | App-facing format preference (Jpeg → Png → Bitmap) |

See [specs/README.md](../specs/README.md).
