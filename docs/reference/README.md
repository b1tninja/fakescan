# WSD / WS-Scan reference (fakescan)

Distilled from **Microsoft’s** Scan Service Definition and Learn element docs — not from Brother HARs or lab guesswork. Use this folder when a WIA/wsdscdrv behavior looks mysterious; map gaps in [coverage.md](../coverage.md).

| Doc | Use when… |
|-----|-----------|
| [sources.md](sources.md) | You need the canonical download, namespaces, crawl date |
| [operations.md](operations.md) | Which SOAP ops/events exist (WSDL) |
| [format.md](format.md) | Image `Format` tokens, fault names, **inbox WIA fallback order** |
| [must-honor.md](must-honor.md) | `MustHonor` / `Override` / `UsedDefault` |
| [validate-vs-create.md](validate-vs-create.md) | ValidateScanTicket vs CreateScanJob (lossless path) |
| [faults.md](faults.md) | SOAP fault catalog from XSD + Learn |
| [schema/](schema/) | Official WSDL + XSD (from ScanService.zip) |

Stack overview (DPWS / Discovery / Eventing) still lives under [specs/](../specs/README.md). Lab status: [e2e-wia-status.md](../e2e-wia-status.md). Idea backlog: [roadmap.md](../roadmap.md).

## Why this exists

Lab 2026-10-12 wasted cycles on lossless WIA because the authoritative rules were scattered:

1. **Format** must be refused with a *named* format fault — then wsdscdrv retries PNG → EXIF → G4 → uncompressed TIFF ([format.md](format.md)).
2. CreateScanJob’s WSDL name is `ClientErrorFormatNotSupported`; the Format element page says `ClientErrorDocumentFormatNotSupported` — both matter ([faults.md](faults.md)).
3. **Format is not** on the HTML MustHonor allow-list, but the **XSD allows** `MustHonor` on Format and inbox WIA stamps it ([must-honor.md](must-honor.md)).
4. ValidateScanTicket returns `ValidTicket` + `ImageInformation`; a hard `false` on format can stall the CreateScanJob fallback loop ([validate-vs-create.md](validate-vs-create.md)).

## Rules of use

- Prefer **schema/** + linked Learn pages over paraphrases in `formats.md` / lab notes.
- When Learn and XSD disagree, record both (see Format fault names).
- Do not treat fakescan soft-accepts or profile knobs as “the spec” — label them as lab adaptations in coverage.
