# Image formats (WSD / WIA)

fakescan encodes RetrieveImage payloads for the WSD Scan `Format` / `FormatValue` tokens that map to common WIA transfer formats. Profile JSON may use short names (`jpeg`, `tiff`, `bmp`, `pdf`); the wire uses WSD tokens.

## Shipped

| Profile name | WSD `FormatValue` | MTOM `Content-Type` | Notes |
|--------------|-------------------|---------------------|-------|
| `jpeg` / `jpg` / `jfif` | `jfif` | `image/jpeg` | Page/side COM markers (`fakescan-page:N`) |
| `exif` | `exif` | `image/jpeg` | JFIF + minimal EXIF APP1 |
| `bmp` / `dib` | `dib` | `image/bmp` | WIA baseline (all WIA 2.0 devices support DIB) |
| `png` | `png` | `image/png` | WSD/WIA fallback preference after JPEG |
| `tiff` / `tif` | `tiff-single-uncompressed` | `image/tiff` | Single-page uncompressed TIFF 6.0 |
| `tiff-g4` / `g4` | `tiff-single-g4` | `image/tiff` | Bilevel CCITT Group 4 |
| `tiff-g3mh` / `g3mh` | `tiff-single-g3mh` | `image/tiff` | Bilevel CCITT T.4 1-D MH |
| `tiff-jpeg` / `jpeg-tn2` | `tiff-single-jpeg-tn2` | `image/tiff` | TIFF Compression=7 (JPEG TN2) |
| `tiff-multi` | `tiff-multi-uncompressed` | `image/tiff` | One RetrieveImage = all remaining pages |
| `tiff-multi-g4` | `tiff-multi-g4` | `image/tiff` | Multi-page G4 |
| `tiff-multi-g3mh` | `tiff-multi-g3mh` | `image/tiff` | Multi-page G3 MH |
| `tiff-multi-jpeg-tn2` | `tiff-multi-jpeg-tn2` | `image/tiff` | Multi-page JPEG TIFF |
| `jpeg2k` / `jp2` / `j2k` | `jpeg2k` | `image/jp2` | JPEG 2000 JP2 (`openjpeg2-pure-rs`) |
| `pdf` / `pdf-a` | `pdf-a` | `application/pdf` | Embedded JPEG pages (not certified PDF/A-1b) |
| `xps` | `xps` | `application/vnd.ms-xpsdocument` | Minimal OpenXPS zip with JPEG resources |

Constant `SHIPPED_WSD_FORMATS` in `state.rs` is the authoritative list. Only formats listed on the active profile **and** encodable are advertised in `ScannerConfiguration`.

### Multi-page RetrieveImage

For `tiff-multi-*`, `pdf-a`, and `xps`, a single RetrieveImage consumes **all** remaining job pages into one file. Single-image Formats still return one page per call.

## Profiles

| Profile | Formats |
|---------|---------|
| `brother-wsd` | `jfif`, `tiff-single-uncompressed`, `dib` |
| `brother-usb` | `bmp` → `dib` |
| `wia-rich` | all shipped tokens |

```bash
cargo run -p fakescan -- --profiles profiles --profile wia-rich
```

## WSD / WIA catalog (reference)

From [WSD Format](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/format) and [WIA_IPA_FORMAT](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/wia-ipa-format) / [ImageScannerFormat](https://learn.microsoft.com/en-us/uwp/api/windows.devices.scanners.imagescannerformat):

| WSD token | Typical WIA | Status |
|-----------|-------------|--------|
| `jfif` | JPEG / JFIF | **shipped** |
| `exif` | EXIF JPEG | **shipped** |
| `dib` | BMP / DIB | **shipped** |
| `png` | PNG | **shipped** |
| `tiff-single-uncompressed` | TIFF (single) | **shipped** |
| `tiff-single-g4` | TIFF G4 | **shipped** |
| `tiff-single-g3mh` | TIFF G3 MH | **shipped** |
| `tiff-single-jpeg-tn2` | TIFF JPEG | **shipped** |
| `tiff-multi-uncompressed` | multi TIFF | **shipped** |
| `tiff-multi-g4` | multi G4 TIFF | **shipped** |
| `tiff-multi-g3mh` | multi G3 MH TIFF | **shipped** |
| `tiff-multi-jpeg-tn2` | multi JPEG TIFF | **shipped** |
| `jpeg2k` | JPEG 2000 | **shipped** (JP2) |
| `pdf-a` | PDF/A | **shipped** (valid PDF + JPEG; not full PDF/A packaging) |
| `xps` | XPS | **shipped** (minimal FixedDocument package) |
| `jbig` | JBIG1 (ISO/IEC 11544) | **not planned** — JBIG-KIT is GPL; no MIT/Apache encoder; JBIG2 is a different codec |

When a client asks for an unsupported Format, WSDScan tries **PNG → monochrome G4 TIFF → uncompressed single-page TIFF**. Advertising `png` and `tiff-single-g4` / `tiff-single-uncompressed` exercises that path.

## Markers

QR and pixel patterns are drawn before encode for every format. JPEG COM `fakescan-page:` / `fakescan-side:` exist on `jfif` / `exif` (and inside JPEG streams embedded in `pdf-a` / `xps` / JPEG TIFF).
