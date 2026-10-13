# Why Windows does not list the fakescan daemon under Add device (diagnosis, 2026-10-12)

Read-only investigation on this PC (Windows 11 26300). Nothing on the system was changed; the daemon
on :3920 was only read with GET. Passive listener scripts live in the session scratchpad
(`...\scratchpad\listen.py`, `listen2.py`). The Brother (e3248000-...) was never contacted.

## Caveat: the host state moved under the investigation

Another session was running tests on this PC at the same time. A tray/manager instance
(`D:\temp\fakescan-target-tray3`, `--manage --listen 127.0.0.1:3944 --device t1 --device t2`) loaded the same
`%LOCALAPPDATA%\fakescan\devices.json` and so also advertises the daemon's UUID `...0000f2`, with
`XAddrs=http://127.0.0.1:3944/WebServices/Device` on `multicast_iface=127.0.0.1`. Between 18:45 and 18:46, f2, fe07 (t2)
and e11e were installed by Windows (DeviceSetupManager "container ready" at 18:45:10, 18:45:34, 18:46:20; WIA lists
"lite caps" and "glass only"). So the premise "no fakescan PnP/WIA/AEP node" was true earlier but is not true now.
Conclusions below separate what was observed before and after.

## Facts measured

| Check | Result |
|---|---|
| Who owns UDP 3702 | `dasHost` (pid 7644), `0.0.0.0:3702` and `[::]:3702`; no other owner. The daemon has no :3702 socket (its only UDP socket is ephemeral :51605). |
| Daemon mode | cmdline `--manage --listen 0.0.0.0:3920 --advertise 192.168.1.70`, no `--probe-listen`. On Windows `probe_listen` defaults to off, so the daemon is Hello-only. `/v1/status`: `wsd_probes_answered=0`. (The registry entry for "default" says `probe_listen: true`, but the running process does not do it.) |
| Passive listener, joined 239.255.255.250 per interface | Daemon Hello (src 192.168.1.70:51605) is delivered on the Ethernet membership every 5 s; not seen on vEthernet or loopback (correct, it is sent out Ethernet only). Hyper-V is not swallowing it. |
| Hello content | `Types=wsdp:Device wscn:ScanDeviceType`, `XAddrs=http://192.168.1.70:3920/WebServices/Device`, no Scopes, `AppSequence InstanceId=1791855223` (= 2026-10-13T01:33:43Z, the daemon start time, sane, 2026 clock) with `MessageNumber` rising 166 -> 190 over the capture. No staleness problem inside the daemon's own sequence. |
| What hits the daemon's SOAP port | `/v1/activity` (64 retained events): every `wsd Get` is from 192.168.1.81 (MAC a8:b8:e0, DNS name `NAS`), two per Hello. Zero requests from 192.168.1.70 (this PC) or any Windows client on this PC. So the "host at .81 already contacted it" is the NAS, not this PC's dasHost. |
| dasHost multicast traffic seen | `Resolve` messages from `192.168.1.70:61284` (dasHost, pid 7644) for f2 and fe07, sent to the group on Ethernet, vEthernet and loopback (so dasHost is joined/active on all three). No `Probe` seen in ~60 s of listening. No fakescan process answers a Resolve (all are Hello-only). |
| Joins (`netsh interface ip show joins`) | 239.255.255.250 joined on Ethernet (3 refs), vEthernet (3), loopback (3). Wi-Fi adapters are disconnected, only 224.0.0.1. |
| Interfaces | Ethernet 192.168.1.70 metric 25, Private; vEthernet (Default Switch) 192.168.16.1 metric 5000; Wi-Fi 3/4/Wi-Fi are 169.254 and disconnected. IPv4 multicast routes exist on all. |
| Firewall | Network Discovery rules (WSD-In for dasHost and svchost, UDP 3702, Private) are enabled. Daemon exe has TCP and UDP inbound Allow rules (Private, Public) created by the Windows allow prompt at 18:28:19. No inbound block rule touches UDP 3702. Auditing is off (`auditpol` needs elevation), so drops are not logged. |
| Services | DeviceAssociationService Running, FDResPub Running, fdPHost Stopped (Manual), SSDPSRV Running, stisvc Running. |
| Event logs | No WSDAPI / FunctionDiscovery event channels exist (providers are ETW-only). DeviceSetupManager/Admin only shows the container events above. Kernel-PnP showed nothing relevant. |
| Second same-host AEP | `Microsoft Publication Service Device Host` (`urn:uuid:8ca7230e-e5bf-42e5-af9f-231ae916dee3`), IsPaired False, CanPair False. It is this PC's own FDResPub WSD publication. So same-host WSD devices do appear as AEPs; Windows has no blanket "ignore own host" filter. |
| FD now | FD returns f2 as `Installable=true` with `XAddrs=http://127.0.0.1:3944/...` (not the daemon's 192.168.1.70:3920) plus t1/t2 on 127.0.0.1. The tray3 Hello won the f2 record. No FD or AEP row ever carried `192.168.1.70:3920`. |
| tray3 log | tray3 advertises with `recv_3702=false` (also Hello-only) and `multicast_iface=127.0.0.1`, InstanceId 1791855934 (> the daemon's 1791855223). Windows opened `POST /ScannerService` from 127.0.0.1 35 ms after its first Hello. |

## Hypotheses

1. Interface binding / Hyper-V swallowing multicast: REFUTED. Hello reaches the host on Ethernet; dasHost sends its own Resolves on all three interfaces; Ethernet is the only connected physical NIC and Private.
2. Firewall: UNLIKELY. Allow rules exist; nothing blocks; a same-host loopback of multicast is not filtered by Windows Firewall anyway. Not provable without auditing (elevated).
3. fdPHost Stopped: REFUTED as cause. Brother and the 127.0.0.1 devices are discovered with it stopped; the Add list uses DeviceAssociationService/dasHost.
4. InstanceId / MessageNumber stale: Daemon alone: REFUTED (value sane, increasing). BUT see 5: while another process advertises the same UUID f2 with a higher InstanceId, WSDAPI would likely treat the daemon's lower-InstanceId Hellos as stale. Inference, not directly shown.
5. Duplicate UUID f2 (daemon @192.168.1.70:3920 vs tray3 manager @127.0.0.1:3944, same devices.json): OBSERVED as fact (FD XAddrs point at 3944, tray3.out shows f2 advertised). It explains why FD/AEP data for f2 is not the daemon's now. It does not explain the original symptom before 18:44 (only the daemon was up then, as far as is known).
6. Hello-only (no Probe/Resolve responder): LIKELY CONTRIBUTING. dasHost issues Resolve for fakescan EPRs and nobody answers; the lab recipe that worked (`docs/e2e-wia-status.md`: `--advertise 192.168.1.70 --probe-listen`, reached `aep-pairable`) had :3702 bound. Counter-evidence: the loopback-advertised tray3 devices (also Hello-only) were discovered and installed. So Hello-only alone is not proven sufficient or insufficient.
7. Windows ignores Hellos whose source is this host's own LAN IP: POSSIBLE, UNPROVEN. Supporting: this PC's dasHost never sent a single request to 192.168.1.70:3920 while the NAS did, for the same Hellos; loopback-sourced Hellos were consumed. Against: the Publication Service AEP shows same-host sources are handled; the earlier successful lab run was also same host and LAN-advertised (with --probe-listen).
8. Add dialog needs a live Probe: UNKNOWN. No Probe was observed in 60 s while Settings (SystemSettings pid 10288) was open somewhere; a Probe at Refresh time was not captured.
9. Types/XAddrs shape makes the list skip it: REFUTED as the sole cause. Same Types/XAddrs form was discovered by FD (Installable=true, DeviceCategory Scanners) when advertised at 127.0.0.1; Brother omits Scopes too (per existing docs).

## Ranked likely cause

1. Likely: the daemon's Hellos (LAN-IP XAddrs, source = this PC's Ethernet IP) are never turned into a metadata fetch by this PC's dasHost, while remote hosts (NAS) do fetch. Direct evidence is the activity log (no request from .70) plus Hellos verifiably on the wire. The reason is not yet shown; the two candidates are 6 (Hello-only, Resolve unanswered) and 7 (own-IP-source Hello ignored), with 5 (UUID collision, higher InstanceId) layered on top from 18:44 on.
2. Confirmed contributor now: UUID f2 collision with the tray3 manager. Fix by removing the collision before any further discovery test.

I cannot name a single proven root cause. The experiments below separate 6, 7 and 5.

## Next experiments (in order)

A. No change, human click: with the daemon alone running, open Settings > Printers & scanners > Add device and press Refresh while the passive listener runs:
`python D:\temp\claude\D--code-pdfcraft-fakescan\bf9242a9-80ed-42d5-9a8e-645ce38dff91\scratchpad\listen2.py 60`
Read-only. Shows whether dasHost sends a Probe (then the daemon must answer, supporting 6) and which Resolve/Probe it issues for f2.

B. Remove hypothesis 5: stop the tray3/manager and the other test fakescan instances that read `devices.json` (they are another session's processes; coordinate first, do not kill the daemon). Then restart the daemon fresh so its InstanceId is the newest, and check `fakescan.ps1 fd` / `aep`. Risk: low, but it interrupts the other session's tests. Not done here (state-changing).

C. Separate 6 from 7 with a throwaway instance (not the daemon, new UUID, other port):
`fakescan.exe --listen 0.0.0.0:3931 --advertise 192.168.1.70 --device-uuid 00000000-0000-4000-8000-0000000000a1 --probe-listen --profile brother-wsd`
then `fakescan.ps1 fd` / `aep`. Repeat without `--probe-listen`. Results: appears only with --probe-listen -> cause is 6 (fix: run the daemon with `--probe-listen`, it binds :3702 with SO_REUSEADDR next to dasHost; `listen2.py` shows Resolve/Probe then get answered). Appears in neither -> cause is 7 (own-IP source Hello). Appears in both -> cause was 5/stale state on the f2 EPR; use a fresh UUID. Risk: one extra Windows firewall prompt, a transient FD/AEP entry; it can be left uninstalled. Only run with the human present.

D. If C shows 7, run the daemon on another LAN host or a VM/WSL2 bridged to the LAN (a remote sender is what the NAS path proves works for Hello), or advertise `127.0.0.1` for a Windows-only test (proven discoverable by the tray3 runs, but not reachable from other machines).

E. Elevated, to see what dasHost does with the Hello (needs an admin shell; read-only capture; risk: none beyond a few MB of trace files):
```
pktmon filter add -p 3702
pktmon start --capture --pkt-size 0 --comp nics -f D:\temp\wsd3702.etl
:: wait 30 s, open Add device and press Refresh
pktmon stop
pktmon etl2txt D:\temp\wsd3702.etl -o D:\temp\wsd3702.txt
pktmon filter remove
```
and, to enable more WSD tracing, `logman query providers | findstr /i "wsd function discovery daf"` then a `logman create trace` on the provider names it prints (names not verified here). Also `auditpol /set /subcategory:"Filtering Platform Packet Drop" /failure:enable` would log firewall drops, but that is a security-audit setting change; do it only if wanted and turn it off afterwards.

F. Human-only, in the daemon-alone state (after B): if Add device then shows "Fake WSD scanner" click Add and confirm WIA; if it does not show, record the Probe from A and the C result and stop.

## Not determined

- What happened between 17:23 (f2 removed) and 18:44 on the Add list: no ETW/WSDAPI log exists, and the Add UI was not observed.
- Why f2/fe07/e11e were installed at 18:45-18:46 (another session's test or a human click); not attributable from logs.
- Whether dasHost actually received the LAN-source Hello (needs the elevated trace in E).
- The `swd` script command fails when invoked through `powershell -File` (SwitchParameter conversion error in `fakescan.ps1`), unrelated to this problem; it works when the module function is called directly. I did not run the standalone `discover` (it multicasts Probes that the real Brother would answer).
