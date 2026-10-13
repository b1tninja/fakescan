# Discovery diagnosis: wire side (why Windows does not list fakescan in Add device)

Scope: what fakescan puts on the wire. Windows state (event logs, interfaces, firewall, pairing store) is out of scope here.
Date: 2026-10-12. The live daemon (:3920, uuid ...f2) was only read passively (its log file) and never contacted. The Brother was never contacted.

## 1. Finding that changes the premise: Windows (192.168.1.70) never fetched our metadata

`D:\temp\fakescan-daemon\out.log` (daemon start 01:33Z to 01:50Z) has 622 `/WebServices/Device` + `/ScannerService` requests:

- 618 from **192.168.1.81**: a Get every 5 s, sequential ephemeral source ports 36232..60074. All are inside 32768-60999, i.e. a Linux client, not Windows (49152+).
- 4 from 127.0.0.1 (our own `GetScannerElements` probes).
- **0 from 192.168.1.70**, which is this PC. A Windows host fetching its own XAddrs would show peer=192.168.1.70.

So "Windows SOAP-Gets after Hello" (docs/windows.md lab note) was most likely .81, not WSDAPI on the Add-device PC. If .70's dasHost never Gets, metadata content is not what gates the Add list in the current state: Windows is not acting on the Hello at all, or hides the device earlier (see ranking). Metadata can only matter after a Get happens.

Passive wire checks (read-only UDP listener on 3702 joined to 239.255.255.250 via 192.168.1.70, 14 s and 45 s):

- Our Hello arrives from 192.168.1.70:<ephemeral>, 1161 bytes, every ~5 s (multicast loop and interface selection are fine at the socket level).
- No Probe/Resolve from Windows and no other device's Hello during those windows.
- 3702 is shared (dasHost pid 7644 and an svchost both hold 0.0.0.0:3702), and a third reuse-socket binds and receives multicast fine, so `--probe-listen` can bind here.

## 2. Exact XML emitted (default device)

Hello (captured through `build_hello`; ProbeMatch/ResolveMatch/Bye differ only in To/Action/RelatesTo, same body children):

```
Envelope: soap=2003/05/soap-envelope wsa=2004/08/addressing wsd=2005/04/discovery
          wsdp=2006/02/devprof wscn=http://schemas.microsoft.com/windows/2006/08/wdp/scan
Header:   wsa:To urn:schemas-xmlsoap-org:ws:2005:04:discovery
          wsa:Action http://schemas.xmlsoap.org/ws/2005/04/discovery/Hello
          wsa:MessageID urn:uuid:<v4>
          wsd:AppSequence InstanceId="<boot secs>" MessageNumber="n"
Body:     wsd:Hello
            wsa:EndpointReference/wsa:Address urn:uuid:<uuid>
            wsd:Types "wsdp:Device wscn:ScanDeviceType"
            wsd:XAddrs http://<adv>:<port>/WebServices/Device
            wsd:MetadataVersion <boot secs>
```

No Scopes (Brother omits them). Sent from an ephemeral UDP port, TTL 4, multicast_if = advertise IP, loopback on.

WS-Transfer GetResponse (POST /WebServices/Device): To = anonymous, Action = .../transfer/GetResponse, RelatesTo echoes the request. Body is `wsx:Metadata` with three sections (dialect base `http://schemas.xmlsoap.org/ws/2006/02/devprof/`):

- ThisModel: Manufacturer "fakescan", ManufacturerUrl, ModelName "Fake WSD Scanner", ModelNumber 1, ModelUrl, PresentationUrl, `pnpx:DeviceCategory` Scanners, `df:DeviceCategory` Imaging.Scanner.
- ThisDevice: FriendlyName, FirmwareVersion 0.1.0, SerialNumber FS-0001.
- Relationship Type=.../devprof/host: Host {EPR urn:uuid:<uuid>, Types "wsdp:Device wscn:ScanDeviceType", ServiceId uri:<uuid>}; Hosted {EPR http://<adv>:<port>/ScannerService, Types wscn:ScannerServiceType, ServiceId uri:<uuid>/ScannerService, `pnpx:HardwareId` VEN_FAKE&DEV_WSDSCAN, `pnpx:CompatibleId` http://schemas.microsoft.com/windows/2006/08/wdp/scan/ScannerServiceType}.

HTTP: 200, `application/soap+xml; charset=utf-8`, Content-Length body (axum), 3232 bytes, answered in 1-11 ms (daemon log).

## 3. Comparison with Microsoft's documented PnP-X / DPWS requirements

Sources: Rally Vertical Pairing PnP-X support (Microsoft Learn archive, dn613992), WSD Provider PKEY table (Function Discovery), WS-Discovery specification compliance and AppSequence validation rules (WSDAPI docs).

| Requirement (MS doc) | fakescan | Verdict |
|---|---|---|
| DeviceCategory in ThisModel, `pnpx:` (Vista) and `df:` 2008/09 (Win7+), "Scanners" / "Imaging.Scanner" | both present | matches the MS sample |
| HardwareId, CompatibleId inside Hosted, pnpx 2005/10; HardwareId form `VEN_x&DEV_y` | present, `VEN_FAKE&DEV_WSDSCAN` | matches |
| CompatibleId for scanner `.../wdp/scan/ScannerServiceType` | present | matches (MS sample also appends `GenericUmPass`, optional) |
| ContainerId (ThisDevice), ModelId (ThisModel) | absent | optional per MS |
| ThisDevice FriendlyName/FirmwareVersion/SerialNumber | present | matches |
| Host with EPR, Types, ServiceId; Hosted with EPR, Types, ServiceId | present | matches |
| WSDAPI uses `urn:uuid:` ids | Hello/Matches yes; Get reply used `uuid:` | fixed for Get |
| MetadataVersion only on discovery messages | yes | matches |
| AppSequence: lower InstanceId => XAddrs ignored; lower MessageNumber => message ignored | InstanceId = boot secs, rises across restarts | fine; one same-second gap fixed |
| Message fits one datagram | Hello 1161 B | fine |

No wire-level defect found in the metadata or Hello. This was not compared against a real Brother capture: the repo has none (no Brother Hello, Probe/ResolveMatch or Transfer Get fixture; code comments cite only scan-side captures). That comparison is unverified.

## 4. Ranked differences / candidates

1. Evidence (log + passive listen): the Add-device PC does not Get at all, so the blocker is upstream of metadata. Either dasHost does not act on the Hello, or the device is suppressed (installed/paired UUID, `IsPaired` ghost AEP). docs/e2e-wia-status.md notes an already-associated device is not shown as Installable. Windows-side agent owns this.
2. Likely (inference): Hello-only on Windows. The daemon runs with Probe/Resolve off (log: "Hello-only mode"). docs/windows.md records the one session where the WinRT AEP appeared with CanPair=true: fresh UUID plus `--probe-listen` and ResolveMatch support. The Add-device refresh issues multicast Probe/Resolve, and with no listener nothing answers. 3702 can be bound with reuse here (section 1). Experiment: restart the daemon with `--probe-listen`.
3. Low, fixed: Get reply MessageID was `uuid:...`; WSDAPI documents `urn:uuid:`. No evidence it gates anything.
4. Low, fixed: `boot_sequence_id` could repeat within one second after an in-process remove/add, giving the same InstanceId with MessageNumber back at 1, which WSDAPI drops as stale.
5. Informational, fixed: the log said "no :3702 recv socket (dasHost likely exclusive)" whenever probe-listen was simply off. Reworded.
6. Not a cause on this evidence: missing GenericUmPass/ContainerId/ModelId/Scopes, HTTP framing, response time, Content-Type.

## 5. Changes (wire files only)

- `crates/fakescan/src/wsd/responses.rs`: new `transfer_get_response` (urn:uuid: MessageID); tests `get_response_uses_urn_uuid_message_id_and_echoes_relates_to` and `get_metadata_places_pnpx_elements_where_wsdapi_reads_them`.
- `crates/fakescan/src/wsd/mod.rs`: the Get arm uses it. Device identity and scan capabilities are untouched.
- `crates/fakescan/src/discovery.rs`: strictly increasing `boot_sequence_id`, clearer Hello-only log, tests `boot_sequence_id_is_strictly_increasing_and_fits_xs_unsigned_int` and `hello_matches_wsd_message_shape`.
- `cargo test -p fakescan` (CARGO_TARGET_DIR=D:\temp\fakescan-target-wire): lib 304, multi_device 8, profile_load 4, registry_install 16, soap_unexpected 11, ui 8, wsd_scan_flow 31, all pass.

## 6. Live daemon step (user applies)

Rebuild and restart the daemon, adding `--probe-listen` to its command line to test candidate 2. Then watch the log for `WS-Discovery request ... kind=ProbeMatch/ResolveMatch` and for a Get with peer=192.168.1.70.

## 7. Could not verify

- A throwaway instance on :3944 would not stay up: `fakescan.exe` shuts down immediately when started without the tray (log "shutting down" right after "named pipe server starting"), also with `--manage`. That is in main.rs/server.rs (not mine). So no live HTTP header capture and no `fakescan.ps1 discoverable` run. The Get response was rendered through unit code only.
- Whether dasHost on .70 receives and accepts the Hello (needs Windows-side evidence).
- Real Brother metadata comparison (no capture in repo; Brother not contacted).
