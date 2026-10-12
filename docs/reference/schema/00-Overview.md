# Overview

Reference for the WSD (Windows Device Driver) scan XSD schema — the `Scan` contract of the WSD Scanner Service. It defines the global request/response/event elements and the simple/complex types they use.

- **Target namespace:** `http://schemas.microsoft.com/windows/2006/08/wdp/scan`
- **Global elements:** 26
- **Named types:** 189

**Pages**

- [Overview](00-Overview.md)
- [WellKnownValues (13 types)](01-WKVs-WellKnownValues.md)
- [Common (54 types)](02-Common-Types.md)
- [Document (48 types)](03-Document.md)
- [Job (18 types)](04-Job.md)
- [Scanner (26 types)](05-Scanner.md)
- [WDPScan (30 types)](06-WDPScan.md)

**Global elements**

| Element | Type |
| --- | --- |
| CancelJobRequest | CancelJobRequestType |
| CancelJobResponse | CancelJobResponseType |
| CreateScanJobRequest | CreateScanJobRequestType |
| CreateScanJobResponse | CreateScanJobResponseType |
| DestinationResponses | DestinationResponsesType |
| GetActiveJobsRequest | GetActiveJobsRequestType |
| GetActiveJobsResponse | GetActiveJobsResponseType |
| GetJobElementsRequest | GetJobElementsRequestType |
| GetJobElementsResponse | GetJobElementsResponseType |
| GetJobHistoryRequest | GetJobHistoryRequestType |
| GetJobHistoryResponse | GetJobHistoryResponseType |
| GetScannerElementsRequest | GetScannerElementsRequestType |
| GetScannerElementsResponse | GetScannerElementsResponseType |
| JobEndStateEvent | JobEndStateEventType |
| JobStatusEvent | JobStatusEventType |
| RetrieveImageRequest | RetrieveImageRequestType |
| RetrieveImageResponse | RetrieveImageResponseType |
| ScanAvailableEvent | ScanAvailableEventType |
| ScanDestinations | ScanDestinationsType |
| Scanner | _element_3 |
| ScannerElementsChangeEvent | ScannerElementsChangeEventType |
| ScannerStatusConditionClearedEvent | ScannerStatusConditionClearedEventType |
| ScannerStatusConditionEvent | ScannerStatusConditionEventType |
| ScannerStatusSummaryEvent | ScannerStatusSummaryEventType |
| ValidateScanTicketRequest | ValidateScanTicketRequestType |
| ValidateScanTicketResponse | ValidateScanTicketResponseType |
