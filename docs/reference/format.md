# Format (document image format)

Canonical token list: `DocumentFormatWKVType` in [`schema/WellKnownValues.xsd`](schema/WellKnownValues.xsd).  
HTML: [Format Element](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/format).

## Allowed tokens (WKV)

| Token | Notes |
|-------|--------|
| `dib` | Windows DIB / BMP; WIA 2.0 baseline |
| `exif` | EXIF JPEG |
| `jbig` | JBIG1 only (no JBIG2 in WSD) |
| `jfif` | JPEG File Interchange Format |
| `jpeg2k` | JPEG 2000 |
| `pdf-a` | PDF/A-shaped |
| `png` | PNG |
| `tiff-single-uncompressed` | Single-page uncompressed TIFF |
| `tiff-single-g4` | Single-page G4 |
| `tiff-single-g3mh` | Single-page G3 MH |
| `tiff-single-jpeg-tn2` | Single-page TIFF+JPEG TN2 |
| `tiff-multi-uncompressed` | Multi-page uncompressed |
| `tiff-multi-g4` / `tiff-multi-g3mh` / `tiff-multi-jpeg-tn2` | Multi-page variants |
| `xps` | OpenXPS |
| vendor MIME extensions | Via `MimeExtensionPatternType` union |

fakescan encode matrix: [formats.md](../formats.md) (`jbig` not planned).

## Parent elements

- `DocumentParameters` (ticket / Validate / Create)
- `DocumentFinalParameters` (CreateScanJobResponse — actual values used)

On FinalParameters, Format may carry `Override` / `UsedDefault` ([documentfinalparameters](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/documentfinalparameters)).

## Refusal → inbox WIA fallback (critical)

If the scanner does **not** support the requested Format, it **must** reject with a format-not-supported fault. Then the **WSD scanner driver (wsdscdrv)** retries other formats in this order ([Learn Format remarks](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/format)):

1. **PNG**
2. **EXIF** (only if color is RGB24 or grayscale 8)
3. **tiff-single-g4** (only if monochrome 1 bpp)
4. **tiff-single-uncompressed**

Also: if auto-configured scanning requests **DIB** and the device lacks DIB, the **same** algorithm runs.

### Lab consequences

| Do | Don’t |
|----|--------|
| Fault CreateScanJob with a **named** format subcode | Return a generic “invalid ticket” / `OperationFailed` and expect retries |
| Advertise the formats you can actually Retrieve (`ScannerConfiguration` / profile) | Assume omitting `jfif` from caps stops WIA from *asking* for jfif — inbox often still tries JPEG first |
| Put `png` and/or `tiff-single-uncompressed` (and usually `dib`) on a “lossless” profile | Expect FormatNotSupported to fall **back to jfif** — that direction is not documented |
| Use pdfcraft `--format jpeg` vs `--format bmp` deliberately | Assume app “BMP” always means wire `dib` without checking CreateScanJob |

fakescan profile **`wia-lossless`**: no `jfif`; wires `dib` after fallback (or direct dib when WIA already prefers Bitmap).

## Fault name split (XSD vs Format page)

| Source | Subcode |
|--------|---------|
| WSDL / `ScanServiceFaultValuesType` | `wscn:ClientErrorFormatNotSupported` |
| Format element Learn remarks (WIA fallback text) | `ClientErrorDocumentFormatNotSupported` |
| JobStateReason WKV | `DocumentFormatError` |

fakescan emits `wscn:ClientErrorFormatNotSupported` (XSD) on CreateScanJob format refusal. Inbound reason strings still match Document*, Format*, and `"format … not in profile"` so WIA fallback keeps working ([faults.md](faults.md)).

## BytesPerLine vs Format

[BytesPerLine](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/bytesperline): for **compressed** formats, service must return **0**. Uncompressed formats return stride (data + padding). Brother live JFIF often uses RGB stride anyway — lab notes that as a Brother quirk, not the MS rule ([e2e-wia-status.md](../e2e-wia-status.md)). Default fakescan matches Brother (`width×3` for JFIF). Set profile `bytes_per_line_ms: true` for the MS compressed=0 rule on Validate/Create ImageInformation.

## Multi-page RetrieveImage

[RetrieveImageResponse](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/retrieveimageresponse):

- Single-image Format → one image per RetrieveImage; client loops until `ClientErrorNoImagesAvailable` or `ImagesToTransfer` met.
- Multi-page Format → one RetrieveImage may return many images (up to `ImagesToTransfer` / available media).
