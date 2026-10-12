# Job

18 types declared in this schema file, sorted by name.

## JobDescriptionType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| JobName | String255ExtType | 1 | 1 |
| JobOriginatingUserName | String255ExtType | 1 | 1 |
| JobInformation | String255ExtType | 0 | 1 |
| any | any element | 0 | unbounded |

## JobElementDataType

- **Kind:** complex

**Attributes**

| Name | Type | Required |
| --- | --- | --- |
| Name | QName | yes |
| Valid | boolean | yes |

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| JobStatus | JobStatusType | 1 | 1 |
| ScanTicket | ScanTicketType | 1 | 1 |
| Documents | DocumentsType | 1 | 1 |
| any | any element | 0 | unbounded |

## JobElementsEntryType

- **Kind:** simple
- **Value:** `JobElementsEntryWKVType or KeywordNsExtensionPatternType`

## JobElementsEntryWKVType

- **Kind:** simple
- **Value:** `JobStatus|ScanTicket|Document`
- **Allowed values:** `JobStatus`, `ScanTicket`, `Document`

## JobElementsType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ElementData | JobElementDataType | 1 | unbounded |

## JobStateBaseType

- **Kind:** simple
- **Value:** `JobStateWKVType or KeywordNsExtensionPatternType`

## JobStateReasonBaseType

- **Kind:** simple
- **Value:** `JobStateReasonWKVType or KeywordNsExtensionPatternType`

## JobStateReasonType

- **Kind:** complex
- **Base:** JobStateReasonBaseType

## JobStateReasonsType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| JobStateReason | JobStateReasonType | 1 | unbounded |

## JobStateType

- **Kind:** complex
- **Base:** JobStateBaseType

## JobStatusType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| JobId | IntOneExtType | 1 | 1 |
| JobState | JobStateType | 1 | 1 |
| JobStateReasons | JobStateReasonsType | 0 | 1 |
| ScansCompleted | IntZeroExtType | 1 | 1 |
| JobCreatedTime | DateTimeExtType | 0 | 1 |
| JobCompletedTime | DateTimeExtType | 0 | 1 |
| any | any element | 0 | unbounded |

## JobSummaryType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| JobId | IntOneExtType | 1 | 1 |
| JobName | String255ExtType | 1 | 1 |
| JobOriginatingUserName | String255ExtType | 1 | 1 |
| JobState | JobStateType | 1 | 1 |
| JobStateReasons | JobStateReasonsType | 0 | 1 |
| ScansCompleted | IntZeroExtType | 1 | 1 |
| any | any element | 0 | unbounded |

## JobTableType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| ActiveJobs | ListOfJobsType | 1 | 1 |
| JobHistory | ListOfJobsType | 0 | 1 |
| any | any element | 0 | unbounded |

## JobType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| JobStatus | JobStatusType | 1 | 1 |
| ScanTicket | ScanTicketType | 1 | 1 |
| Documents | DocumentsType | 0 | 1 |
| any | any element | 0 | unbounded |

## ListOfJobsType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| Job | JobType | 0 | unbounded |
| any | any element | 0 | unbounded |

## ListOfSummarysType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| JobSummary | JobSummaryType | 0 | unbounded |

## RequestedJobElementsType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| Name | JobElementsEntryType | 1 | unbounded |

## ScanTicketType

- **Kind:** complex

**Child elements**

| Name | Type | Min | Max |
| --- | --- | --- | --- |
| JobDescription | JobDescriptionType | 1 | 1 |
| DocumentParameters | DocumentParametersType | 0 | 1 |
| any | any element | 0 | unbounded |
