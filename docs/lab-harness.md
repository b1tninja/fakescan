# Lab harness (Python)

The WIA / pdfcraft scan harness lives **in fakescan**, not in pdfcraft. Print labs use the sibling **[fakeprint](../../fakeprint)** ([siblings.md](siblings.md)) — capture-assert on `:3921`, not this loopback store.

| Layer | Where | Role |
|-------|--------|------|
| Server | Rust `fakescan` binary | WSD + control plane + hard loopback store |
| Harness | `python/` package (`pip install -e ./python`) | Arm `/v1/*`, SOAP acquire, digest/byte expect |
| Windows install | `scripts/Fakescan.psm1` | Discovery, Add device, rebind (person required) |
| App under test | pdfcraft-cli (optional) | Real WIA path — **not** a fakescan dependency |

pdfcraft must not take a fakescan (or this package) dependency for normal builds.

## Soft vs hard ground truth

| Mode | How | Expect |
|------|-----|--------|
| Soft | Synthetic patterns + JPEG COM / QR | Markers / visual cues |
| **Hard loopback** | `POST /v1/loopback` with uploaded JFIF bytes | Acquired bytes **==** original |

Hard loopback is the “echo this page back” store: upload → CreateScanJob / WIA acquire → compare.

Modes are mutually exclusive: `POST /v1/loopback` clears pattern/side/scenario; `POST /v1/pages` (with or without side), `pattern`, or `scenario` clears the loopback store. Effects do **not** alter loopback JFIF bytes (exact echo).

```bash
# terminal 1
cargo run -p fakescan -- --profiles profiles --profile brother-wsd

# terminal 2
pip install -e "./python[dev]"
# any local JFIGs — CLI resets before arming
fakescan-lab loopback-soap python/fixtures/a.jpg python/fixtures/b.jpg
# or via PowerShell CLI:
.\scripts\fakescan.ps1 lab loopback-soap python/fixtures/a.jpg python/fixtures/b.jpg
cd python && pytest -m live
```


## Control plane additions

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/v1/loopback` | count + digests (`len:first8hex`) |
| `POST` | `/v1/loopback` | `{"pages":["base64…",…]}` — replace store; empty clears |
| `DELETE` | `/v1/loopback` | clear store |

`GET /v1/status` also exposes `loopback_pages` and `loopback_digests`.

JFIF RetrieveImage returns **exact** uploaded bytes. Multi-page Formats are refused for loopback — use `jfif` for byte-identical compare.

## Python API

```python
from fakescan import Client

with Client("http://127.0.0.1:3920") as c:
    c.reset()
    c.set_loopback_files(["front0.jpg", "front1.jpg", "front2.jpg"])
    # … WIA / pdfcraft-cli acquire …
    c.expect_loopback_files(["front0.jpg", "front1.jpg", "front2.jpg"])

# Soft / fault paths stay separate from loopback:
with Client() as c:
    c.reset()
    c.set_pages(3, side="front")   # clears any prior loopback
    c.set_fault(jam_after_n=2)
```

SOAP-only (CI, no WIA): `from fakescan.soap import acquire_jfif_pages` or `fakescan-lab loopback-soap`.

## Optional pdfcraft (WIA)

Needs Fake listed in Imaging / `pdfcraft-cli scan devices`. WIA usually **re-encodes** JPEG, so compare defaults to `auto` (exact, else Pillow pixels). Use `loopback-soap` for hard byte identity.

```bash
fakescan-lab loopback-wia page0.jpg page1.jpg \
  --pdfcraft-cli D:/code/pdfcraft/target/release/pdfcraft-cli.exe
# arm only (no CLI):
fakescan-lab loopback-wia page0.jpg --arm-only
```

Never add pdfcraft as a Cargo/Python dependency of fakescan — pass `--pdfcraft-cli` or PATH only in the lab.
