# fakescan (Python)

Optional **control-plane client** for the fakescan binary. Not a native extension — the Rust server still owns WSD/WIA; this package scripts `/v1/*`, hard loopback, and lab asserts.

Install only for scan-lab work (pdfcraft / WIA developers). It is **not** a dependency of pdfcraft or jason.

```bash
# from fakescan repo
pip install -e "./python[dev]"
# terminal 1
cargo run -p fakescan -- --profiles profiles --profile brother-wsd
# terminal 2
pytest python/tests -m live
```

## Why Python

- pytest + Pillow for loopback upload/expect (byte-identical JFIF)
- Same language as pypdfcraft / jason scan tests
- PowerShell stays for Windows install / discovery; Python owns the harness

## Quick use

```python
from fakescan import Client

c = Client("http://127.0.0.1:3920")
c.reset()
c.set_pages(3, side="front")
c.set_fault(jam_after_n=2)
digests = c.set_loopback_files(["page0.jpg", "page1.jpg"])
# acquire via WIA / pdfcraft-cli, then:
assert digests == c.loopback().digests
```

```bash
# SOAP hard echo (CI)
fakescan-lab loopback-soap fixtures/a.jpg fixtures/b.jpg
# WIA + optional pdfcraft-cli (lab machine with Fake installed)
fakescan-lab loopback-wia fixtures/a.jpg --pdfcraft-cli path\to\pdfcraft-cli.exe
```

See [docs/lab-harness.md](../docs/lab-harness.md). Print sibling (capture-assert, not loopback): [fakeprint](../../fakeprint/python/README.md) / [siblings](../docs/siblings.md).
