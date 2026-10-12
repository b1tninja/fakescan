# Sibling: fakeprint

**fakescan** and **[fakeprint](../../fakeprint)** are companion lab devices for
Windows scan and print testing.  Same host can run both; keep ports and UUIDs
distinct.

| | **fakescan** | **fakeprint** |
|--|--------------|---------------|
| Role | Fake **scanner** | Fake **printer** |
| Default TCP | `:3920` | `:3921` |
| Wire (Phase 1) | WSD Scan + WIA | WSD Print + WSDMON |
| Lab ground truth | **Hard loopback** — upload JFIF → exact RetrieveImage echo | **Capture-assert** — SOAP/print → inbox / `GET /v1/jobs` |
| Python package | `python/` → `fakescan-lab`, `fakescan-ctl` | `python/` → `fakeprint-lab` |
| PowerShell | `scripts/fakescan.ps1` | `scripts/fakeprint.ps1` |
| Registry | `%LOCALAPPDATA%\fakescan\devices.json` | `%LOCALAPPDATA%\fakeprint\devices.json` |
| Named pipe | `\\.\pipe\fakescan-<name>` | `\\.\pipe\fakeprint-<name>` |
| MCP | `fakescan-mcp` | `fakeprint-mcp` |
| Port band | **Even**: 3920, 3922, … | **Odd**: 3921, 3923, … |
| Default UUID | `…000f2` | `…000f3` |
| Docs | this tree | [fakeprint docs](../../fakeprint/docs/README.md) |

## Co-existence on one host

Both can run simultaneously.  The port bands, UUIDs, registries, pipes, and MCP
entries are disjoint.  Windows PnP sees two separate devices.

```
fakescan.exe --lab-name default --listen 0.0.0.0:3920 --device-uuid …000f2
fakeprint.exe --lab-name default --listen 0.0.0.0:3921 --device-uuid …000f3
```

## Do NOT kill the sibling

**fakescan scripts must never stop or kill fakeprint processes, and vice versa.**

- `fakescan.ps1 remove` only removes fakescan PnP nodes (by UUID / name match).
- `fakescan-ctl ensure` only starts a fakescan exe.
- The registries are in separate `%LOCALAPPDATA%` subdirectories.
- Never read the other product's registry; never touch its port band.

Restarting a sibling from the wrong product's script would force a WIA
re-Add for the other device (expensive, manual step).

## Prefer ctl over kill

Same principle within each product: reconfigure the running instance via
`/v1/ctl` instead of stopping and restarting it.  See
[lab-instance.md](lab-instance.md) for details.

## Do not copy across

| Belongs in fakescan only | Belongs in fakeprint only |
|--------------------------|---------------------------|
| `/v1/loopback`, pages, pattern, effects, scenario | Job capture inbox + `/v1/jobs` |
| Feeder / jam / WIA acquire glue | Print job FSM / SendDocument |
| Glass + ADF **sources** (and hardware / manual duplex) | Printer **input trays** (`InputBins` / Tray1) |
| `pdfcraft-cli scan` lab path | Spooler / Add printer lab path |

Shared by design (copy patterns, not crates): WS-Discovery, Transfer Get shape,
"person clicks Add device," optional Python control client.

Marks use the same idea: device art plus a two-letter app monogram (**Fs** on
the scanner glass, **Fp** on the printer face).  fakeprint's LCD and logo
subtitle read **PC LOAD LETTER**.

- Scan lab: [lab-harness.md](lab-harness.md)
- Scan lab-instance control: [lab-instance.md](lab-instance.md)
- Print lab: [fakeprint lab-harness](../../fakeprint/docs/lab-harness.md)
