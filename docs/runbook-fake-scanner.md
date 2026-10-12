# Runbook: fake WSD scanner → Windows WIA

Ordered checklist for operators/devs. Wire formats live under [specs/](specs/README.md) — this page is only “what it takes to run.” Scripts and dead ends: [windows.md](windows.md). Lab matrix: [e2e-wia-status.md](e2e-wia-status.md).

Fake example values: name **Fake WSD scanner**, advertise `192.168.1.70:3920`, UUID `00000000-0000-4000-8000-0000000000f2`.

## Checklist

1. **Build** — `cargo build -p fakescan` (or use a known `fakescan.exe`).
2. **Advertise LAN IP** — run on the Windows LAN (native exe preferred; Docker Desktop bridge is SOAP-only):
   ```text
   fakescan.exe --listen 0.0.0.0:3920 --advertise 192.168.1.70 \
     --profiles profiles --profile brother-wsd \
     --device-uuid 00000000-0000-4000-8000-0000000000f2 \
     --probe-listen -v
   ```
3. **Hello** — confirm WS-Discovery Hellos (`wsd_hellos_sent` / discover scripts). dasHost may own `:3702`; Hello-only can still publish an AEP.
4. **Discoverable verdict** — `.\scripts\fakescan.ps1 discoverable -BaseUrl http://192.168.1.70:3920` → prefer **`aep-pairable`** (or `fd-installable`). See verdict table in [windows.md](windows.md).
5. **Person Add** — **this is the install gate.** Stay on **Printers & scanners** only (`ms-settings:printers`; `Open-FakescanAddDevice` must not open the Bluetooth URI — it steals focus). Under **Add a printer or scanner**, click **Add device** next to Fake WSD scanner. If Fake is missing from Add:
   - First check discovery: Hellos / `--advertise` / `discoverable` → prefer `aep-pairable`.
   - Also check **stale pairing**: after `pnputil` remove, AEP can stay `IsPaired=True` and Settings hides Fake. Clear with `.\scripts\fakescan.ps1 unpair` (or `rebind`, which Unpairs after remove), then Refresh. Custom INF / Docker / registry Enum are **not** the gate ([windows-client.md](specs/windows-client.md)).
6. **WIA present** — `.\scripts\fakescan.ps1 wia` (and `swd`) shows the Image / `SWD\DAFWSDPROVIDER\urn:uuid:…000f2` node. Settings **Ready** ≠ WIA online.
7. **Connect / Subscribe in `-v` logs** — open should hit `/ScannerService`: **Subscribe** succeeds; if `ScanDestinations` present, **DestinationResponses** are required ([ws-eventing.md](specs/ws-eventing.md)). Empty feeder must stay Idle/None (not AttentionRequired).
8. **Glass acquire by name** — do **not** rely on `pdfcraft-cli scan devices` (hangs on offline Brother USB). Prefer:
   ```text
   pdfcraft-cli scan --out DIR --yes --device "Fake WSD scanner" \
     --source glass --pages 1 --dpi 100 --format png
   ```
9. **Control plane** — `/v1` pages, feeder, slow, fault/jam/cover still work without re-install (`.\scripts\fakescan.ps1 pages …`).

## After fakescan restart

Sticky WIA **offline** with **no SOAP** is possible while the Imaging node remains. Recovery: keep fakescan up → elevated `remove` / `rebind` (remove + **Unpair** + Printers & scanners) → person **re-Add**. AppSequence `InstanceId` now bumps per process start (see [ws-discovery.md](specs/ws-discovery.md)); that may reduce sticky offline but is **not** proven live — still use rebind if Connect stays `0x80210005` with no `/ScannerService` traffic. Details: [windows-client.md](specs/windows-client.md).
