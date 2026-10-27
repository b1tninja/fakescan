# fakescan

<p align="center">
  <img src="docs/assets/fakescan-logo.svg" alt="fakescan" width="640" />
</p>

<p align="center">
  <a href="https://github.com/b1tninja/fakescan/actions/workflows/ci.yml"><img src="https://github.com/b1tninja/fakescan/actions/workflows/ci.yml/badge.svg" alt="ci" /></a>
</p>

A **fake scanner** for testing acquire paths: scriptable glass/feeder, synthetic page patterns, image defects, and capability profiles that look like real WIA / WSD devices — without a physical MFP.

Built for [pdfcraft](https://github.com/storytold/pdfcraft) and similar apps (sticky `FEED_READY`, page counts, format refusals, deskew/despeckle stress). **pdfcraft** implements WIA/UI; **fakescan** implements the fake device and control API; end-to-end pdfcraft exerciser tests live in **pdfcraft-fakescan** ([stack](https://github.com/storytold/pdfcraft-fakescan/blob/main/docs/stack.md) when published, or `../pdfcraft-fakescan/docs/stack.md` in a monorepo). Companion **[fakeprint](https://github.com/b1tninja/fakeprint)** covers WSD Print on `:3921`. How the two scan/print labs differ: [docs/siblings.md](docs/siblings.md).

## The diag page

The default scan is fakescan's `diag` test page: a parametric layout that is generated, not stored, for any sheet size. It carries ArUco corner and edge markers, a chessboard and ChArUco board, dual-unit rulers, resolution and tone targets, and a QR payload that regenerates the exact page. The same page prints from [fakeprint](https://github.com/b1tninja/fakeprint) as a PDF at the real physical size, so a scanner and a printer are checked against one reference.

<p align="center">
  <img src="docs/diag-page-v3.png" alt="The diag page (layout v3), A4 at 100 dpi" width="360" />
  <img src="docs/diag-page-v3-closeup.png" alt="Close-up at 300 dpi: dual-unit rulers, an ArUco corner marker, a chessboard and a ChArUco board" width="360" />
</p>

Layout, parameters and what each element measures: [docs/diag-page-v3.md](docs/diag-page-v3.md).

## Docs

| Doc | Start here for… |
|-----|-----------------|
| [docs/README.md](docs/README.md) | Full index |
| [docs/siblings.md](docs/siblings.md) | vs fakeprint (ports, loopback ≠ capture) |
| [docs/lab-harness.md](docs/lab-harness.md) | Python client + hard loopback; `GET /v1/jobs`, `/v1/activity`, `/v1/log` |
| [docs/windows.md](docs/windows.md) | WIA install assist (person clicks Add device) |
| [docs/architecture.md](docs/architecture.md) | Layout, ports, control API |
| [docs/reference/](docs/reference/README.md) | Official WSD/WS-Scan rules (Format fallback, MustHonor, faults) + XSD/WSDL |
| [docs/coverage.md](docs/coverage.md) | Spec → feature completion map |
| [docs/roadmap.md](docs/roadmap.md) | Backlog (test pages, ANAC, more effects, …) |
| [docs/e2e-wia-status.md](docs/e2e-wia-status.md) | Live Windows gate |
| [docs/prior-art.md](docs/prior-art.md) | Why not WIA samples alone |
| [docs/deploy.md](docs/deploy.md) | Docker / LAN |
| [docs/linux-client.md](docs/linux-client.md) | sane-airscan |
| [docs/patterns.md](docs/patterns.md) | Page layouts + tester progression |
| [docs/defects.md](docs/defects.md) | Skew, speckles, streaks, hole punches, Gray8 |

## Quick start (Docker)

```bash
docker compose up --build -d
curl http://127.0.0.1:3920/health
curl http://127.0.0.1:3920/v1/status
```

Published image: `ghcr.io/b1tninja/fakescan:latest`

### From source

```bash
cargo run -p fakescan -- --profiles profiles --profile brother-wsd
# LAN: --listen 0.0.0.0:3920 --advertise 192.168.1.10
# Stable Windows identity: --device-uuid 00000000-0000-4000-8000-0000000000f2
```

Default port **3920** (fakeprint uses **3921**). Both can run on the same host with distinct UUIDs.

## Profiles

- `profiles/brother-usb.json` — high dpi, bmp → wire `dib`, optical, sticky feeder
- `profiles/brother-wsd.json` — 100/200/300 dpi, jpeg + tiff → wire `jfif` + `tiff-single-uncompressed` (Brother-shaped)
- `profiles/wia-lossless.json` — no jfif; WIA glass can ticket wire `dib` via FormatNotSupported fallback ([reference/format.md](docs/reference/format.md), [formats.md](docs/formats.md))
- `profiles/accept-dib.json` — `dib` only (SOAP lossless / `fakescan-lab acquire-dib`)
- `profiles/wia-rich.json` — all encodable WIA/WSD formats (see [docs/formats.md](docs/formats.md))
- Formats matrix: [docs/formats.md](docs/formats.md)

```bash
# lossless WIA BMP (restart or hot-switch; Imaging node already Added):
fakescan-lab --base-url http://127.0.0.1:3920 profile wia-lossless
# Brother-shaped JFIF wire again:
fakescan-lab --base-url http://127.0.0.1:3920 profile brother-wsd
```

Mirrored from a dump of a real network multifunction device, not product UI strings.

## Control plane (examples)

```bash
curl -X POST http://127.0.0.1:3920/v1/pages \
  -H "content-type: application/json" -d '{"count":2}'
curl -X POST http://127.0.0.1:3920/v1/pattern \
  -H "content-type: application/json" -d '{"id":"page_id"}'
curl -X POST http://127.0.0.1:3920/v1/effects \
  -H "content-type: application/json" -d '{"preset":"dirty_adf","seed":1}'
curl -X POST http://127.0.0.1:3920/v1/scenario \
  -H "content-type: application/json" -d '{"id":"order"}'
```

A fake scanner starts with an empty feeder, so a feeder scan fails until paper is loaded. Load it per run (`POST /v1/pages`, or `load_paper(n)` in fakescan-python) or start with paper: `fakescan --initial-pages 6` (every device without its own value), per device `initial_pages` in `POST /v1/devices`, or a profile's `feeder.initial_pages`. The default stays 0; see [docs/multi-device.md](docs/multi-device.md) section 4a.

Read-only, in this tree (they show up after the next start): `GET /v1/jobs` (scan-task list), `GET /v1/activity?after=SEQ`, and `GET /v1/log?after=SEQ` (last 64 calls; `after` returns only newer events). `GET /v1/trace` is the activity JSON. `POST /v1/ctl` takes `jobs` and `activity`. An older binary still on `:3920` answers 404 for `/v1/jobs`, `/v1/activity`, and `/v1/log`. Shapes and `.\scripts\fakescan.ps1 jobs`, `activity`, `log`, `wia-trace`: [docs/lab-harness.md](docs/lab-harness.md).

## Status

| Piece | State |
|-------|--------|
| Control API `/v1/*` (pages, pattern, effects, scenario, fault, **fault plan** (`/v1/faults/plan`: between pages / mid page / timed / now), slow, **loopback**) | working |
| Python lab client (`python/`, optional) | working — [lab-harness.md](docs/lab-harness.md) |
| Profiles `brother-usb` / `brother-wsd` | working |
| WS-Discovery Hello (+ ProbeMatch when port free) | working |
| WS-Scan SOAP + MTOM RetrieveImage (common WIA formats + multi-page) | wire OK — WIA accept still CancelJob ~30s / `0x80004005` ([e2e-wia-status.md](docs/e2e-wia-status.md), [mtom-retrieveimage.md](docs/specs/mtom-retrieveimage.md)) |
| WS-Transfer Get (ThisModel / ThisDevice / Host+Hosted + PnP-X) | working |
| Image effects (skew, speckles, streaks, hole punch) | working — [defects.md](docs/defects.md) |
| ColorProcessing RGB24 / Grayscale8 | working |
| Windows Imaging install + WIA Connect | **live** (person Add once); glass acquire **partial** — [e2e-wia-status.md](docs/e2e-wia-status.md) |
| Docker + GHCR | yes (`ghcr.io/b1tninja/fakescan:latest`) |
| WIA USD (USB-like local) | not yet |
| Extra test-page patterns / ANAC / more effects | [roadmap.md](docs/roadmap.md) |

## Lab harness (Python, optional)

For WIA / pdfcraft developers only — not a default app dependency:

```bash
pip install -e "../fakescan-python[dev]"
fakescan-lab loopback-soap page0.jpg page1.jpg   # SOAP exact-echo (no WIA)
pytest ../fakescan-python/tests -m live                      # needs fakescan running
```

See [docs/lab-harness.md](docs/lab-harness.md).

## Windows helpers

```powershell
.\scripts\fakescan.ps1 status
.\scripts\fakescan.ps1 jobs
.\scripts\fakescan.ps1 activity
.\scripts\fakescan.ps1 log
.\scripts\fakescan.ps1 wia-trace
.\scripts\fakescan.ps1 discoverable
.\scripts\fakescan.ps1 scenarios
.\scripts\fakescan.ps1 scenario order
.\scripts\fakescan.ps1 install -BaseUrl http://192.168.1.10:3920 -ProbeDiscovery -WaitSeconds 120
```

See [docs/windows.md](docs/windows.md) for soap-only vs on-wire-discoverable vs fd-installable vs installed.

## Develop

```bash
cargo fmt --all -- --check
cargo clippy -p fakescan --all-targets -- -D warnings
cargo test -p fakescan
pip install -e "../fakescan-python[dev]"
pytest ../fakescan-python/tests -m "not live and not local"
```

## CI

- `.github/workflows/ci.yml` — fmt / clippy / test on Ubuntu and Windows; Docker smoke; sane-airscan; Python lab
- `.github/workflows/docker.yml` — GHCR push

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

[MIT](LICENSE-MIT) OR [Apache-2.0](LICENSE-APACHE). Anything derived from Microsoft WIA samples must keep **MS-PL** notices.
