# fakescan

<p align="center">
  <img src="docs/assets/fakescan-logo.svg" alt="fakescan" width="220" />
</p>

A **fake scanner** for testing acquire paths: scriptable glass/feeder, synthetic page patterns, image defects, and capability profiles that look like real WIA / WSD devices â€” without a physical MFP.

Built for [pdfcraft](https://github.com/) and similar apps (sticky `FEED_READY`, page counts, format refusals, deskew/despeckle stress). Companion **[fakeprint](https://github.com/b1tninja/fakeprint)** covers WSD Print on `:3921` â€” [docs/siblings.md](docs/siblings.md).

## Docs

| Doc | Start here forâ€¦ |
|-----|-----------------|
| [docs/README.md](docs/README.md) | Full index |
| [docs/siblings.md](docs/siblings.md) | vs fakeprint (ports, loopback â‰  capture) |
| [docs/lab-harness.md](docs/lab-harness.md) | Python client + hard loopback (optional) |
| [docs/deploy.md](docs/deploy.md) | Docker / LAN |
| [docs/windows.md](docs/windows.md) | WIA install assist (person clicks Add device) |
| [docs/linux-client.md](docs/linux-client.md) | sane-airscan |
| [docs/patterns.md](docs/patterns.md) | Page layouts + tester progression |
| [docs/defects.md](docs/defects.md) | Skew, speckles, streaks, hole punches, Gray8 |
| [docs/roadmap.md](docs/roadmap.md) | Backlog (test pages, ANAC, more effects, â€¦) |
| [docs/prior-art.md](docs/prior-art.md) | Why not WIA samples alone |

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
# Stable Windows identity: --device-uuid 00000000-0000-4000-8000-000000000001
```

## Profiles

- `profiles/brother-usb.json` â€” high dpi, bmp â†’ wire `dib`, optical, sticky feeder  
- `profiles/brother-wsd.json` â€” 100/200/300 dpi, jpeg + tiff â†’ wire `jfif` + `tiff-single-uncompressed`  
- `profiles/wia-rich.json` â€” all encodable WIA/WSD formats (see [docs/formats.md](docs/formats.md))  
- Formats matrix: [docs/formats.md](docs/formats.md)

Mirrored from a Brother MFC-9330CDW dump, not product UI strings.

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

## Status

| Piece | State |
|-------|--------|
| Control API `/v1/*` (pages, pattern, effects, scenario, fault, slow, **loopback**) | working |
| Python lab client (`python/`, optional) | working â€” [lab-harness.md](docs/lab-harness.md) |
| Profiles `brother-usb` / `brother-wsd` | working |
| WS-Discovery Hello (+ ProbeMatch when port free) | working |
| WS-Scan SOAP + MTOM RetrieveImage (common WIA formats + multi-page) | wire OK â€” WIA accept still CancelJob ~30s / `0x80004005` ([e2e-wia-status.md](docs/e2e-wia-status.md), [mtom-retrieveimage.md](docs/specs/mtom-retrieveimage.md)) |
| WS-Transfer Get (ThisModel / ThisDevice / Host+Hosted + PnP-X) | working |
| Image effects (skew, speckles, streaks, hole punch) | working â€” [defects.md](docs/defects.md) |
| ColorProcessing RGB24 / Grayscale8 | working |
| Windows Imaging install + WIA Connect | **live** (person Add once); glass acquire **partial** â€” [e2e-wia-status.md](docs/e2e-wia-status.md) |
| Docker + GHCR | yes |
| WIA USD (USB-like local) | not yet |
| Extra test-page patterns / ANAC / more effects | [roadmap.md](docs/roadmap.md) |

## Lab harness (Python, optional)

For WIA / pdfcraft developers only â€” not a default app dependency:

```bash
pip install -e "./python[dev]"
fakescan-lab loopback-soap page0.jpg page1.jpg   # SOAP exact-echo (no WIA)
pytest python/tests -m live                      # needs fakescan running
```

See [docs/lab-harness.md](docs/lab-harness.md).

## Windows helpers

```powershell
.\scripts\fakescan.ps1 status
.\scripts\fakescan.ps1 discoverable
.\scripts\fakescan.ps1 scenarios
.\scripts\fakescan.ps1 scenario order
.\scripts\fakescan.ps1 install -BaseUrl http://192.168.1.10:3920 -ProbeDiscovery -WaitSeconds 120
```

See [docs/windows.md](docs/windows.md) for soap-only vs on-wire-discoverable vs fd-installable vs installed.

## CI

- `.github/workflows/ci.yml` â€” fmt / clippy / test; Docker smoke; optional linux-sane  
- `.github/workflows/docker.yml` â€” GHCR push  

## License

MIT OR Apache-2.0 (project-original). Anything derived from Microsoft WIA samples must keep **MS-PL** notices.
