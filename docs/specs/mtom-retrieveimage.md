# RetrieveImage MTOM packaging (wsdscdrv)

Lab symptom ([e2e-wia-status.md](../e2e-wia-status.md)): Connect OK → CreateScanJob → **RetrieveImage** returns `multipart/related` XOP+JPEG (~200KB), then inbox **wsdscdrv** ~30s → CancelJob / COM `0x80004005`. Wire transfer succeeds; **accept of the attachment** (not Connect / offline) is the gate.

Packager: `crates/fakescan/src/wsd/mod.rs` → `mtom_response` (+ `RetrieveImage` arm that builds `xop:Include href="cid:…@uuid"`).

**Lab verdict (2026-10-11):** MTOM packaging A/B vs the [MS RetrieveImageResponse sample](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/retrieveimageresponse) is **exhausted** — still CancelJob. Do **not** churn MIME headers further on that evidence. Next hypothesis: **JPEG payload / encode** (SOF sampling, APP0/JFIF, DHT/DQT, color transform) vs what wsdscdrv expects; optional Brother WSD JPEG byte-compare. `jpeg_com` toggle exists (`POST /v1/jpeg-com`, profile/env); off and on both CancelJob ~30s — COM markers alone are not the accept gate ([e2e-wia-status.md](../e2e-wia-status.md)).

Do **not** treat sane-airscan or Brother shell scripts as proof of Windows-correct packaging (they ignore Content-ID).

## Current fakescan shape (aligned to MS sample)

| Piece | What we emit (post A/B) |
|-------|-------------------------|
| Outer `Content-Type` | `multipart/related; type=application/xop+xml; boundary={uuid}; start="<{soap_cid}>"; startinfo=application/soap+xml` (MS spelling) **and** `start-info=` tried — both CancelJob |
| `Mime-Version` | `1.0` |
| SOAP part | `Content-Type: application/xop+xml; type="application/soap+xml"; charset=UTF-8` (type-before-charset tried) + CTE `8bit`/`binary` + `Content-ID: <{uuid}@uuid>` |
| Envelope | `xmlns:xop` on `<soap:Envelope>`; `wsa:Action` A/B `…/RetrieveImage` vs `…/RetrieveImageResponse` — same CancelJob |
| Image part | `Content-Type: image/jpeg;` (trailing `;` tried) + binary CTE + `Content-ID: <{same as cid:}>` |
| Boundary / CIDs | Bare UUID boundary (no `uuid:` prefix); image CID `{uuid}@uuid` matching MS sample shape |
| Body | Complete JFIF (~200KB): `FFD8`…`FFD9`, SOF dims match ImageInformation, closing `--boundary--`; optional COM via `jpeg_com` |

## Prior art (what to cite)

| Source | Role | Path / URL |
|--------|------|------------|
| **MS RetrieveImageResponse sample** | Canonical packaging sketch for WSD Scan | [learn.microsoft.com …/retrieveimageresponse](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/retrieveimageresponse) · [raw md](https://github.com/MicrosoftDocs/windows-driver-docs/blob/staging/windows-driver-docs-pr/image/retrieveimageresponse.md) |
| **MS CreateScanJobResponse sample** | Same docs use `wsa:Action` = `…/CreateScanJob` (no `Response` suffix) on the *response* | [createscanjobresponse](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/createscanjobresponse) — fakescan already uses `…Response` Actions for Create/Validate/etc. and WIA accepts those; Action A/B on RetrieveImage did not clear CancelJob |
| **W3C SOAP 1.2 MTOM / XOP** | Outer `multipart/related`, `type=application/xop+xml`, root `application/xop+xml` + `type=application/soap+xml`, `start` / `start-info` | [SOAP MTOM](https://www.w3.org/TR/soap12-mtom/) · [XOP](https://www.w3.org/TR/xop10/) · [RFC 2387](https://datatracker.ietf.org/doc/html/rfc2387) (`start-info` with hyphen) |
| **Windows WebServices MTOM structs** | Document `start-info` / `start` parameter names | [WS_XML_READER_MTOM_ENCODING](https://learn.microsoft.com/en-us/windows/win32/api/webservices/ns-webservices-ws_xml_reader_mtom_encoding) |
| **sane-airscan (client)** | Speaks **WS-Scan** (2006/08) *and* **eSCL**; RetrieveImage decode takes **multipart part index 1**, does **not** match Content-ID | [airscan-wsd.c](https://github.com/alexpevzner/sane-airscan/blob/master/airscan-wsd.c) (`wsd_load_decode` → `http_query_get_mp_response_data(..., 1)`) |
| **Brother RetrieveImage script** | Real device client; splits on boundary, saves part body after stripping `Content-*` lines — again **position-based**, not CID | [denysvitali/brother-scanner-scripts/retrieve-image.sh](https://github.com/denysvitali/brother-scanner-scripts/retrieve-image.sh) |
| **simple-scan WSD helper** | Finds JPEG SOI (`FF D8`) in the raw HTTP body | [shalommmitz/simple-scan/scan](https://github.com/shalommmitz/simple-scan/scan) |
| **bobi/WsdScanService** | Closest open **server** (.NET CoreWCF WS-Scan); useful for discovery/ops, not a published MTOM wire dump | [github.com/bobi/WsdScanService](https://github.com/bobi/WsdScanService) · listed in [prior-art.md](../prior-art.md) |
| **eSCL vs WSD** | Different stack: eSCL = HTTP REST (`/eSCL/…`), not SOAP+MTOM. A working eSCL path does not validate wsdscdrv packaging. | [specs/README.md](README.md) “Not this stack” |

No vendored Brother HARs or MTOM captures under `D:\code\fakescan` today — capture one live if available.

## Diff: MS sample vs lab (historical)

Copied from the MS sample (verbatim quirks kept):

```text
Content-Type: multipart/related;
    type=application/xop+xml;
    boundary=4aa7d814-adc1-47a2-8e1c-07585b9892a4;
    start="<14629f74-2047-436c-8046-5cac76d280fc@uuid>";
    startinfo=application/soap+xml
```

| Delta | MS sample | Lab A/B result |
|-------|-----------|----------------|
| Outer param name | **`startinfo=`** (no hyphen) | Tried MS + RFC `start-info=` — **still CancelJob** |
| SOAP part CT order | `type=` then `charset=` | Tried type-before-charset — **still CancelJob** |
| Image `Content-Type` | `image/jpeg;` (trailing `;`) | Tried — **still CancelJob** |
| Image `Content-ID` | `<…@uuid >` (space before `>`) | Prefer no space; not pursued after other MIME fails |
| `wsa:Action` | `…/RetrieveImage` | Tried vs `…/RetrieveImageResponse` — **same ~30s CancelJob** |
| Explicit `Content-Length` | (varies) | Tried — **same CancelJob** |
| JPEG body | “Binary Scan Data” | JFIF ~200KB; **`jpeg_com` off/on both CancelJob**; BMP/TIFF early fail (no 30s path) |

Live dump: `D:\temp\fakescan-retrieve.mtom`. Full table: [e2e-wia-status.md](../e2e-wia-status.md).

## Next checklist (payload — packaging closed)

Do **not** reopen MIME A/B unless a Brother (or other working WSD) MTOM capture shows a header delta we never tried.

1. **JPEG encode / bitstream** — SOF sampling, APP0/JFIF version, DHT/DQT, color transform vs wsdscdrv expectations.
2. **`jpeg_com` remains available** for encode trials (`POST /v1/jpeg-com` / profile) — already ruled out as sole gate.
3. **Force Format `dib` or `tiff-single-uncompressed`** — already early-fail live; useful only if encode path changes.
4. **ImageInformation / DocumentFinalParameters** vs attachment dims — re-verify after encode changes (dims already matched once).
5. **Capture Brother (or any working WSD scanner) RetrieveImage response** — outer CT, part headers, CID vs `xop:Include`, and **JPEG bytes**. Diff against fakescan.

### Out of scope for this gate

- eSCL / AirPrint success
- sane-airscan success (part[1] only)
- Claiming the device is WIA-offline — Connect passes; sticky offline after restart is a separate recovery path ([windows-client.md](windows-client.md))

## Suggested isolated test (no live process)

When editing the packager or encoder, prefer a **small pure helper** with unit tests asserting:

- outer CT contains the chosen `start-info` / `startinfo` spelling
- SOAP + image `Content-ID: <…>` angle brackets
- `xop:Include` cid string equals image CID (no `<>` inside `cid:`)
- CRLF part separators and closing `--boundary--`
- (encode) JFIF markers / SOF dims match claimed ImageInformation

Keep helpers + tests in `crates/fakescan`; avoid drive-by edits to the live dispatcher while other agents own `mod.rs`.

## Cross-links

- Ops symptom: [e2e-wia-status.md](../e2e-wia-status.md)
- Scan ops overview: [ws-scan.md](ws-scan.md)
- Windows stack: [windows-client.md](windows-client.md)
- Servers / clients list: [prior-art.md](../prior-art.md)
