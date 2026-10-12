# Windows: detect, SOAP, and install assist

pdfcraft talks **WIA**. fakescan must appear under Imaging Devices before `scan devices` sees it. There is **no supported silent "install this scanner URL" API** for inbox WSDScan - Windows discovers the device (WS-Discovery + Function Discovery) or you add it from Settings.

**Short operator path:** [runbook-fake-scanner.md](runbook-fake-scanner.md). Lab matrix: [e2e-wia-status.md](e2e-wia-status.md).  
**Python harness** (loopback / faults / optional pdfcraft-cli): [lab-harness.md](lab-harness.md).

Protocol reference (WS-Scan / DPWS / Discovery / Transfer / Eventing / Windows client): **[specs/](specs/README.md)** (Microsoft WSD/WDP Scan — not IETF RFCs).

The scripts under `scripts/` help with everything around that.

## Scripts

| Entry | Role |
|-------|------|
| `scripts/fakescan.ps1` | CLI (status / soap / discover / discoverable / fd / aep / swd / wia / install / elevated / helper / remove / unpair / bounce-fd / rebind / e2e / pages...) |
| `scripts/Fakescan.psm1` | Importable module (`Get-Fakescan*`, `Start-FakescanElevatedHelper`, `Install-FakescanScanner`, …) |
| `scripts/Fakescan.FunctionDiscovery.cs` | COM interop for Function Discovery (loaded by the module) |
| `scripts/e2e-wia.ps1` | Full WIA path: health -> discoverable -> install -> WIA assert -> optional pdfcraft-cli scan |

```powershell
cd D:\code\fakescan
Import-Module .\scripts\Fakescan.psm1

# Or CLI:
.\scripts\fakescan.ps1 status
.\scripts\fakescan.ps1 soap -BaseUrl http://127.0.0.1:3920
.\scripts\fakescan.ps1 discover
.\scripts\fakescan.ps1 discoverable -BaseUrl http://192.168.1.70:3920
.\scripts\fakescan.ps1 fd
.\scripts\fakescan.ps1 aep
.\scripts\fakescan.ps1 swd
.\scripts\fakescan.ps1 wia
.\scripts\fakescan.ps1 pages 3
.\scripts\fakescan.ps1 install -BaseUrl http://192.168.1.70:3920 -ProbeDiscovery -WaitSeconds 120
.\scripts\fakescan.ps1 helper     # one UAC; then helper remove|whatif|unpair|bounce-fd|stop (suite stays unelevated)
.\scripts\fakescan.ps1 elevated   # interactive UAC shell (remove / rebind / associate)
.\scripts\fakescan.ps1 remove     # needs elevation in-process, or use helper remove
.\scripts\fakescan.ps1 e2e -BaseUrl http://192.168.1.70:3920 -WaitSeconds 120
```

### What each command does

| Command | Network | Needs Windows install? |
|---------|---------|------------------------|
| `status` / `health` / `pages` / `feeder` / `profile` / `reset` | HTTP `/v1` | No |
| `fault <json>` | HTTP `/v1/fault` | No |
| `lab …` | Python `fakescan-lab` (loopback / SOAP / optional pdfcraft) | No for SOAP; WIA for `loopback-wia` |
| `scenarios` / `scenario <id>` | HTTP `/v1/scenarios` / `/v1/scenario` | No |
| `soap` | HTTP SOAP `/ScannerService` | No |
| `discover` | UDP 3702 Probe | No (multicast must work) |
| `discoverable` | SOAP + UDP + Function Discovery + WinRT AEP + WIA | Verdict includes `fd-installable` |
| `fd` | Function Discovery COM (WSD provider) | Lists discovered WSD instances (Brother and/or fakescan) |
| `aep` | WinRT `DeviceInformation` WSD AssociationEndpoints | Paired / CanPair rows (Brother often paired; fakescan only if Windows published an AEP) |
| `swd` | Local PnP | Installed `SWD\DAFWSDPROVIDER\...` nodes only |
| `wia` | Local WIA/PnP list | Lists whatever is already installed |
| `install` | HTTP + FD + AEP + optional Probe + Settings UI | Assists install; **cannot** force PnP / silent Add device |
| `elevated` | UAC-elevated PowerShell + module | Interactive shell for `remove` / `rebind` / `associate` / `pnputil` — **not** silent install; **not** for running `fakescan.exe` |
| `helper` | One UAC, then session remove / whatif / unpair / bounce-fd / stop | Preferred for WIA lab teardown. Suite stays unelevated. Fake nodes only. Idle default 1800s |
| `remove` | Local PnP | `pnputil /remove-device` on fakescan **Image + SoftwareDevice** (never Brother). Needs elevation in-process, or `helper remove` |
| `unpair` | WinRT AEP | `UnpairAsync` on fakescan AssociationEndpoint (not fakeprint `…f3`) |
| `bounce-fd` | Local services | Restart **fdPHost**; **Start** DAS only if Stopped — never `Restart-Service -Force` on DAS (StopPending wedges Add/AEP). Never **dasHost**. Elevation or helper |
| `rebind` | Local PnP + Settings | Sticky/offline: remove → Unpair → on AccessDenied bounce fdPHost+DAS → Printers & scanners → wait WIA |
| `e2e` | HTTP + discovery + WIA + optional pdfcraft-cli | Runs `e2e-wia.ps1` (install wait, then glass/feeder acquire if CLI present) |

### Can PowerShell install the scanner?

| Goal | PowerShell? | How |
|------|-------------|-----|
| Enumerate / discover | **Yes** | `discover`, `discoverable`, `fd`, `aep`, `wia`, `swd` |
| SOAP / local matrix (`pytest`, `lab loopback-soap`, `/v1`) | **Yes (unelevated)** | No Imaging remove; no UAC |
| Open Add device UI + wait for WIA | **Assist only** | `install` / `Install-FakescanScanner` — **person** clicks Add device |
| Silent install (no person) | **No** | Gate is **PnP-X association**, not the INF. Inbox `wsdscdrv.inf` already matches once associated. |
| Remove installed Fake Imaging | **Yes (elevated step)** | Prefer `helper` then `helper remove`; or `elevated` + `remove` / `Remove-FakescanScanner` |
| Sticky WIA offline rebind | **Assist** | `rebind` / `Repair-FakescanScanner` (remove via helper or elevated) → person Add device. `stisvc` + DAS restart does **not** clear it. |
| Ghost AEP (PnP gone, Add empty, Unpair AccessDenied) | **Assist** | After Unpair AccessDenied, `rebind` bounces **fdPHost** then **DeviceAssociationService**. Opt-in: `rebind -RestartDiscovery` / `bounce-fd`. **Never** `dasHost` (UDP 3702). |

**Verdict stays:** scripts detect FD/AEP candidates and WIA completion; a person must click Add device. A custom INF does **not** create the association (see dead ends). **Do not run `fakescan.exe` elevated** — keep the scanner in a normal terminal; elevate only PnP remove (helper or shell).

## soap-only vs on-wire-discoverable vs fd-installable vs installed

These are different readiness levels. `.\scripts\fakescan.ps1 discoverable` prints one **Verdict**:

| Verdict | Meaning | What works | What does not |
|---------|---------|------------|---------------|
| **soap-only** | TCP `/ScannerService` answers (`SoapReachable`) but no fakescan ProbeMatch/Hello | Control plane + SOAP protocol tests | Windows Add device / WIA install (usually) |
| **on-wire-discoverable** | SOAP OK **and** Hello/ProbeMatch and/or AEP hit without CanPair | UDP + metadata path alive | Not yet Add-device ready |
| **aep-pairable** | SOAP OK **and** WinRT WSD AEP lists fakescan with `CanPair=true` and **not** IsPaired | Strongest pre-UI signal on this lab; Add device should list it | `PairAsync` still Failed without person click |
| **fd-installable** | SOAP OK **and** Function Discovery returned fakescan with `Installable=true` | Classic Add-device *candidate* | Still needs a person to click Add device |
| **installed** | WIA enum has fakescan (`FakescanInWia` / `AlreadyInstalled`) | pdfcraft can open it | - |
| **installed-offline** | PnP residue / sticky presence without usable WIA enum | Needs `rebind` (helper/elevated remove + person Add device) | `stisvc` restart alone does not fix (tried) |
| **ghost-aep** | SOAP OK, Imaging node **gone**, AEP still `IsPaired` | Unpair / `bounce-fd` then person Add | Settings Add list is empty until the pairing store clears |
| **unreachable** | SOAP failed | Nothing yet | Fix listen/advertise/firewall first |

Quick signals:

| Signal | Meaning | Command |
|--------|---------|---------|
| SOAP OK | Scan service answers on TCP | `soap` / `discoverable` -> `SoapReachable` |
| Hello / ProbeMatch (fakescan) | WS-Discovery UDP announce (`:3702`, not SSDP `:1900`) with **fakescan name or `:3920`**, or `wsd_hellos_sent` advancing | `test-discovery.ps1` / `discoverable` -> `UdpProbeMatch` |
| Function Discovery hit | Live FD WSD query returned a fakescan-like instance | `fd` / `Find-FakescanFunctionDiscovery` |
| FD Installable=true | `PKEY_PNPX_Installable` - Add device candidate | `discoverable` -> `FunctionDiscoveryInstallable` |
| WinRT AEP hit | WSD AssociationEndpoint published (may CanPair) | `aep` / `Get-FakescanAssociationEndpoints` |
| SWD DAFWSDPROVIDER | PnP association already created (installed WSD) | `swd` / `Get-FakescanSwdDevices` |
| Already in WIA | Windows already installed it | `wia` / `discoverable` -> `AlreadyInstalled` |

Lab note (2026-10-11, this PC `--advertise 192.168.1.70`):

1. Early: **on-wire-discoverable** — Hellos + SOAP Get; FD/AEP only Brother.
2. After restart with fresh `--device-uuid` + `--probe-listen` (+ ResolveMatch support in discovery): WinRT **`aep` lists `Fake WSD scanner (lite caps)` with `CanPair=true`** → verdict **`aep-pairable`**. Function Discovery COM still often omits fakescan.
3. `Connect-FakescanAssociationEndpoint` / `PairAsync(None)` → **Failed**; Imaging still empty until a person finishes **Printers & scanners → Add device**.
4. Another LAN host (`.81`) was SOAP-GETting `:3920` throughout.

```powershell
.\scripts\fakescan.ps1 aep          # expect Fake WSD scanner, CanPair=True
.\scripts\fakescan.ps1 discoverable # verdict aep-pairable
.\scripts\fakescan.ps1 pair         # experimental; expect Failed
.\scripts\fakescan.ps1 install -BaseUrl http://192.168.1.70:3920 -WaitSeconds 120
# person: Printers & scanners → Add device next to Fake WSD scanner
```

Notes:

- **SOAP alone is not discovery.** Docker Desktop port-publish often gives `soap-only`: pdfcraft still cannot see it until WSD install.
- **`UdpProbeMatch` ignores real Brother devices.** Matching uses the device name (`fakescan` / `Fake WSD scanner`) or port/URL `:3920`. A ProbeMatch whose XAddrs only contain `/ScannerService` (typical Brother WSD) does **not** count as fakescan.
- **Hello / SOAP Get is not install.** Live runs have seen Windows SOAP-Get fakescan every ~5s after Hello while Function Discovery still listed only Brother and no `SWD\DAFWSDPROVIDER\...` Image node was created for fakescan. Relationship metadata now includes DPWS `Host` + `Hosted` (was Hosted-only); re-check FD after rebuild.

- **Function Discovery is queryable.** `Get-FakescanFunctionDiscovery` / `Find-FakescanFunctionDiscovery` call COM `IFunctionDiscovery` against `Provider\Microsoft.Networking.WSD` (async; `Execute` returns `E_PENDING`, wait for `FD_EVENTID_SEARCHCOMPLETE`). A live Brother WSD MFP typically appears with `Installable=true` and `DeviceCategory` including Scanners. That proves the API works; it does not prove fakescan will appear.

## Function Discovery / PnP-X (what worked)

| API / surface | Result on a Win10/11 lab with a Brother WSD MFP |
|---------------|--------------------------------------------------|
| `CLSID_FunctionDiscovery` (`C72BE2EC-...`) | Registered; `Test-FakescanFunctionDiscoveryRegistered` |
| `CreateInstanceCollectionQuery("Provider\\Microsoft.Networking.WSD")` | Works; async `E_PENDING`; notifications deliver hits |
| `Layered\\Microsoft.Networking.Devices` + sub `WSD` | Same Brother hits |
| `AddQueryConstraint("Type", ScannerServiceType)` | Still returned Brother (device advertises `ScanDeviceType`) |
| `Provider\\Microsoft.PnPX.Association` sync | Empty (0 instances) |
| `GetInstanceCollection` on PnP / Devices layered | Fragile from PowerShell (null subcategory / stub errors); prefer async WSD query |
| Property store `PKEY_PNPX_*` / `PKEY_PNPX_Installable` | Readable (`FriendlyName`, `XAddrs`, `Types`, `Installable`) |
| Discovered-but-not-installed list | FD hits with `Installable=true` and **no** matching Image `SWD\DAFWSDPROVIDER` node; scripts can report that gap, not close it |

## Settings: exact clicks (person required)

Add device **cannot** be completed by a supported API. `Install-FakescanScanner` / `Open-FakescanAddDevice` open **only** `ms-settings:printers` (Printers & scanners) and wait for WIA. Do **not** open the Bluetooth Settings URI — it steals focus from the Add list.

1. Start fakescan with LAN `--advertise` so Hellos reach this PC (`discoverable` / `aep` should show fakescan).
2. **Settings → Printers & scanners** (`ms-settings:printers` only — scripts never open Bluetooth).
3. Under **Add a printer or scanner**, click **Refresh** if needed; when **Fake WSD scanner (lite caps)** appears, click its **Add device**.
4. If Printers & scanners is already open but Fake is **not** in the Add list → fix **discovery/advertise** (Hello, `--advertise`, `discoverable` verdict). Wrong Settings page is not the issue.
5. Verify: `.\scripts\fakescan.ps1 wia` and `swd` show a fakescan-like Image / `SWD\DAFWSDPROVIDER\urn:uuid:...` row; acquire by name (avoid `scan devices` if Brother USB is offline).

## Dead ends (honest)

| Tool / idea | Why it does not silently install fakescan |
|-------------|-------------------------------------------|
| `pnputil /scan-devices` | Rescans the existing PnP tree only. Does not create a WSD association from the network. Needs elevation; without admin: "Access is denied." |
| `pnputil /enable-device <id>` | Only enables an **already present** instance ID. There is no fakescan Image id until Add device succeeds. |
| `pnputil /add-driver wsdscdrv.inf` | Inbox INF is already present. Without a PnP-X **association**, nothing new attaches. |
| Custom INF matching `UMB\VEN_FAKE&DEV_WSDSCAN` | Useful later for a **vendor WIA USD** on top of `WSDScan.sys` ([sample INF](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/sample-inf-file-for-a-web-services-scanner)). Does **not** create the Imaging node. Live install already matched CompatibleId → inbox `wsdscdrv.inf`. |
| `SilentInstall` class flag | Only suppresses UI when a **devnode already exists** and the package is in the driver store. Not association. |
| `IPNPXDeviceAssociation::Associate` | Documented programmatic Install equivalent — needs an FD function instance with `PKEY_PNPX_GlobalIdentity`. **Wired (2026-10-12):** `Invoke-FakescanPnpxAssociate` / `fakescan.ps1 associate` (requires elevation). FD CAN list fakescan with `Installable=true` for an uninstalled device actively sending Hellos. When FD has the live instance, `Associate` call chain is correct. Whether it creates the Imaging node without a person is **unproven** (device was already installed on this lab). See [e2e-wia-status.md](e2e-wia-status.md) §FD/Associate. |
| WinRT `PairAsync` on WSD AEP | Tried; returns `Failed` for fakescan. Does not create Imaging. |
| `Enable-PnpDevice` / `Disable-PnpDevice` | Same as enable/disable existing nodes. Useful for `remove`, not create. |
| `devcon create` / `devcon install` | Can invent hardware ids; does **not** wire Function Discovery + inbox WSDScan SOAP transport the way Add device does. Easy to leave junk devices; do not use against Brother. |
| `IFunctionDiscovery::AddInstance` | Publication/registry style instances - not "install this WSD scanner into Image class". |
| UI Automation clicking Settings | Fragile, build-specific, not a supported install API; not shipped as automation here. |
| Docker Desktop bridge + custom INF | Orthogonal. Bridge often yields `soap-only` (no multicast). INF never fixes UDP 3702. Use native `fakescan.exe --advertise <LAN-IP>` (or Linux host-net) for Windows discover/install. |

**Verdict: Add device cannot be automated in a supported way today.** The missing step is **association** (person click, or FD `Associate` if/when an FD instance exists) — not a custom INF. Scripts detect FD/AEP candidates and WIA completion; a person must click.

## Lab process (foreground / agent-monitored)

Prefer a normal terminal (no UAC window) so logs stay visible:

```powershell
# Stop any previous fakescan.exe first.
$env:RUST_LOG = 'fakescan=debug,tower_http=debug'   # optional; -v does the same when RUST_LOG unset
& D:\temp\fakescan-target\debug\fakescan.exe `
  --listen 0.0.0.0:3920 --advertise 192.168.1.70 `
  --profiles D:\code\fakescan\profiles --profile brother-wsd `
  --device-uuid 00000000-0000-4000-8000-0000000000f2 `
  --probe-listen -v
```

`-v` logs each WSD SOAP Action with peer, path, elapsed ms, CreateScanJob / RetrieveImage details, and tower-http request lines. Hellos stay at `debug` (noisy but useful when diagnosing discovery). Elevate only Imaging remove (`helper` or `elevated` + `remove`) — **never** run `fakescan.exe` elevated.

## Typical lab flow

1. Start fakescan so the **Windows PC can reach TCP 3920** and, for install, **UDP 3702 on the LAN**:

   ```bash
   # Preferred for WSD install: Linux host network + advertise LAN IP
   docker run --rm --network host ghcr.io/b1tninja/fakescan:latest \
     --listen 0.0.0.0:3920 --advertise 192.168.1.10 \
     --profiles /app/profiles --profile brother-wsd \
     --device-uuid 00000000-0000-4000-8000-0000000000f2
   ```

   On Docker Desktop for Windows, bridge port-publish is fine for **SOAP/control**, but multicast discovery usually fails. For install, run the `fakescan.exe` binary on Windows (or a Linux VM on the LAN) with `--advertise <this-PC-LAN-IP>`. Pass a stable `--device-uuid` so Add device / PnP does not see a new EPR on every restart.

2. From the Windows box:

   ```powershell
   .\scripts\fakescan.ps1 soap -BaseUrl http://192.168.1.10:3920
   .\scripts\fakescan.ps1 discoverable -BaseUrl http://192.168.1.10:3920
   .\scripts\fakescan.ps1 fd
   .\scripts\fakescan.ps1 install -BaseUrl http://192.168.1.10:3920 -ProbeDiscovery -WaitSeconds 120
   ```

3. In **Printers & scanners**, under Add a printer or scanner, click **Add device** next to **Fake WSD scanner**.

4. Confirm:

   ```powershell
   .\scripts\fakescan.ps1 wia
   .\scripts\fakescan.ps1 swd
   # then pdfcraft-cli scan devices  -> connection: wsd
   ```

5. Optional one-shot path (install wait + WIA assert + glass/feeder acquire when `pdfcraft-cli` is available):

   ```powershell
   .\scripts\fakescan.ps1 e2e -BaseUrl http://192.168.1.10:3920 -WaitSeconds 120 -ProbeDiscovery
   # or: .\scripts\e2e-wia.ps1 -BaseUrl http://192.168.1.10:3920 -WaitSeconds 120
   ```

6. Tear down / rebind when done or when WIA goes sticky-offline (Image + parent SoftwareDevice via `pnputil`; never Brother). Prefer the session **helper** so the WIA suite stays unelevated:

   ```powershell
   .\scripts\fakescan.ps1 helper          # one UAC; idle default 1800s
   .\scripts\fakescan.ps1 helper whatif   # preview Fake targets
   .\scripts\fakescan.ps1 helper remove   # Remove-FakescanScanner via helper
   .\scripts\fakescan.ps1 helper stop
   # Interactive alternative:
   .\scripts\fakescan.ps1 elevated
   .\scripts\fakescan.ps1 remove          # or: Remove-FakescanScanner -WhatIf
   # Sticky 0x80210005 / PnP OK / no SOAP after fakescan restart:
   .\scripts\fakescan.ps1 rebind -BaseUrl http://192.168.1.10:3920 -WaitSeconds 120
   ```

## `helper` / `elevated` / `remove` / `rebind`

| When | Use | Notes |
|------|-----|-------|
| SOAP / local matrix / control plane | Nothing elevated | `pytest`, `lab`, `/v1`, `soap` — no Imaging remove |
| Repeated Fake Imaging remove while lab stays unelevated | **`helper`** | One UAC → `helper remove` / `whatif` / `unpair` / `bounce-fd` / `stop`. Request file accepts only those cmds (never instance ids). Session field is `process_id`. Idle default 1800s. IPC under `%TEMP%\fakescan-elev-helper`; Watch drops stale request/reply on start and exit. Self-check (no UAC): `.\scripts\check-elev-helper.ps1` |
| One-off remove, `associate`, or poking `pnputil` | **`elevated`** | Interactive UAC shell with module imported |
| Tear down Fake Image + SoftwareDevice | **`remove`** / `Remove-FakescanScanner` | Needs elevation in-process, or ask the helper. Never Brother |
| Sticky offline (`0x80210005`, PnP OK, no SOAP) | **`rebind`** | Remove (+ Unpair AEP) → Printers & scanners → person **re-Add**. Remove step still needs elevation or a running helper. `stisvc` / DAS restart does **not** clear this |
| Ghost AEP / empty Add / Unpair AccessDenied | **`bounce-fd`** / `rebind` | After remove, pairing store ≠ PnP. Bounce **fdPHost** (WSD FD cache, [KB 973560](https://support.microsoft.com/kb/973560)). DAS: **Start only if Stopped** — never `Restart-Service -Force` (lab 2026-10-12: StopPending wedged Add/AEP until lone DAS svchost killed). Ghost cleared after fdPHost alone then Unpair. Never **dasHost**. `PairTool /disassociate` if Unpair still denied. Last resort: new `--device-uuid` |

`Remove-FakescanScanner` collects fakescan **Image** and parent **SoftwareDevice** `SWD\DAFWSDPROVIDER\urn:uuid:…` nodes (`Get-FakescanPnpRemoveTargets`), then `Disable-PnpDevice` and **`pnputil /remove-device`** (preferred: `Remove-PnpDevice` is missing on some builds). Matching uses `Test-FakescanLooksLike` (name `fakescan` / `Fake WSD scanner`, or `:3920`) — never a generic Brother entry. `-WhatIf` lists both node kinds.

**Add device stays a person.** Helper and elevated remove only clear Fake PnP; they do not install. **Do not run `fakescan.exe` elevated.**

**Two stuck states — do not mix the service bounces:**

| State | What you see | Service bounce tried? | Correct recovery |
|-------|----------------|----------------------|------------------|
| **Sticky WIA offline** | PnP Image `Status=OK`, Connect `0x80210005`, **no** `/ScannerService` SOAP | **Yes:** `stisvc` and DeviceAssociationService — **did not clear it** | `pnputil` remove (helper) → Unpair → person **re-Add** while fakescan stays up (`rebind`) |
| **Ghost AEP** | PnP Image **gone**, AEP `IsPaired=True`, Settings Add list empty, `UnpairAsync` **AccessDenied** (even elevated) | **fdPHost was never bounced** for this. DAS bounce alone was the sticky-offline experiment, not this | Unpair (WinRT or `PairTool /disassociate`) → if still denied, bounce **fdPHost** then **DeviceAssociationService** (`bounce-fd` / `rebind` auto on AccessDenied). **Never** bounce **dasHost** (UDP 3702 — fakescan and fakeprint Hellos). **FDResPub** publishes *this* PC; it is not the WSD-scan cache. Last resort: new `--device-uuid` so Windows is not pairing the stuck AEP id |

`rebind` / `Repair-FakescanScanner`: elevated or helper remove → Unpair → on AccessDenied (or `-RestartDiscovery`) bounce fdPHost then DAS → open Printers & scanners only → wait for `FakescanInWia`. AccessDenied on Unpair is [DeviceUnpairingResultStatus](https://learn.microsoft.com/uwp/api/windows.devices.enumeration.deviceunpairingresultstatus): the caller lacks pairing permission, not “needs more UAC.” PairTool is the protocol-device unpair; WinRT Unpair is the same store.

## `e2e` (WIA end-to-end)

`scripts/e2e-wia.ps1` (also `.\scripts\fakescan.ps1 e2e`):

1. `/health`
2. `Test-FakescanDiscoverable` (SOAP + Hello/FD, or already installed)
3. `Install-FakescanScanner` with `-WaitSeconds` (opens Add device unless `-SkipUi`; polls FD vs WIA)
4. Assert WIA/PnP shows a fakescan-like device
5. If `pdfcraft-cli` is on PATH or `-PdfcraftCli` is set: `scan devices` (filter fakescan + wsd), glass acquire 1 page to a temp folder, `Set-FakescanPages` / `Set-FakescanFeeder`, feeder acquire
6. Optional `-Remove` calls `Remove-FakescanScanner`

On process exit, fakescan multicasts a WS-Discovery **Bye** (same Types / XAddrs / device UUID as Hello, AppSequence MessageNumber incremented) so Windows can drop a stale discovery entry. On start, `AppSequence/@InstanceId` and discovery `MetadataVersion` use boot-time seconds (not fixed `1`), with a short Hello burst — see [specs/ws-discovery.md](specs/ws-discovery.md). Sticky WIA offline after restart without re-Add remains an open lab question ([specs/windows-client.md](specs/windows-client.md#presence-sticky-offline--open-questions)).

## Why `install` opens Settings

Microsoft documents WSD scanner install via **WSDScan.sys + INF / Function Discovery**, not a PowerShell "Add-Scanner -Uri ...". You can enable an *already present* PnP image device with `Enable-PnpDevice`, but you cannot create the WSD association that way.

So `Install-FakescanScanner`:

1. Verifies `/health` + SOAP `GetScannerElements`
2. Optionally runs WS-Discovery Probe / Hello checks
3. Queries Function Discovery and prints whether fakescan is an installable candidate
4. Opens **`ms-settings:printers` only** (`Open-FakescanAddDevice` — never the Bluetooth URI)
5. Optionally waits, reporting FD-visible-but-not-WIA vs WIA-installed

## Firewall

Allow inbound:

- TCP 3920 (control + SOAP)
- UDP 3702 (WS-Discovery), when testing install
