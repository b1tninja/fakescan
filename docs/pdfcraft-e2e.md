# pdfcraft WIA E2E harness

The parametrized **pdfcraft + fakescan** WIA matrix moved to the sibling repo **[pdfcraft-fakescan](../../pdfcraft-fakescan/README.md)**.

- Install: `pip install -e ../fakescan-python` then `pip install -e ../pdfcraft-fakescan`
- Run: `pytest -m pdfcraft_wia` from `pdfcraft-fakescan`
- Findings: [pdfcraft-findings.md](pdfcraft-findings.md)

fakescan’s own Python package keeps the HTTP/SOAP lab (`pytest -m live`, `-m local`). `fakescan-lab loopback-wia` optionally imports **pdfcraft-fakescan** for pdfcraft-cli comparison.
