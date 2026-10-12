# Document

48 types declared in this schema file, sorted by name.

## ActualDocumentParametersType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| Format | DocumentFormatOverrideType | 1 | 1 |
| CompressionQualityFactor | CompressionQualityFactorOverrideType | 1 | 1 |
| ImagesToTransfer | ImagesToTransferOverrideType | 1 | 1 |
| InputSource | DocumentInputSourceOverrideType | 1 | 1 |
| FilmScanMode | FilmScanModeOverrideType | 0 | 1 |
| ContentType | ContentTypeOverrideType | 1 | 1 |
| InputSize | DocumentInputSizeOverrideType | 1 | 1 |
| Exposure | ScanExposureOverrideType | 1 | 1 |
| Scaling | ScalingOverrideType | 1 | 1 |
| Rotation | RotationOverrideType | 1 | 1 |
| MediaSides | MediaSidesOverrideType | 1 | 1 |
| any | any element | 0 | unbounded |

## Adjustment1000OverrideType

- **Kind:** complex
- **Base:** Adjustment1000Type

**Attributes**

| Name | Type | Required |
| --- | --- | --- |
| Override | boolean | no |
| UsedDefault | boolean | no |

## Adjustment100OverrideType

- **Kind:** complex
- **Base:** Adjustment100Type

**Attributes**

| Name | Type | Required |
| --- | --- | --- |
| Override | boolean | no |
| UsedDefault | boolean | no |

## ColorJobType

- **Kind:** complex
- **Base:** ColorConfigType

**Attributes**

| Name | Type | Required |
| --- | --- | --- |
| MustHonor | boolean | no |

## ColorOverrideType

- **Kind:** complex
- **Base:** ColorConfigType

**Attributes**

| Name | Type | Required |
| --- | --- | --- |
| Override | boolean | no |
| UsedDefault | boolean | no |

## CompressionQualityFactorOverrideType

- **Kind:** complex
- **Base:** CompressionQualityFactorExtType

**Attributes**

| Name | Type | Required |
| --- | --- | --- |
| Override | boolean | no |
| UsedDefault | boolean | no |

## CompressionQualityFactorType

- **Kind:** complex
- **Base:** CompressionQualityFactorExtType

**Attributes**

| Name | Type | Required |
| --- | --- | --- |
| MustHonor | boolean | no |

## ContentTypeOverrideType

- **Kind:** complex
- **Base:** ContentTypeExtType

**Attributes**

| Name | Type | Required |
| --- | --- | --- |
| Override | boolean | no |
| UsedDefault | boolean | no |

## ContentTypeTicketType

- **Kind:** complex
- **Base:** ContentTypeExtType

**Attributes**

| Name | Type | Required |
| --- | --- | --- |
| MustHonor | boolean | no |

## DimensionExtType

- **Kind:** complex
- **Base:** IntOneExtType

**Attributes**

| Name | Type | Required |
| --- | --- | --- |
| MustHonor | boolean | no |

## DimensionOverrideBaseType

- **Kind:** complex
- **Base:** IntOneExtType

**Attributes**

| Name | Type | Required |
| --- | --- | --- |
| Override | boolean | no |
| UsedDefault | boolean | no |

## DimensionsOverrideType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| Width | DimensionOverrideBaseType | 1 | 1 |
| Height | DimensionOverrideBaseType | 1 | 1 |

## DocumentDescriptionType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| DocumentName | String255ExtType | 1 | 1 |
| any | any element | 0 | unbounded |

## DocumentFormatOverrideType

- **Kind:** complex
- **Base:** DocumentFormatExtType

**Attributes**

| Name | Type | Required |
| --- | --- | --- |
| Override | boolean | no |
| UsedDefault | boolean | no |

## DocumentFormatType

- **Kind:** complex
- **Base:** DocumentFormatExtType

**Attributes**

| Name | Type | Required |
| --- | --- | --- |
| MustHonor | boolean | no |

## DocumentInputSizeOverrideType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| InputMediaSize | DimensionsOverrideType | 1 | 1 |

## DocumentInputSizeType

- **Kind:** complex

**Attributes**

| Name | Type | Required |
| --- | --- | --- |
| MustHonor | boolean | no |

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| DocumentSizeAutoDetect | BoolExtType | 1 | 1 |
| InputMediaSize | DimensionsType | 1 | 1 |

## DocumentInputSourceBaseType

- **Kind:** simple
- **Value:** `DocumentInputSourceWKVType or KeywordNsExtensionPatternType`

## DocumentInputSourceExtType

- **Kind:** complex
- **Base:** DocumentInputSourceBaseType

## DocumentInputSourceOverrideType

- **Kind:** complex
- **Base:** DocumentInputSourceExtType

**Attributes**

| Name | Type | Required |
| --- | --- | --- |
| Override | boolean | no |
| UsedDefault | boolean | no |

## DocumentInputSourceType

- **Kind:** complex
- **Base:** DocumentInputSourceExtType

**Attributes**

| Name | Type | Required |
| --- | --- | --- |
| MustHonor | boolean | no |

## DocumentParametersType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| Format | DocumentFormatType | 0 | 1 |
| CompressionQualityFactor | CompressionQualityFactorType | 0 | 1 |
| ImagesToTransfer | ImagesToTransferType | 0 | 1 |
| InputSource | DocumentInputSourceType | 0 | 1 |
| FilmScanMode | FilmScanModeType | 0 | 1 |
| ContentType | ContentTypeTicketType | 0 | 1 |
| InputSize | DocumentInputSizeType | 0 | 1 |
| Exposure | ScanExposureType | 0 | 1 |
| Scaling | ScalingType | 0 | 1 |
| Rotation | RotationType | 0 | 1 |
| MediaSides | MediaSidesType | 0 | 1 |
| any | any element | 0 | unbounded |

## DocumentType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| DocumentDescription | DocumentDescriptionType | 1 | 1 |
| any | any element | 0 | unbounded |

## DocumentsType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| DocumentFinalParameters | ActualDocumentParametersType | 1 | 1 |
| Document | DocumentType | 1 | unbounded |
| any | any element | 0 | unbounded |

## ExposureSettingsOverrideType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| Contrast | Adjustment1000OverrideType | 1 | 1 |
| Brightness | Adjustment1000OverrideType | 1 | 1 |
| Sharpness | Adjustment100OverrideType | 1 | 1 |
| any | any element | 0 | unbounded |

## ExposureSettingsType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| Contrast | Adjustment1000Type | 0 | 1 |
| Brightness | Adjustment1000Type | 0 | 1 |
| Sharpness | Adjustment100Type | 0 | 1 |
| any | any element | 0 | unbounded |

## FilmScanModeOverrideType

- **Kind:** complex
- **Base:** FilmScanModeExtType

**Attributes**

| Name | Type | Required |
| --- | --- | --- |
| Override | boolean | no |
| UsedDefault | boolean | no |

## FilmScanModeType

- **Kind:** complex
- **Base:** FilmScanModeExtType

**Attributes**

| Name | Type | Required |
| --- | --- | --- |
| MustHonor | boolean | no |

## ImagesToTransferOverrideType

- **Kind:** complex
- **Base:** IntZeroExtType

**Attributes**

| Name | Type | Required |
| --- | --- | --- |
| Override | boolean | no |
| UsedDefault | boolean | no |

## ImagesToTransferType

- **Kind:** complex
- **Base:** IntZeroExtType

**Attributes**

| Name | Type | Required |
| --- | --- | --- |
| MustHonor | boolean | no |

## MediaSideOverrideType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ScanRegion | ScanRegionOverrideType | 1 | 1 |
| ColorProcessing | ColorOverrideType | 1 | 1 |
| Resolution | ResolutionPairOverrideType | 1 | 1 |
| any | any element | 0 | unbounded |

## MediaSideType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ScanRegion | ScanRegionType | 0 | 1 |
| ColorProcessing | ColorJobType | 0 | 1 |
| Resolution | ResolutionPairExtType | 0 | 1 |
| any | any element | 0 | unbounded |

## MediaSidesOverrideType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| MediaFront | MediaSideOverrideType | 1 | 1 |
| MediaBack | MediaSideOverrideType | 0 | 1 |

## MediaSidesType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| MediaFront | MediaSideType | 1 | 1 |
| MediaBack | MediaSideType | 0 | 1 |

## OffsetExtType

- **Kind:** complex
- **Base:** IntZeroExtType

**Attributes**

| Name | Type | Required |
| --- | --- | --- |
| MustHonor | boolean | no |

## OffsetOverrideType

- **Kind:** complex
- **Base:** IntZeroExtType

**Attributes**

| Name | Type | Required |
| --- | --- | --- |
| Override | boolean | no |
| UsedDefault | boolean | no |

## ResolutionOverrideBaseType

- **Kind:** complex
- **Base:** IntOneExtType

**Attributes**

| Name | Type | Required |
| --- | --- | --- |
| Override | boolean | no |
| UsedDefault | boolean | no |

## ResolutionPairExtType

- **Kind:** complex

**Attributes**

| Name | Type | Required |
| --- | --- | --- |
| MustHonor | boolean | no |

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| Width | IntOneExtType | 1 | 1 |
| Height | IntOneExtType | 0 | 1 |

## ResolutionPairOverrideType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| Width | ResolutionOverrideBaseType | 1 | 1 |
| Height | ResolutionOverrideBaseType | 1 | 1 |

## RotationOverrideType

- **Kind:** complex
- **Base:** RotationExtType

**Attributes**

| Name | Type | Required |
| --- | --- | --- |
| Override | boolean | no |
| UsedDefault | boolean | no |

## RotationType

- **Kind:** complex
- **Base:** RotationExtType

**Attributes**

| Name | Type | Required |
| --- | --- | --- |
| MustHonor | boolean | no |

## ScalingOverrideType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ScalingWidth | ScalingRangeOverrideType | 1 | 1 |
| ScalingHeight | ScalingRangeOverrideType | 1 | 1 |

## ScalingRangeOverrideType

- **Kind:** complex
- **Base:** ScalingRangeType

**Attributes**

| Name | Type | Required |
| --- | --- | --- |
| Override | boolean | no |
| UsedDefault | boolean | no |

## ScalingType

- **Kind:** complex

**Attributes**

| Name | Type | Required |
| --- | --- | --- |
| MustHonor | boolean | no |

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ScalingWidth | ScalingRangeType | 1 | 1 |
| ScalingHeight | ScalingRangeType | 1 | 1 |

## ScanExposureOverrideType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ExposureSettings | ExposureSettingsOverrideType | 1 | 1 |

## ScanExposureType

- **Kind:** complex

**Attributes**

| Name | Type | Required |
| --- | --- | --- |
| MustHonor | boolean | no |

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| AutoExposure | BoolExtType | 1 | 1 |
| ExposureSettings | ExposureSettingsType | 1 | 1 |

## ScanRegionOverrideType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ScanRegionXOffset | OffsetOverrideType | 1 | 1 |
| ScanRegionYOffset | OffsetOverrideType | 1 | 1 |
| ScanRegionWidth | DimensionOverrideBaseType | 1 | 1 |
| ScanRegionHeight | DimensionOverrideBaseType | 1 | 1 |

## ScanRegionType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ScanRegionXOffset | OffsetExtType | 0 | 1 |
| ScanRegionYOffset | OffsetExtType | 0 | 1 |
| ScanRegionWidth | DimensionExtType | 1 | 1 |
| ScanRegionHeight | DimensionExtType | 1 | 1 |
