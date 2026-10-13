# Windows client stack (WIA + WSDScan)

How the OS turns a network Scan Service into something **pdfcraft** can open via WIA.

## Spec status

Not a single standard — Microsoft product documentation:

| Topic | Link |
|-------|------|
| WIA + WSD overview | [WIA with Web Services for Devices](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/wia-with-web-services-for-devices) |
| Inbox kernel piece | `WSDScan.sys` + `wsdscdrv.inf` (`STI.WSDSection`) |
| Install with WSD | [Installing a WIA Scanner Driver with WSD](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/installing-a-wia-scanner-driver-with-wsd) |
| Sample vendor INF | [Sample INF for a Web Services Scanner](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/sample-inf-file-for-a-web-services-scanner) |
| SOAP from a USD | [SOAP Communications for WIA Scanners](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/soap-communications-for-wia-scanners) |
| PnP-X association API | [IPNPXDeviceAssociation](https://learn.microsoft.com/en-us/windows/win32/api/pnpxassoc/nn-pnpxassoc-ipnpxdeviceassociation) |

## End-to-end path

```text
WS-Discovery Hello/Probe
        ↓
Function Discovery / WinRT AEP  (candidate)
        ↓
Person Add device  (or IPNPXAssociation::Associate when FD instance exists)
        ↓
Association DB → IPBusEnum → UMBus
        ↓
SWD\DAFWSDPROVIDER\urn:uuid:…/ScannerService   (Image class)
        ↓
PnP picks INF: CompatibleId → inbox wsdscdrv.inf → WSDScan.sys
        ↓
WIA enumerates scanner; open → SOAP Subscribe + GetScannerElements + …
        ↓
CreateScanJob / RetrieveImage (MTOM)
```

Lab details and dead ends (registry, custom INF myths, Docker): [windows.md](../windows.md).

## What is *not* the scanner API

| Mechanism | Role |
|-----------|------|
| Custom INF matching `UMB\VEN_…` | Optional **vendor WIA USD** package after the node exists; inbox CompatibleId already loads WSDScan |
| Writing `Enum\SWD\DAFWSDPROVIDER` | Effect of install, not a create API |
| WSDMON `AssocDevice` | **Printers** only (spooler), not scanners |
| `pnputil /add-driver wsdscdrv.inf` | Inbox already present; no association ⇒ no device |

## Online vs Ready vs Installed

Three different truths; do not conflate them:

| Word | Where it shows | Means |
|------|----------------|-------|
| **Installed** | PnP Image / `SWD\DAFWSDPROVIDER\…`, `.\scripts\fakescan.ps1 wia` / `swd` | Association happened (person **Add**). Inbox `wsdscdrv.inf` attached. Device *exists*. |
| **Ready** (Settings) | Printers & scanners UI | UI convenience only — **not** proof WIA can open or that SOAP is flowing. |
| **Online** (WIA) | WIA Connect succeeds; `-v` shows Subscribe + GetScannerElements | Client reached `/ScannerService`, Subscribe accepted (with **DestinationResponses** when asked), status Idle/None. Acquire can proceed. |

Settings can show **Ready** while:

- PnP Status is Started, but
- WIA `Connect` fails with offline (`0x80210005`), or
- `IWiaDevMgr2` enum omits the device, or
- **no SOAP** hits fakescan at all (sticky stale association after process restart)

Often correlated with SOAP faults on **Subscribe**, empty-feeder misuse of `AttentionRequired` (fixed — use Idle/None), Device URL unreachable after fakescan restart, or UUID/advertise change. Watch fakescan `-v` logs: peer, path, `short=Subscribe|GetScannerElements|CreateScanJob`.

### Presence sticky offline — open questions

After **fakescan restart**, Windows may keep the Imaging node (`Status=OK IsPresent=True`) while WIA Connect returns **`0x80210005`** and sends **no** POST to `/ScannerService`. Peers often keep WS-Transfer Get on `/WebServices/Device` (~5s). **`stisvc` / DeviceAssociationService restart has not cleared it in lab**; elevated `pnputil` remove + person **re-Add** recovers. That bounce was the experiment for *this* state, not for a ghost AEP. Checklist: [runbook-fake-scanner.md](../runbook-fake-scanner.md). Lab notes: [e2e-wia-status.md](../e2e-wia-status.md).

### Ghost AEP after PnP remove

`pnputil /remove-device` clears the Imaging / `SWD\DAFWSDPROVIDER` node. It does **not** unpair the WinRT WSD AssociationEndpoint. Then:

- AEP stays `IsPaired=True`
- Settings **Add a printer or scanner** lists nothing for Fake (paired devices are not “addable”)
- `UnpairAsync` returns **AccessDenied** even from an elevated helper ([DeviceUnpairingResultStatus.AccessDenied](https://learn.microsoft.com/uwp/api/windows.devices.enumeration.deviceunpairingresultstatus) = missing pairing permission, not missing UAC)

**fdPHost was never bounced for this in lab.** Layered recovery (`rebind` / `bounce-fd`):

1. `pnputil` remove (already done)
2. WinRT `UnpairAsync` (or WDK/Win11 [`PairTool /disassociate`](https://learn.microsoft.com/windows-hardware/drivers/devtest/pairtool-command-syntax) if present)
3. If still AccessDenied: restart **fdPHost** (WSD Function Discovery cache — [KB 973560](https://support.microsoft.com/kb/973560)). For **DeviceAssociationService**, only **Start** if it is Stopped — never `Restart-Service -Force` (lab 2026-10-12: that left DAS in **StopPending**, hung WinRT AEP, and hid Add until the lone DAS svchost was killed). Lab cleared a ghost AEP after **fdPHost alone**, then WinRT Unpair succeeded. Restore **FDResPub** only if the fdPHost `-Force` stop knocked it; FDResPub publishes *this* PC, it is not the scan-discovery cache.
4. **Never** restart **dasHost** — it owns UDP 3702; bouncing it drops Hellos/Probes for fakescan and fakeprint.
5. Last resort: new `--device-uuid` so Windows is pairing a different AEP id (lab `…f6` unstuck Add when Unpair stayed denied).

`stisvc` is WIA enum only; bounce it only after a successful re-Add if Connect still sees a stale Imaging item.

**Transfer Get does not need `MetadataVersion` in the XML.** DPWS puts version only on Hello / ProbeMatch / ResolveMatch; clients refresh Get when that rises ([dpws.md](dpws.md) R2001). Lab dumps omitting it from Get are correct.

**Mitigation shipped (needs live proof — do not claim fixed):** discovery bumps `AppSequence/@InstanceId` and `MetadataVersion` from boot time each process start, plus a short Hello burst ([ws-discovery.md](ws-discovery.md)). Fixed `InstanceId="1"` with MessageNumber reset after a long prior run matches WSDAPI’s [stale MessageNumber / XAddrs rules](https://learn.microsoft.com/en-us/windows/win32/wsdapi/appsequence-validation-rules).

**Subscribe / Transfer (code-locked; live residual separate):**

| Question | Notes |
|----------|--------|
| Subscribe Filter dialects | **done** — accept; `@Dialect` logged; filter body not evaluated (ignore unknown) |
| SubscriptionManager EPR | **done** — ScannerService Address + Identifier (flow-tested) |
| Expires format | **done** — PT / `P1D` / `P1DT…` / UTC `Z`/`+00:00` (flow-tested) |
| Hosted-service GetMetadata | **done** — same WS-Transfer Get dispatcher on `/ScannerService` and `/WebServices/Device` (Device MEX). Windows lab only polls Device today |
| Transport “online” bit | **open (live)** — PnP + Device Get OK yet WIA never opens Scan after restart; association / WSDScan outside fakescan ([e2e-wia-status.md](../e2e-wia-status.md)) |
| Gray Create → no Retrieve | **fixed, live-proven 2026-10-12** — cause was the RGB24 FinalParams coercion; default `jfif_gray=gray8` answers Grayscale8 + BPL width + 1-component JPEG (`rgb24` is a legacy opt-in) |

**Suggested live experiments**

1. Capture Hello before/after restart: `InstanceId` and `MetadataVersion` must increase; dasHost should still Transfer-Get. Without Wireshark, `GET /v1/status` exposes `wsd_instance_id` and `wsd_metadata_version` (same values as AppSequence / Hello) for A/B across restarts.
2. After restart, WIA Connect / `pdfcraft-cli scan --device "Fake WSD scanner"` **without** remove+Add; watch for any `/ScannerService` POST.
3. A/B old binary (`InstanceId=1`) vs new; same UUID/`--advertise`.
4. If still sticky: Wireshark Resolve after Hello; optionally omit Hello XAddrs temporarily (dev only) to force Resolve.
5. When Connect *does* hit ScannerService, diff Brother vs fakescan Subscribe (Filter, SubscriptionManager, Expires, DestinationResponses).

## RetrieveImage request shape (wsdscdrv)

Live Windows client (inbox WSDScan / `wsdscdrv`) POST body, lab dump `D:\temp\fakescan-retrieve-request.xml`:

| Element | Value |
|---------|--------|
| Scan NS | `http://schemas.microsoft.com/windows/2006/08/wdp/scan` (`sca:`) |
| `JobId` / `JobToken` | From CreateScanJobResponse |
| `DocumentDescription` / `DocumentName` | Fixed: `Scanned image file for the WSD Scan Driver` |

No format/dpi/region in RetrieveImage — those were on CreateScanJob / ValidateScanTicket. CancelJob follows failed accept with **`JobId` only**; see [e2e-wia-status.md](../e2e-wia-status.md).

## pdfcraft notes

- `scan devices` opens **every** WIA device; an offline Brother USB can fail/hang the whole list.
- Prefer `scan --device "Fake WSD scanner" …` once Fake is in the WIA enum.
- Connection type for WSD ports is typically `wsd` (URN port name).

## Protocol layers (pointer)

| Layer | Doc |
|-------|-----|
| Discovery | [ws-discovery.md](ws-discovery.md) |
| Device metadata | [ws-transfer.md](ws-transfer.md) + [dpws.md](dpws.md) |
| Eventing | [ws-eventing.md](ws-eventing.md) |
| Scan ops | [ws-scan.md](ws-scan.md) |
