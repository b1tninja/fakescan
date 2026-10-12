# WS-Discovery

How clients **find** the Device on the LAN before any HTTP Scan call.

## Spec status

| Question | Answer |
|----------|--------|
| IETF RFC? | **No** (UDP port **3702** is IANA-registered for WS-Discovery) |
| Wire namespace Windows uses | Often `http://schemas.xmlsoap.org/ws/2005/04/discovery` (WSDAPI) |
| OASIS Standard | [WS-Discovery Version 1.1](https://docs.oasis-open.org/ws-dd/discovery/1.1/os/wsdd-discovery-1.1-spec-os.html) (`http://docs.oasis-open.org/ws-dd/ns/discovery/2009/01`) |
| Earlier PDF | [WS-Discovery 2005/04](https://specs.xmlsoap.org/ws/2005/04/discovery/ws-discovery.pdf) |
| Windows notes | [WS-Discovery specification compliance](https://learn.microsoft.com/en-us/windows/win32/wsdapi/ws-discovery-specification-compliance), [inspecting UDP traces](https://learn.microsoft.com/en-us/windows/win32/wsdapi/inspecting-network-traces-for-udp-ws-discovery) |

DPWS requires a DEVICE to be a compliant Target Service ([dpws.md](dpws.md)).

## Transport assignments (ad hoc IP)

| Parameter | Value |
|-----------|--------|
| Port | **UDP 3702** |
| IPv4 multicast | `239.255.255.250` |
| IPv6 link-local | `FF02::C` |
| Framing | SOAP-over-UDP |

## Message types

| Message | Direction | Purpose |
|---------|-----------|---------|
| **Hello** | Device → multicast | “I joined” (Types, Scopes, XAddrs, MetadataVersion) |
| **Bye** | Device → multicast | “I left” |
| **Probe** | Client → multicast (or HTTP per DPWS) | Search by Types / Scopes |
| **ProbeMatches** | Device → **unicast** to Probe source | Match list + XAddrs |
| **Resolve** | Client → multicast | Find by endpoint reference (UUID) when XAddrs missing |
| **ResolveMatches** | Device → **unicast** | Address for that EPR |

Microsoft guidance: if ProbeMatches omits XAddrs, the client **Resolve**s; ResolveMatches should arrive within ~4s or the firewall may drop it.

## Scanner Types on the wire

Brother-shaped devices announce **device-level** scan type in Hello/ProbeMatch (e.g. `wscn:ScanDeviceType`), not only `ScannerServiceType`. The hosted service type appears in Relationship metadata ([dpws.md](dpws.md)).

fakescan Hello uses a Brother-like device type; Hosted metadata advertises `ScannerServiceType`.

## Same-host Windows pitfall

On Windows, **`dasHost`** often binds UDP 3702 exclusively. fakescan:

- Always **sends** Hello/Bye (and ProbeMatch/ResolveMatch when it can receive)
- Optionally `--probe-listen` to bind `:3702` with reuse; may still be Hello-only if dasHost wins

Lab: Hello + SOAP Get is enough for WinRT AEP / Add device even when `probes_answered=0`. See [windows.md](../windows.md).

## Docker / multicast

Bridge networks (Docker Desktop) frequently break UDP multicast → verdict **soap-only**. Use host networking (Linux), a LAN VM, or native `fakescan.exe --advertise <LAN-IP>`. SOAP on TCP 3920 alone does not install into WIA.

## AppSequence / MetadataVersion (restart)

WS-Discovery requires `AppSequence/@InstanceId` to increase when the target service goes down and comes back ([WSD_APP_SEQUENCE](https://learn.microsoft.com/en-us/windows/win32/api/wsdtypes/ns-wsdtypes-wsd_app_sequence)). WSDAPI [ignores](https://learn.microsoft.com/en-us/windows/win32/wsdapi/appsequence-validation-rules) Hellos whose `MessageNumber` is below the last seen value for that device, and ignores XAddrs when `InstanceId` or `MetadataVersion` goes backwards.

fakescan therefore:

| Field | Behavior |
|-------|----------|
| `InstanceId` | Seconds since UNIX epoch at process start (not hardcoded `1`) |
| `MetadataVersion` | Same as `InstanceId` at start (MAY bump on restart; never decreases) |
| Startup Hellos | Burst of 4 (0 / +50 / +200 / +500 ms), then every 5s |

`MetadataVersion` belongs in Hello / ProbeMatch / ResolveMatch only — **not** in WS-Transfer Get metadata ([dpws.md](dpws.md)). Lab dumps that omit it from Get are correct.

Sticky WIA offline after restart (PnP OK, no `/ScannerService` POST): see [windows-client.md](windows-client.md#presence-sticky-offline--open-questions).

## fakescan mapping

| Message | Code |
|---------|------|
| Announcer + Bye | `crates/fakescan/src/discovery.rs` |
| Counters | `/v1/status` → `wsd_hellos_sent`, `wsd_probes_answered`, `wsd_instance_id`, `wsd_metadata_version`, … |
| Stable EPR | `--device-uuid` (PnP sees a new device if UUID changes) |
