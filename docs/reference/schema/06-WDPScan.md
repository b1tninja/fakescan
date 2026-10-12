# WDPScan

30 types declared in this schema file, sorted by name.

## CancelJobRequestType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| JobId | IntOneExtType | 1 | 1 |

## CancelJobResponseType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| any | any element | 0 | unbounded |

## CreateScanJobRequestType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ScanIdentifier | String255ExtType | 0 | 1 |
| DestinationToken | String255ExtType | 0 | 1 |
| ScanTicket | ScanTicketType | 1 | 1 |

## CreateScanJobResponseType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| JobId | IntOneExtType | 1 | 1 |
| JobToken | String255ExtType | 1 | 1 |
| ImageInformation | ImageInformationType | 1 | 1 |
| DocumentFinalParameters | ActualDocumentParametersType | 1 | 1 |

## DeviceConditionClearedType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ConditionId | IntOneExtType | 1 | 1 |
| ConditionClearTime | DateTimeExtType | 1 | 1 |
| any | any element | 0 | unbounded |

## ElementChangesType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ScannerDescription | ScannerDescriptionType | 0 | 1 |
| ScannerConfiguration | ScannerConfigurationType | 0 | 1 |
| DefaultScanTicket | ScanTicketType | 0 | 1 |
| any | any element | 0 | unbounded |

## GetActiveJobsRequestType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| any | any element | 0 | unbounded |

## GetActiveJobsResponseType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ActiveJobs | ListOfSummarysType | 1 | 1 |

## GetJobElementsRequestType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| JobId | IntOneExtType | 1 | 1 |
| RequestedElements | RequestedJobElementsType | 1 | 1 |

## GetJobElementsResponseType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| JobElements | JobElementsType | 1 | 1 |

## GetJobHistoryRequestType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| any | any element | 0 | unbounded |

## GetJobHistoryResponseType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| JobHistory | ListOfSummarysType | 1 | 1 |

## GetScannerElementsRequestType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| RequestedElements | RequestedScannerElementsType | 1 | 1 |

## GetScannerElementsResponseType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ScannerElements | ScannerElementsType | 1 | 1 |

## JobEndStateEventType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| JobEndState | JobEndStateType | 1 | 1 |
| any | any element | 0 | unbounded |

## JobEndStateType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| JobId | IntOneExtType | 1 | 1 |
| JobCompletedState | JobStateType | 1 | 1 |
| JobCompletedStateReasons | JobStateReasonsType | 1 | 1 |
| JobName | String255ExtType | 1 | 1 |
| JobOriginatingUserName | String255ExtType | 1 | 1 |
| ScansCompleted | IntZeroExtType | 1 | 1 |
| JobCompletedTime | DateTimeExtType | 0 | 1 |
| any | any element | 0 | unbounded |

## JobStatusEventType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| JobStatus | JobStatusType | 1 | 1 |
| any | any element | 0 | unbounded |

## RetrieveImageRequestType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| JobId | IntOneExtType | 1 | 1 |
| JobToken | String255ExtType | 1 | 1 |
| DocumentDescription | DocumentDescriptionType | 1 | 1 |

## RetrieveImageResponseType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ScanData | ScanDataExtType | 1 | 1 |

## ScanAvailableEventType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ClientContext | String255ExtType | 1 | 1 |
| ScanIdentifier | String255ExtType | 1 | 1 |
| InputSource | DocumentInputSourceExtType | 0 | 1 |
| any | any element | 0 | unbounded |

## ScanDataExtType

- **Kind:** complex
- **Base:** xs:base64Binary

## ScanServiceFaultValuesType

- **Kind:** simple
- **Value:** `wscn:InvalidArgs|wscn:OperationFailed|wscn:ServerErrorTemporaryError|wscn:ServerErrorInternalError|wscn:ServerErrorNotAcceptingJobs|wscn:ClientErrorJobCancelled|wscn:ClientErrorJobIdNotFound|wscn:ClientErrorFormatNotSupported|wscn:ClientErrorInvalidScanIdentifier|wscn:ClientErrorInvalidDestinationToken|wscn:ClientErrorNoImagesAvailable|wscn:ClientErrorInvalidJobToken|wscn:ClientErrorConflictingRequiredParameters`
- **Allowed values:** `wscn:InvalidArgs`, `wscn:OperationFailed`, `wscn:ServerErrorTemporaryError`, `wscn:ServerErrorInternalError`, `wscn:ServerErrorNotAcceptingJobs`, `wscn:ClientErrorJobCancelled`, `wscn:ClientErrorJobIdNotFound`, `wscn:ClientErrorFormatNotSupported`, `wscn:ClientErrorInvalidScanIdentifier`, `wscn:ClientErrorInvalidDestinationToken`, `wscn:ClientErrorNoImagesAvailable`, `wscn:ClientErrorInvalidJobToken`, `wscn:ClientErrorConflictingRequiredParameters`

## ScannerElementsChangeEventType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ElementChanges | ElementChangesType | 1 | 1 |
| any | any element | 0 | unbounded |

## ScannerStatusConditionClearedEventType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| DeviceConditionCleared | DeviceConditionClearedType | 1 | 1 |
| any | any element | 0 | unbounded |

## ScannerStatusConditionEventType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| DeviceCondition | DeviceConditionType | 1 | 1 |
| any | any element | 0 | unbounded |

## ScannerStatusSummaryEventType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| StatusSummary | StatusSummaryType | 1 | 1 |
| any | any element | 0 | unbounded |

## StatusSummaryType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ScannerState | ScannerStateType | 1 | 1 |
| ScannerStateReasons | ScannerStateReasonsType | 0 | 1 |
| any | any element | 0 | unbounded |

## ValidateScanTicketRequestType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ScanTicket | ScanTicketType | 1 | 1 |

## ValidateScanTicketResponseType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ValidationInfo | ValidationInfoType | 1 | 1 |

## ValidationInfoType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ValidTicket | BoolExtType | 1 | 1 |
| ImageInformation | ImageInformationType | 1 | 1 |
| ValidScanTicket | ScanTicketType | 0 | 1 |
