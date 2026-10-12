# Common

54 types declared in this schema file, sorted by name.

## ADFEntryType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ADFOpticalResolution | ResolutionPairType | 1 | 1 |
| ADFResolutions | ResolutionsType | 1 | 1 |
| ADFColor | ColorEntriesType | 1 | 1 |
| ADFMinimumSize | DimensionsType | 1 | 1 |
| ADFMaximumSize | DimensionsType | 1 | 1 |
| any | any element | 0 | unbounded |

## ADFType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ADFSupportsDuplex | BoolExtType | 1 | 1 |
| ADFFront | ADFEntryType | 1 | 1 |
| ADFBack | ADFEntryType | 0 | 1 |
| any | any element | 0 | unbounded |

## Adjustment1000BaseType

- **Kind:** simple
- **Value:** `int>=-1000`

## Adjustment1000Type

- **Kind:** complex
- **Base:** Adjustment1000BaseType

## Adjustment100BaseType

- **Kind:** simple
- **Value:** `int>=-100`

## Adjustment100Type

- **Kind:** complex
- **Base:** Adjustment100BaseType

## AnyURIExtType

- **Kind:** complex
- **Base:** xs:anyURI

## BoolExtType

- **Kind:** complex
- **Base:** xs:boolean

## ColorConfigBaseType

- **Kind:** simple
- **Value:** `ColorWKVType or KeywordNsExtensionPatternType`

## ColorConfigType

- **Kind:** complex
- **Base:** ColorConfigBaseType

## ColorEntriesType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ColorEntry | ColorConfigType | 1 | unbounded |

## CompressionQualityFactorBaseType

- **Kind:** simple
- **Value:** `int>=0`

## CompressionQualityFactorExtType

- **Kind:** complex
- **Base:** CompressionQualityFactorBaseType

## CompressionQualityFactorSupportedType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| MinValue | CompressionQualityFactorExtType | 1 | 1 |
| MaxValue | CompressionQualityFactorExtType | 1 | 1 |

## ContentTypeBaseType

- **Kind:** simple
- **Value:** `ContentTypeWKVType or KeywordNsExtensionPatternType`

## ContentTypeExtType

- **Kind:** complex
- **Base:** ContentTypeBaseType

## ContentTypesSupportedType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ContentTypeValue | ContentTypeExtType | 1 | unbounded |

## DateTimeExtType

- **Kind:** complex
- **Base:** xs:dateTime

## DeviceSettingsType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| FormatsSupported | FormatSupportedType | 1 | 1 |
| CompressionQualityFactorSupported | CompressionQualityFactorSupportedType | 1 | 1 |
| ContentTypesSupported | ContentTypesSupportedType | 1 | 1 |
| DocumentSizeAutoDetectSupported | BoolExtType | 1 | 1 |
| AutoExposureSupported | BoolExtType | 1 | 1 |
| BrightnessSupported | BoolExtType | 1 | 1 |
| ContrastSupported | BoolExtType | 1 | 1 |
| ScalingRangeSupported | ScalingRangeSupportedType | 1 | 1 |
| RotationsSupported | RotationsSupportedType | 1 | 1 |
| any | any element | 0 | unbounded |

## DimensionsType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| Width | IntOneExtType | 1 | 1 |
| Height | IntOneExtType | 1 | 1 |

## DocumentFormatBaseType

- **Kind:** simple
- **Value:** `DocumentFormatWKVType or MimeExtensionPatternType`

## DocumentFormatExtType

- **Kind:** complex
- **Base:** DocumentFormatBaseType

## FilmScanModeBaseType

- **Kind:** simple
- **Value:** `FilmScanModeWKVType or KeywordNsExtensionPatternType`

## FilmScanModeExtType

- **Kind:** complex
- **Base:** FilmScanModeBaseType

## FilmScanModesSupportedType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| FilmScanModeValue | FilmScanModeExtType | 1 | unbounded |

## FilmType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| FilmScanModesSupported | FilmScanModesSupportedType | 1 | 1 |
| FilmOpticalResolution | ResolutionPairType | 1 | 1 |
| FilmResolutions | ResolutionsType | 1 | 1 |
| FilmColor | ColorEntriesType | 1 | 1 |
| FilmMinimumSize | DimensionsType | 1 | 1 |
| FilmMaximumSize | DimensionsType | 1 | 1 |
| any | any element | 0 | unbounded |

## FormatSupportedType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| FormatValue | DocumentFormatExtType | 1 | unbounded |

## ImageInformationType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| MediaFrontImageInfo | RegionImageInformationBaseType | 1 | 1 |
| MediaBackImageInfo | RegionImageInformationBaseType | 0 | 1 |
| any | any element | 0 | unbounded |

## IntExtType

- **Kind:** complex
- **Base:** xs:int

## IntOneBaseType

- **Kind:** simple
- **Value:** `int>=1`

## IntOneExtType

- **Kind:** complex
- **Base:** IntOneBaseType

## IntZeroBaseType

- **Kind:** simple
- **Value:** `int>=0`

## IntZeroExtType

- **Kind:** complex
- **Base:** IntZeroBaseType

## NMTOKENSExtType

- **Kind:** complex
- **Base:** xs:NMTOKENS

## PlatenType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| PlatenOpticalResolution | ResolutionPairType | 1 | 1 |
| PlatenResolutions | ResolutionsType | 1 | 1 |
| PlatenColor | ColorEntriesType | 1 | 1 |
| PlatenMinimumSize | DimensionsType | 1 | 1 |
| PlatenMaximumSize | DimensionsType | 1 | 1 |
| any | any element | 0 | unbounded |

## QNameExtType

- **Kind:** complex
- **Base:** xs:QName

## RegionImageInformationBaseType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| PixelsPerLine | IntOneExtType | 1 | 1 |
| NumberOfLines | IntOneExtType | 1 | 1 |
| BytesPerLine | IntZeroExtType | 1 | 1 |
| any | any element | 0 | unbounded |

## ResolutionHeightsType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| Height | IntOneExtType | 1 | unbounded |

## ResolutionPairType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| Width | IntOneExtType | 1 | 1 |
| Height | IntOneExtType | 0 | 1 |

## ResolutionWidthsType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| Width | IntOneExtType | 1 | unbounded |

## ResolutionsType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| Widths | ResolutionWidthsType | 1 | 1 |
| Heights | ResolutionHeightsType | 1 | 1 |

## RotationBaseType

- **Kind:** simple
- **Value:** `RotationWKVType or KeywordNsExtensionPatternType`

## RotationExtType

- **Kind:** complex
- **Base:** RotationBaseType

## RotationsSupportedType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| RotationValue | RotationExtType | 1 | unbounded |

## ScalingRangeBaseType

- **Kind:** simple
- **Value:** `int>=1`

## ScalingRangeSupportedType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ScalingWidth | _element_1 | 1 | 1 |
| ScalingHeight | _element_2 | 1 | 1 |

## ScalingRangeType

- **Kind:** complex
- **Base:** ScalingRangeBaseType

## String127BaseType

- **Kind:** simple
- **Value:** `string(127)`

## String127ExtType

- **Kind:** complex
- **Base:** String127BaseType

## String255BaseType

- **Kind:** simple
- **Value:** `string(255)`

## String255ExtType

- **Kind:** complex
- **Base:** String255BaseType

## ValueIntListType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| AllowedValue | IntExtType | 1 | unbounded |

## ValueIntRangeType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| MinValue | IntExtType | 0 | 1 |
| MaxValue | IntExtType | 1 | 1 |

## ValueTokenListType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| AllowedValue | NMTOKENSExtType | 1 | unbounded |
