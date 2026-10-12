# Devices Profile for Web Services (DPWS) 1.1

The **glue profile**: which WS-* pieces a constrained network device must implement so Windows (and other DPWS clients) can discover it and fetch topology metadata.

## Spec status

| Question | Answer |
|----------|--------|
| IETF RFC? | **No** |
| Owner | OASIS WS-DD TC |
| Spec | [Devices Profile for Web Services Version 1.1](https://docs.oasis-open.org/ws-dd/dpws/wsdd-dpws-1.1-spec.html) |
| PDF | [cd-04 PDF](https://docs.oasis-open.org/ws-dd/dpws/1.1/cd-04/wsdd-dpws-1.1-spec-cd-04.pdf) |

DPWS **profiles** (constrains) WS-Discovery, WS-Transfer, WS-Eventing, WS-MetadataExchange, SOAP-over-HTTP/UDP — it does **not** define scanner tickets. Scanner semantics stay in [ws-scan.md](ws-scan.md).

## Core model

| Role | Meaning |
|------|---------|
| **DEVICE** | One network endpoint that is a WS-Discovery **Target Service** and a metadata resource |
| **HOSTED SERVICE** | Logical service (Scan, Print, …) described *inside* the DEVICE’s Relationship metadata; has its own HTTP Address |

Only the DEVICE joins discovery multicast. Hosted services must **not** each send Hello (noise).

A DEVICE advertising DPWS includes type `dpws:Device` in discovery Types when Types are present.

## Metadata a client expects (WS-Transfer Get)

After discovery, the client HTTP POSTs WS-Transfer **Get** to the Device EPR (fakescan: `/WebServices/Device`). The body is empty; the response is SOAP with `wsx:Metadata` sections. Common dialects:

| Dialect | Content |
|---------|---------|
| ThisModel | Manufacturer, ModelName, **pnpx:DeviceCategory**, df:DeviceCategory |
| ThisDevice | FriendlyName, FirmwareVersion, SerialNumber |
| Relationship (`…/host`) | **Host** + **Hosted** endpoint references |

Cache invalidation uses discovery **`wsd:MetadataVersion`** (Hello / ProbeMatch / ResolveMatch), not a field inside this Get body (R2001 / R2002).

For a scanner, Hosted must include:

- Address → Scan Service URL (`…/ScannerService`)
- Types → `wscn:ScannerServiceType`
- PnP-X: `pnpx:HardwareId`, `pnpx:CompatibleId` (Windows INF match)

CompatibleId used by inbox WSDScan:

```text
UMB\http://schemas.microsoft.com/windows/2006/08/wdp/scan/ScannerServiceType
```

(fakescan publishes `VEN_FAKE&DEV_WSDSCAN` as HardwareId + that CompatibleId.)

## Binding constraints (practical)

- SOAP 1.2 over HTTP for device and hosted service messages
- Envelope size limits (DPWS `MAX_ENVELOPE_SIZE` ~32K for many control messages; image Retrieve uses MTOM and is larger)
- Hosted services that event must support WS-Eventing with Action-dialect filters (see [ws-eventing.md](ws-eventing.md))

## fakescan mapping

| DPWS idea | fakescan |
|-----------|----------|
| DEVICE EPR / XAddrs | `http://{advertise}:3920/WebServices/Device` |
| HOSTED Scan Service | `http://{advertise}:3920/ScannerService` |
| Get metadata | `short=Get` branch in `wsd/mod.rs` |
| Discovery Target Service | `discovery.rs` Hello / Probe / Resolve |

Windows PnP-X install pipeline after metadata is understood: [windows-client.md](windows-client.md).
