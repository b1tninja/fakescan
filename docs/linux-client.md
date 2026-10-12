# Linux client (sane-airscan)

On Linux there is **no WIA**. Scanning goes through [SANE](http://www.sane-project.org/) backends. For network scanners that speak Microsoft WS-Scan (WSD), use **[sane-airscan](https://github.com/alexpevzner/sane-airscan)** — the same SOAP surface Windows reaches via `WSDScan.sys`.

| Host | Client stack | fakescan role |
|------|--------------|---------------|
| Windows | App → WIA → WSDScan → SOAP `/ScannerService` | Scan Service + optional discovery |
| Linux | App → SANE → sane-airscan → SOAP `/ScannerService` | Same Scan Service; prefer **manual URL** |

WS-Discovery (UDP 3702) often fails on Docker Desktop bridge networking even when SOAP on TCP 3920 works. CI and local Docker therefore pin the device URL instead of relying on multicast.

## Install (Ubuntu / GitHub Actions)

```bash
sudo apt-get update
sudo apt-get install -y sane-utils sane-airscan
```

Packages provide `scanimage` and the `airscan` SANE backend. Other distros: install `sane` / `sane-backends` plus `sane-airscan` from the distro or upstream.

## Point at fakescan (manual WSD URL)

Start fakescan (compose or binary) so `http://127.0.0.1:3920/health` succeeds.

### Manual config (preferred for CI / Docker)

WS-Discovery is flaky on Docker bridge networks. Disable discovery and pin the SOAP URL in `airscan.conf` (or a temp dir via `SANE_CONFIG_DIR`):

```ini
[options]
discovery = disable

[devices]
fakescan = http://127.0.0.1:3920/ScannerService, WSD
```

```bash
export SANE_CONFIG_DIR=/tmp/fakescan-sane-conf   # dir that contains airscan.conf
scanimage -L
# device `airscan:w0:fakescan' is a WSD fakescan ...
```

Ubuntu’s sane-airscan often prefixes manual devices (`airscan:w0:fakescan`). Use the exact name from `scanimage -L`.

### Optional env override

When supported by the installed sane-airscan build, `SANE_AIRSCAN_DEVICE` can override config:

```bash
export SANE_AIRSCAN_DEVICE='wsd:fakescan:http://127.0.0.1:3920/ScannerService'
```

Format: `wsd:<display-name>:<url>`. Prefer the config file above for CI — some packaged builds ignore the env unless discovery is also quieted.

## Example scanimage commands

Load a synthetic page on the control plane first:

```bash
curl -fsS -X POST http://127.0.0.1:3920/v1/pages \
  -H 'content-type: application/json' -d '{"count":1}'
```

List and probe options:

```bash
scanimage -L
DEV=$(scanimage -L | sed -n "s/^device \`\\(airscan:[^']*fakescan[^']*\\)'.*/\\1/p" | head -1)
scanimage -d "$DEV" -A
```

One-page acquire (PBM/PNG depending on backend defaults):

```bash
scanimage -d "$DEV" --format=png -o /tmp/fakescan-page.png
file /tmp/fakescan-page.png
```

Verbose backend tracing:

```bash
SANE_DEBUG_AIRSCAN=4 scanimage -d "$DEV" -n
```

(`-n` opens and reads options without transferring an image.)

## Smoke script

From the repo root, with Docker available:

```bash
docker compose up --build -d --wait
./scripts/linux-sane-smoke.sh
docker compose down
```

The script waits for `/health`, loads one page, lists devices, opens the device, and attempts a one-page acquire. Listing/open are required; acquire is best-effort if a SOAP gap blocks the scan job (exit still 0 unless `ACQUIRE_REQUIRED=1`). Against `GetScannerElements` → `CreateScanJob` → `RetrieveImage` (MTOM), a one-page PNG acquire has been observed to succeed.

On GitHub Actions, the `linux-sane` job in `.github/workflows/ci.yml` runs the same path on `ubuntu-latest`.

## Notes

- Host networking is only needed for **discovery** on a real LAN; SOAP over published port `3920` is enough for this client.
- Remote URL from another container: use the compose service hostname or `host.docker.internal` (Docker Desktop), not `127.0.0.1` inside that container.
- Windows WIA install and helpers: [windows.md](windows.md).
