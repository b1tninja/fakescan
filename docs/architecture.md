# Architecture

## Goals

1. **Deterministic** — same profile + same page stack → same bitmaps out of WIA.
2. **Two capability surfaces** — “USB-full” vs “WSD-lite”, matching real inbox WSD vs vendor USB.
3. **Scriptable** — CI sets feeder state and pages without a person at the glass.
4. **No app mocks** — pdfcraft talks real WIA; fakescan is the other end of the pipe.

## Components

```
┌─────────────────────────────────────────────────────────────┐
│  fakescan host                                              │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────────────┐ │
│  │ control API │  │ page store   │  │ capability profile │ │
│  │ :3920       │  │ BMP/PNG/JPEG │  │ JSON               │ │
│  └──────┬──────┘  └──────┬───────┘  └──────────┬─────────┘ │
│         │                │                      │           │
│         └────────────────┼──────────────────────┘           │
│                          ▼                                  │
│              ┌───────────────────────┐                      │
│              │ scan engine (job FSM) │                      │
│              └───────────┬───────────┘                      │
│           ┌──────────────┴──────────────┐                   │
│           ▼                             ▼                   │
│  ┌─────────────────┐         ┌────────────────────┐         │
│  │ WSD Scan +      │         │ WIA USD (optional) │         │
│  │ WS-Discovery    │         │ INF + DLL          │         │
│  └────────┬────────┘         └─────────┬──────────┘         │
└───────────┼────────────────────────────┼────────────────────┘
            │ SOAP / HTTP                │ WIA COM
            ▼                            ▼
     WSDScan.sys / WIA            Scanners and Cameras
            └────────────┬───────────────┘
                         ▼
                    pdfcraft / WIA apps
```

## Control API (planned)

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/health` | process up |
| `GET` | `/v1/status` | profile id, feeder ready, pages remaining, active jobs |
| `POST` | `/v1/profile` | switch `brother-usb` / `brother-wsd` / custom |
| `POST` | `/v1/feeder` | `{ "ready": true\|false }` — sticky FEED_READY behavior |
| `POST` | `/v1/pages` | replace stack from directory or uploaded images |
| `POST` | `/v1/reset` | clear jobs, restore profile defaults |

Localhost only by default.

## WSD path (P0)

| Step | Status |
|------|--------|
| WS-Discovery Hello + ProbeMatch (`discovery.rs`) | stub in tree |
| HTTP `/ScannerService` endpoint | SOAP dispatcher |
| `GetScannerElements` / `DefaultScanTicket` | done (from profile) |
| `CreateScanJob` / `RetrieveImage` (MTOM JPEG) | done |
| WS-Transfer Get metadata | minimal ThisDevice/ThisModel |
| Windows installs via WSDScan → WIA acquire | live verify next |

Reference prior art: [bobi/WsdScanService](https://github.com/bobi/WsdScanService) (CoreWCF). Stay in Rust with a thin SOAP layer unless GetScannerElements proves too painful.

Published capabilities come from the active JSON profile (dpi lists, formats, page sizes, presence/absence of optical dpi).

## WIA USD path (P1)

A user-mode WIA 2.0 minidriver (patterned on Microsoft `wiadriverex` / `ProdScan`) that:

- Registers under Imaging Devices.
- Reads the same control API / page store (or a shared memory/named-pipe IPC).
- Publishes USB-full property trees for feeder + glass.

Requires INF install (admin once). Prefer unsigned test-signing on lab machines.

## Sticky FEED_READY

Real Brother USB reports feeder “ready” even when empty. Profiles may set:

```json
"feeder": { "sticky_ready": true }
```

so Auto→glass fallback and empty-feeder races can be tested without hardware.

## Out of scope

- Print / fax / copy simulation (use a different project if needed).
- Evading security software or spoofing a vendor’s signed driver.
- Shipping vendor firmware or copyrighted UI strings.
