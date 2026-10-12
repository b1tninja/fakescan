# Protocol reference (WSD / WS-Scan stack)

**None of this is an IETF RFC.** The scanner ops are Microsoft’s **WDP Scan / WS-Scan**. Discovery, metadata, and eventing are OASIS / W3C WS-* specs (and earlier XML SOAP submissions) that DPWS profiles together.

```text
                          Windows PC
   ┌──────────────────────────────────────────────────────────┐
   │  pdfcraft / WIA app                                      │
   │       │                                                  │
   │       ▼                                                  │
   │  WIA service  ←── WSDScan.sys (inbox)                    │
   │       │                                                  │
   │  PnP-X association (Add device / IPBusEnum / UMBus)      │
   └───────┼──────────────────────────────────────────────────┘
           │ SOAP over HTTP (+ MTOM for images)
           │ UDP 3702 WS-Discovery
           ▼
   ┌──────────────────────────────────────────────────────────┐
   │  fakescan                                                │
   │  /WebServices/Device   WS-Transfer Get  (DPWS metadata)  │
   │  /ScannerService       WS-Scan ops + WS-Eventing         │
   │  UDP :3702             Hello / ProbeMatch / ResolveMatch │
   └──────────────────────────────────────────────────────────┘
```

## Documents in this folder

| Doc | Spec | What it covers |
|-----|------|----------------|
| [ws-scan.md](ws-scan.md) | Microsoft Scan Service Definition V1.0 | Scan SOAP ops, namespaces, tickets, MTOM |
| [mtom-retrieveimage.md](mtom-retrieveimage.md) | MS RetrieveImageResponse + XOP/MTOM | packaging A/B closed; JPEG payload next vs `mtom_response` |
| [dpws.md](dpws.md) | OASIS DPWS 1.1 | Device vs hosted service, metadata shape |
| [ws-discovery.md](ws-discovery.md) | OASIS WS-Discovery 1.1 (+ 2005/04 wire) | Hello / Probe / Resolve, UDP 3702 |
| [ws-transfer.md](ws-transfer.md) | W3C Submission WS-Transfer | `Get` on the Device EPR |
| [ws-eventing.md](ws-eventing.md) | W3C Submission WS-Eventing | `Subscribe` (required by WIA open) |
| [windows-client.md](windows-client.md) | Microsoft WIA + WSDScan.sys | How Windows ties the stack to Imaging Devices |

## Authoritative downloads

| Artifact | Where |
|----------|--------|
| **Scan Service Definition V1.0** (Word + WSDL/XSD, ~1.5 MB) | [Implementing Web Services on Devices for Printing](https://learn.microsoft.com/en-us/windows-hardware/design/whitepapers/implementing-web-services-on-devices-for-printing) (same page hosts Scan Device Definition V1.0) |
| HTML op catalog | [WSD Scan Service Operation Elements](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/wsd-scan-service-operation-elements) |
| DPWS 1.1 | [docs.oasis-open.org/ws-dd/dpws](https://docs.oasis-open.org/ws-dd/dpws/wsdd-dpws-1.1-spec.html) |
| WS-Discovery 1.1 | [docs.oasis-open.org/ws-dd/discovery](https://docs.oasis-open.org/ws-dd/discovery/1.1/os/wsdd-discovery-1.1-spec-os.html) |
| WS-Transfer | [W3C Submission](https://www.w3.org/Submission/WS-Transfer/) |
| WS-Eventing | [W3C Submission 2004/08](https://www.w3.org/Submission/WS-Eventing/) (Windows wire) / [W3C TR](https://www.w3.org/TR/ws-eventing/) (later) |

## Not this stack

- **PWG Semantic Model / IPP Scan** — different schema (`ScanService.xsd` at pwg.org). Do not mix with `wscn:` WDP Scan.
- **eSCL / AirPrint** — Apple/Mozilla network scan; sane-airscan speaks both eSCL and WS-Scan.
- **TWAIN / USB WIA minidriver** — local bus; see [prior-art.md](../prior-art.md).

## fakescan status (quick)

See also [architecture.md](../architecture.md). Operator checklist: [runbook-fake-scanner.md](../runbook-fake-scanner.md). Lab install/acquire notes: [windows.md](../windows.md), [e2e-wia-status.md](../e2e-wia-status.md).
