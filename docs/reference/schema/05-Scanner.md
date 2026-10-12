# Scanner

26 types declared in this schema file, sorted by name.

## ActiveConditionsTableType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| DeviceCondition | DeviceConditionType | 0 | unbounded |

## ConditionComponentBaseType

- **Kind:** simple
- **Value:** `ADF|Film|MediaPath|Platen`
- **Allowed values:** `ADF`, `Film`, `MediaPath`, `Platen`

## ConditionComponentType

- **Kind:** complex
- **Base:** ConditionComponentBaseType

## ConditionHistoryEntryType

- **Kind:** complex
- **Base:** DeviceConditionType

## ConditionHistoryTableType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ConditionHistoryEntry | ConditionHistoryEntryType | 1 | unbounded |

## ConditionNameBaseType

- **Kind:** simple
- **Value:** `Calibrating|CoverOpen|InputTrayEmpty|InterlockOpen|InternalStorageFull|MediaJam|LampError|LampWarning|MultipleFeedError`
- **Allowed values:** `Calibrating`, `CoverOpen`, `InputTrayEmpty`, `InterlockOpen`, `InternalStorageFull`, `MediaJam`, `LampError`, `LampWarning`, `MultipleFeedError`

## ConditionNameType

- **Kind:** complex
- **Base:** ConditionNameBaseType

## ConditionSeverityBaseType

- **Kind:** simple
- **Value:** `Informational|Warning|Critical`
- **Allowed values:** `Informational`, `Warning`, `Critical`

## ConditionSeverityType

- **Kind:** complex
- **Base:** ConditionSeverityBaseType

## DestinationResponseBaseType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ClientContext | String255ExtType | 1 | 1 |
| DestinationToken | String255ExtType | 1 | 1 |
| any | any element | 0 | unbounded |

## DestinationResponsesType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| DestinationResponse | DestinationResponseBaseType | 1 | unbounded |

## DeviceConditionType

- **Kind:** complex

**Attributes**

| Name | Type | Required |
| --- | --- | --- |
| Id | int>=1 | yes |

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| Time | DateTimeExtType | 1 | 1 |
| Name | ConditionNameType | 1 | 1 |
| Component | ConditionComponentType | 1 | 1 |
| Severity | ConditionSeverityType | 1 | 1 |
| any | any element | 0 | unbounded |

## RequestedScannerElementsType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| Name | xs:QName | 1 | unbounded |

## ScanDestinationBaseType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ClientDisplayName | String127ExtType | 1 | 1 |
| ClientContext | String255ExtType | 1 | 1 |
| any | any element | 0 | unbounded |

## ScanDestinationsType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ScanDestination | ScanDestinationBaseType | 1 | unbounded |

## ScannerConfigurationType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| DeviceSettings | DeviceSettingsType | 1 | 1 |
| Platen | PlatenType | 0 | 1 |
| ADF | ADFType | 0 | 1 |
| Film | FilmType | 0 | 1 |
| any | any element | 0 | unbounded |

## ScannerDescriptionType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ScannerName | String127ExtType | 1 | unbounded |
| ScannerInfo | String127ExtType | 0 | unbounded |
| ScannerLocation | String127ExtType | 0 | unbounded |
| any | any element | 0 | unbounded |

## ScannerElementDataType

- **Kind:** complex

**Attributes**

| Name | Type | Required |
| --- | --- | --- |
| Name | QName | yes |
| Valid | boolean | yes |

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ScannerDescription | ScannerDescriptionType | 1 | 1 |
| ScannerConfiguration | ScannerConfigurationType | 1 | 1 |
| ScannerStatus | ScannerStatusType | 1 | 1 |
| DefaultScanTicket | ScanTicketType | 1 | 1 |
| any | any element | 0 | unbounded |

## ScannerElementsType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ElementData | ScannerElementDataType | 1 | unbounded |

## ScannerStateBaseType

- **Kind:** simple
- **Value:** `ScannerStateWKVType or KeywordNsExtensionPatternType`

## ScannerStateReasonBaseType

- **Kind:** simple
- **Value:** `ScannerStateReasonWKVType or KeywordNsExtensionPatternType`

## ScannerStateReasonType

- **Kind:** complex
- **Base:** ScannerStateReasonBaseType

## ScannerStateReasonsType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ScannerStateReason | ScannerStateReasonType | 1 | unbounded |

## ScannerStateType

- **Kind:** complex
- **Base:** ScannerStateBaseType

## ScannerStatusType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ScannerCurrentTime | DateTimeExtType | 1 | 1 |
| ScannerState | ScannerStateType | 1 | 1 |
| ScannerStateReasons | ScannerStateReasonsType | 0 | 1 |
| ActiveConditions | ActiveConditionsTableType | 1 | 1 |
| ConditionHistory | ConditionHistoryTableType | 0 | 1 |
| any | any element | 0 | unbounded |

## ScannerType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ScannerDescription | ScannerDescriptionType | 1 | 1 |
| ScannerConfiguration | ScannerConfigurationType | 1 | 1 |
| ScannerStatus | ScannerStatusType | 1 | 1 |
| DefaultScanTicket | ScanTicketType | 1 | 1 |
| any | any element | 0 | unbounded |
