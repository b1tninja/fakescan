# fakescan

A **fake scanner** for Windows testing: configurable glass/feeder, page images from disk, and capability profiles that look like real WIA devices (USB Brother-style vs inbox WSD-style).

Built so [pdfcraft](https://github.com/) (and similar apps) can exercise acquire, Auto, sticky `FEED_READY`, page counts, and format refusals **without** a physical MFP.

## Research (what already exists)

| Option | What it is | Gap for us |
|--------|------------|------------|
| [Microsoft WIA driver samples](https://github.com/microsoft/Windows-driver-samples/tree/main/wia) (`wiadriverex`, `ProdScan`) | Real WIA 2.0 minidrivers; emit a test pattern; install via Scanners and Cameras + Have Disk | WDK/C++, MS-PL, not profile-driven, awkward for CI, no Brother-like USB vs WSD split |
| [twain/twain-samples](https://github.com/twain/twain-samples) + [wia-on-twain](https://github.com/twain/wia-on-twain) | Software TWAIN DS + WIA adapter | Aged, TWAIN-centric, hard to script FEED_READY / capability matrices |
| App-level mocks (e.g. NAPS2 `MockScanBridge`) | Unit-test doubles inside the app | Never hits real WIA / WSD |
| [bobi/WsdScanService](https://github.com/bobi/WsdScanService) | .NET CoreWCF **WSD Scan Service** + WS-Discovery | Closest “server”; not aimed at pdfcraft profiles / sticky feeder / dual USB·WSD surfaces |

**Conclusion:** there is no maintained, scriptable “fake scanner” that:

1. shows up under WIA the way a Brother USB + WSD pair does, and  
2. lets tests set feeder empty/ready, page stacks, dpi/format lists, and optical-res omission.

So this repo exists.

## Design (two surfaces)

Windows never talks “HTTP status page → WIA”. Paths that matter:

```
App (pdfcraft) ──WIA COM──► USB minidriver     (rich caps, high dpi)
App (pdfcraft) ──WIA COM──► WSDScan.sys ──SOAP──► network Scan Service
```

**fakescan** targets both eventually:

| Surface | How Windows sees it | Priority |
|---------|---------------------|----------|
| **WSD Scan Service** (user-mode) | `Brother… [uuid]`-style network scanner via inbox WSDScan | **P0** — no kernel driver, matches network testing |
| **WIA USD minidriver** (user-mode DLL + INF) | Local “USB-like” device with full property lists | **P1** — fork/extend `wiadriverex` ideas under MS-PL attribution |
| **Control plane** | HTTP/named-pipe: load pages, set FEED_READY, switch profile | **P0** — required for CI |

Capability **profiles** (JSON) describe what each surface publishes — see `profiles/`.

## Profiles

- `profiles/brother-usb.json` — high dpi, bmp-only, optical 1200×2400, pages 0–30  
- `profiles/brother-wsd.json` — 100/200/300 dpi, jpeg/tiff, no optical, richer page sizes  

These mirror live dumps from a Brother MFC-9330CDW (USB + WSD), not product UI strings.

## Quick start (Docker)

Docker is the supported deploy path. See [docs/deploy.md](docs/deploy.md).

```bash
docker compose up --build -d
curl http://127.0.0.1:3920/health
curl http://127.0.0.1:3920/v1/status
# SOAP: POST http://127.0.0.1:3920/ScannerService
```

Published image (after CI): `ghcr.io/b1tninja/fakescan:latest`

```bash
docker run --rm -p 3920:3920 ghcr.io/b1tninja/fakescan:latest
```

### From source

```bash
cargo run -p fakescan -- --profiles profiles --profile brother-wsd
# LAN advertise: --listen 0.0.0.0:3920 --advertise 192.168.1.10
```

## Status

| Piece | State |
|-------|--------|
| Control API (`/v1/*`, including `/v1/pages`) | working |
| Capability profiles (`brother-usb`, `brother-wsd`) | working |
| WS-Discovery Hello / ProbeMatch | stub (multicast) |
| WS-Scan SOAP (`GetScannerElements`, `ValidateScanTicket`, `CreateScanJob`, `RetrieveImage` MTOM, `CancelJob`) | working (unit-tested) |
| WS-Transfer Get (ThisDevice / ThisModel) | minimal stub |
| Windows WSDScan install → WIA list | **unverified live** (needs LAN advertise + firewall) |
| Docker image + compose | yes (`Dockerfile`, `docker-compose.yml`) |
| GHCR publish | `.github/workflows/docker.yml` |
| WIA USD (USB-like local device) | not yet |

```bash
curl -X POST http://127.0.0.1:3920/v1/pages \
  -H "content-type: application/json" -d "{\"count\":2}"
```

## CI

- `.github/workflows/ci.yml` — `cargo fmt` / clippy / test on Ubuntu + Windows; Docker compose smoke  
- `.github/workflows/docker.yml` — build/push `ghcr.io/<owner>/fakescan`  
- `.devcontainer/` — Rust tooling for Codespaces / VS Code

## License

MIT OR Apache-2.0 (project-original code). Any file derived from Microsoft WIA samples must keep **MS-PL** notices.
