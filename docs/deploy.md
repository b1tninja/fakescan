# Deploy with Docker

Docker is the primary way to run fakescan on a lab machine or CI host. The image exposes:

- **TCP 3920** — control API (`/v1/*`) and WSD Scan Service (`/ScannerService`)
- **UDP 3702** — WS-Discovery (only useful with host networking on Linux)

## Local

```bash
docker compose up --build -d
curl http://127.0.0.1:3920/health
curl http://127.0.0.1:3920/v1/status
```

Load feeder pages, switch profile:

```bash
curl -X POST http://127.0.0.1:3920/v1/pages \
  -H 'content-type: application/json' -d '{"count":3}'
curl -X POST http://127.0.0.1:3920/v1/profile \
  -H 'content-type: application/json' -d '{"id":"brother-usb"}'
```

## LAN advertise (Windows WSD install)

Bridge networking is enough for SOAP from the host. For **Windows to discover** the scanner, the container must send/receive multicast on the LAN:

```bash
# Linux
docker run --rm --network host \
  -e RUST_LOG=fakescan=info \
  ghcr.io/b1tninja/fakescan:latest \
  --listen 0.0.0.0:3920 \
  --advertise 192.168.1.10 \
  --profiles /app/profiles \
  --profile brother-wsd
```

On Docker Desktop (Windows/macOS), host networking is limited; run the binary on Windows, or put the container on a Linux VM on the same LAN.

## GHCR

Images publish from CI to `ghcr.io/b1tninja/fakescan`:

```bash
docker pull ghcr.io/b1tninja/fakescan:latest
docker run --rm -p 3920:3920 ghcr.io/b1tninja/fakescan:latest
```

## What Docker does *not* give you

Windows still must **install** the device (WSDScan) before pdfcraft’s WIA path can see it. The container is the Scan Service endpoint; Imaging Devices registration is a separate Windows step.
