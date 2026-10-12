# MustHonor, Override, UsedDefault

## MustHonor (ticket → Create / Validate)

From [CreateScanJobRequest](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/createscanjobrequest):

- If `MustHonor` is **true** and the value is unsupported → **reject** the job.
- If the element has no `MustHonor`, or it is **false**, and the element is **unsupported** → **ignore**.
- If supported and `MustHonor` is **false** → service may **substitute** a supported value.
- Conflicting MustHonor=true elements → reject (`ClientErrorConflictingRequiredParameters` on Validate).

### HTML allow-list (CreateScanJob remarks)

Learn lists these as allowed to carry MustHonor:

`ColorProcessing`, `CompressionQualityFactor`, `ContentType`, `Exposure`, `FilmScanMode`, `ImagesToTransfer`, `InputSize`, `InputSource`, `MediaSides`, `Resolution`, `Rotation`, `Scaling`, `ScanRegionHeight`, `ScanRegionWidth`, `ScanRegionXOffset`, `ScanRegionYOffset`.

**Format is not named in that HTML list.**

### XSD reality

[`schema/Document.xsd`](schema/Document.xsd): `DocumentFormatType` **does** allow `wscn:MustHonor` on `Format`. Inbox WIA routinely sends:

```xml
<wscn:Format wscn:MustHonor="true">jfif</wscn:Format>
```

So: schema permits it; HTML prose omits it; Windows stamps it. Treat MustHonor Format as **live wire**, not “invalid XML”.

Default in [`schema/Common.xsd`](schema/Common.xsd): `MustHonor` attribute default **`false`**.

### fakescan (`refuse_unknown` profiles)

- Parses `MustHonor` on **Format**, **Resolution**, **InputSource**, **ScanRegionWidth/Height/XOffset/YOffset**, and **ColorProcessing** (`extract_must_honor`; absent → false).
- Unsupported **dpi** + MustHonor not true → substitute nearest profile dpi; ticket stays valid.
- Unsupported **format** + MustHonor not true → substitute `preferred_format`; ticket stays valid.
- Unsupported **ColorProcessing** + MustHonor not true → substitute first profile `colors` entry (or `RGB24` if the list is empty); ticket stays valid.
- Unsupported **InputSource** + MustHonor not true → ignore (ticket stays valid).
- Out-of-bounds **ScanRegion** + MustHonor not true → ignore.
- Unsupported + MustHonor=true → reject (format reason keeps `ClientErrorDocumentFormatNotSupported` so Create hard-faults; Validate may still soft-accept that phrase — see [validate-vs-create.md](validate-vs-create.md)).
- Profile `colors` (WSD tokens `RGB24` / `Grayscale8` / `BlackAndWhite1`, same aliases as `ColorMode::parse`): empty or omitted → every token soft-maps; non-empty → allow-list under `refuse_unknown`.
- Conflicting MustHonor=true (e.g. unsupported Resolution **and** Format, Format **and** ColorProcessing, or InputSource **and** Format) → `ClientErrorConflictingRequiredParameters` on Validate (SOAP fault) and Create.

## Override / UsedDefault (DocumentFinalParameters)

On CreateScanJobResponse, FinalParameters may differ from the ticket ([DocumentFinalParameters](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/documentfinalparameters)):

- `Override="true"` — device replaced the requested value.
- `UsedDefault="true"` — device filled its default.

Elements that may carry those attributes (Learn): Brightness, ColorProcessing, CompressionQualityFactor, ContentType, Contrast, FilmScanMode, **Format**, Height, ImagesToTransfer, InputSource, Rotation, ScalingHeight, ScalingWidth, ScanRegion*, Sharpness, Width.

fakescan emits FinalParameters with Override on substituted Format/Resolution (and ColorProcessing when JFIF coerces RGB24), and UsedDefault when Format/Resolution were omitted — see [coverage.md](../coverage.md).
