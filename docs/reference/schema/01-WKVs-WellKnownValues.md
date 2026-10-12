# WellKnownValues

13 types declared in this schema file, sorted by name.

## ColorWKVType

- **Kind:** simple
- **Value:** `BlackAndWhite1|Grayscale4|Grayscale8|Grayscale16|RGB24|RGB48|RGBa32|RGBa64`
- **Allowed values:** `BlackAndWhite1`, `Grayscale4`, `Grayscale8`, `Grayscale16`, `RGB24`, `RGB48`, `RGBa32`, `RGBa64`

## ContentTypeWKVType

- **Kind:** simple
- **Value:** `Auto|Text|Photo|Halftone|Mixed`
- **Allowed values:** `Auto`, `Text`, `Photo`, `Halftone`, `Mixed`

## DocumentFormatWKVType

- **Kind:** simple
- **Value:** `dib|exif|jbig|jfif|jpeg2k|pdf-a|png|tiff-single-uncompressed|tiff-single-g4|tiff-single-g3mh|tiff-single-jpeg-tn2|tiff-multi-uncompressed|tiff-multi-g4|tiff-multi-g3mh|tiff-multi-jpeg-tn2|xps`
- **Allowed values:** `dib`, `exif`, `jbig`, `jfif`, `jpeg2k`, `pdf-a`, `png`, `tiff-single-uncompressed`, `tiff-single-g4`, `tiff-single-g3mh`, `tiff-single-jpeg-tn2`, `tiff-multi-uncompressed`, `tiff-multi-g4`, `tiff-multi-g3mh`, `tiff-multi-jpeg-tn2`, `xps`

## DocumentInputSourceWKVType

- **Kind:** simple
- **Value:** `ADF|ADFDuplex|Film|Platen`
- **Allowed values:** `ADF`, `ADFDuplex`, `Film`, `Platen`

## FilmScanModeWKVType

- **Kind:** simple
- **Value:** `NotApplicable|ColorSlideFilm|ColorNegativeFilm|BlackandWhiteNegativeFilm`
- **Allowed values:** `NotApplicable`, `ColorSlideFilm`, `ColorNegativeFilm`, `BlackandWhiteNegativeFilm`

## JobStateReasonWKVType

- **Kind:** simple
- **Value:** `InvalidScanTicket|CompressionError|DocumentFormatError|JobCanceledAtDevice|JobCompletedSuccessfully|JobCompletedWithErrors|JobCompletedWithWarnings|JobScanning|JobScanningAndTransfering|JobTimedOut|JobTransfering|None|ScannerStopped`
- **Allowed values:** `InvalidScanTicket`, `CompressionError`, `DocumentFormatError`, `JobCanceledAtDevice`, `JobCompletedSuccessfully`, `JobCompletedWithErrors`, `JobCompletedWithWarnings`, `JobScanning`, `JobScanningAndTransfering`, `JobTimedOut`, `JobTransfering`, `None`, `ScannerStopped`

## JobStateWKVType

- **Kind:** simple
- **Value:** `Aborted|Canceled|Completed|Creating|Pending|Pending-Held|Processing|Started|Terminating`
- **Allowed values:** `Aborted`, `Canceled`, `Completed`, `Creating`, `Pending`, `Pending-Held`, `Processing`, `Started`, `Terminating`

## KeywordNsExtensionPatternType

- **Kind:** simple
- **Value:** `namespace keyword`

## MimeExtensionPatternType

- **Kind:** simple
- **Value:** `namespace keyword`

## RotationWKVType

- **Kind:** simple
- **Value:** `0|90|180|270`
- **Allowed values:** `0`, `90`, `180`, `270`

## ScannerStateReasonWKVType

- **Kind:** simple
- **Value:** `AttentionRequired|Calibrating|CoverOpen|InterlockOpen|InternalStorageFull|LampError|LampWarming|MediaJam|MultipleFeedError|None|Paused`
- **Allowed values:** `AttentionRequired`, `Calibrating`, `CoverOpen`, `InterlockOpen`, `InternalStorageFull`, `LampError`, `LampWarming`, `MediaJam`, `MultipleFeedError`, `None`, `Paused`

## ScannerStateWKVType

- **Kind:** simple
- **Value:** `Idle|Processing|Stopped`
- **Allowed values:** `Idle`, `Processing`, `Stopped`

## StringNsExtensionPatternType

- **Kind:** simple
- **Value:** `namespace keyword`
