# RetrieveImage MTOM packaging (wsdscdrv)

**Status (2026-10-12):** WIA JPEG/dib Transfer **accept cleared**. Root cause was XOP xmlns **`2003/12`** on the SOAP Envelope; Brother live / wsdscdrv need **`2004/08`**. Secondary: ImageInformation + JPEG SOF use Brother optical crop (`optical_page_pixels`), not full letter. Historical CancelJob A/B below is retained so MIME is not re-churned.

Packager: `crates/fakescan/src/wsd/mod.rs` → `build_mtom` / `mtom_response` (+ `RetrieveImage` arm that builds `xop:Include href="cid:…@uuid"`).

Do **not** treat sane-airscan or Brother shell scripts as proof of Windows-correct packaging (they ignore Content-ID).

## Current fakescan shape (accept path)

| Piece | What we emit |
|-------|----------------|
| Outer `Content-Type` | `multipart/related; type=application/xop+xml; boundary={uuid}; start="<{soap_cid}>"; start-info=…; startinfo=…` (both spellings) |
| `Mime-Version` | `1.0` |
| `Content-Length` | full multipart body length (no `Transfer-Encoding: chunked`) |
| SOAP part | `application/xop+xml; type="application/soap+xml"; charset=utf-8` + CTE `binary` + `Content-ID: <{uuid}@uuid>` |
| Envelope | `xmlns:xop="http://www.w3.org/2004/08/xop/include"` on `<soap:Envelope>` (`ensure_xop_xmlns_2004_08` replaces a wrong URI) |
| Image part | `Content-Type: image/jpeg` (no trailing `;`) + binary CTE + `Content-ID: <{same as cid:}>` |
| Boundary / CIDs | Bare UUID boundary (no `uuid:` prefix); image CID `{uuid}@uuid` matching MS sample shape |
| Body | Complete JFIF: `FFD8`…`FFD9`, SOF dims = ImageInformation optical crop, closing `--boundary--` |

Unit tests (no live WIA): `build_mtom_locks_wsdscdrv_accept_shape`, `ensure_xop_xmlns_*`, airscan RetrieveImage SOF/headers in `wsd_scan_flow.rs`, encode SOF vs `job_page_pixels` in `image_gen`.

## Prior art (what to cite)

| Source | Role | Path / URL |
|--------|------|------------|
| **MS RetrieveImageResponse sample** | Canonical packaging sketch for WSD Scan | [learn.microsoft.com …/retrieveimageresponse](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/retrieveimageresponse) · [raw md](https://github.com/MicrosoftDocs/windows-driver-docs/blob/staging/windows-driver-docs-pr/image/retrieveimageresponse.md) |
| **W3C SOAP 1.2 MTOM / XOP** | Outer `multipart/related`, `type=application/xop+xml`, root `application/xop+xml` + `type=application/soap+xml`, `start` / `start-info` | [SOAP MTOM](https://www.w3.org/TR/soap12-mtom/) · [XOP](https://www.w3.org/TR/xop10/) · [RFC 2387](https://datatracker.ietf.org/doc/html/rfc2387) (`start-info` with hyphen) |
| **Windows WebServices MTOM structs** | Document `start-info` / `start` parameter names | [WS_XML_READER_MTOM_ENCODING](https://learn.microsoft.com/en-us/windows/win32/api/webservices/ns-webservices-ws_xml_reader_mtom_encoding) |
| **sane-airscan (client)** | Speaks **WS-Scan** (2006/08) *and* **eSCL**; RetrieveImage decode takes **multipart part index 1**, does **not** match Content-ID | [airscan-wsd.c](https://github.com/alexpevzner/sane-airscan/blob/master/airscan-wsd.c) |
| **Brother RetrieveImage script** | Real device client; splits on boundary — **position-based**, not CID | [denysvitali/brother-scanner-scripts/retrieve-image.sh](https://github.com/denysvitali/brother-scanner-scripts/retrieve-image.sh) |

No vendored Brother HARs or MTOM captures under `D:\code\fakescan` today.

## Historical: MIME A/B before XOP fix (still CancelJob)

Copied from the MS sample (verbatim quirks kept):

```text
Content-Type: multipart/related;
    type=application/xop+xml;
    boundary=4aa7d814-adc1-47a2-8e1c-07585b9892a4;
    start="<14629f74-2047-436c-8046-5cac76d280fc@uuid>";
    startinfo=application/soap+xml
```

| Delta | MS sample | Lab A/B result (pre-2004/08) |
|-------|-----------|------------------------------|
| Outer param name | **`startinfo=`** (no hyphen) | Tried MS + RFC `start-info=` — **still CancelJob** |
| SOAP part CT order | `type=` then `charset=` | Tried type-before-charset — **still CancelJob** |
| Image `Content-Type` | `image/jpeg;` (trailing `;`) | Tried — **still CancelJob** |
| Image `Content-ID` | `<…@uuid >` (space before `>`) | Prefer no space; not the gate |
| `wsa:Action` | `…/RetrieveImage` | Tried vs `…/RetrieveImageResponse` — **same ~30s CancelJob** |
| Explicit `Content-Length` | (varies) | Tried — **same CancelJob** under wrong XOP NS |
| JPEG body / encode | “Binary Scan Data” | Marker-order / BPL / `wia_plain` A/B — not sole gate |

Full table: [e2e-wia-status.md](../e2e-wia-status.md). **Do not reopen MIME A/B** unless a Brother MTOM capture shows a header delta we never tried; packaging is locked by unit tests.

## Remaining live-only (not MTOM packaging)

| Item | Why live-only |
|------|----------------|
| Gray `CreateScanJob` → Pending, no RetrieveImage | **Fixed, live-proven 2026-10-12:** the old RGB24 coercion of a gray JFIF ticket was the cause. Default now answers consistent gray (FinalParams/ValidScanTicket Grayscale8, BPL = width, 1-component JPEG); legacy coercion is opt-in `jfif_gray=rgb24`. |
| Sticky offline after fakescan restart | PnP / AEP; no SOAP reaches packager |
| Associate without person Add | FD + elevation; not RetrieveImage |

## Cross-links

- Ops symptom / accept clear: [e2e-wia-status.md](../e2e-wia-status.md)
- Coverage row: [coverage.md](../coverage.md) RetrieveImage + MTOM → **done**
- Scan ops overview: [ws-scan.md](ws-scan.md)
- Windows stack: [windows-client.md](windows-client.md)
