# WS-Transfer

How a client **reads device metadata** from the Device EPR after discovery (no scan ticket yet).

## Spec status

| Question | Answer |
|----------|--------|
| IETF RFC? | **No** |
| Primary cite for DPWS / WSD | [Web Services Transfer (WS-Transfer) — W3C Member Submission](https://www.w3.org/Submission/WS-Transfer/) (2006-09-27) |
| Later work | W3C WS-Transfer / WS-ResourceTransfer evolved separately; **Windows WSD devices use the Submission-shaped Get** |

Action URI on the wire:

```text
http://schemas.xmlsoap.org/ws/2004/09/transfer/Get
```

Response action:

```text
http://schemas.xmlsoap.org/ws/2004/09/transfer/GetResponse
```

## What Get means for a scanner

1. Client learned an HTTP address from Hello / ProbeMatch / ResolveMatch **XAddrs** (Device URL).
2. Client POSTs SOAP `Get` with **empty Body** to that URL.
3. Device returns Metadata (ThisModel / ThisDevice / Relationship) — see [dpws.md](dpws.md).

This is **not** `wscn:GetScannerElements`. Transfer Get describes the *device*; Scan GetScannerElements describes *scan capabilities*.

**`MetadataVersion` is not part of Get.** It lives on Hello / ProbeMatch / ResolveMatch only. A lab dump of Get XML without `MetadataVersion` matches DPWS; clients invalidate cached Get when discovery `MetadataVersion` increases.

## Windows behavior (lab)

After Hello, `dasHost` / WSD stack repeatedly Transfer-Gets the Device URL (fakescan log: `short=Get` on `/WebServices/Device`, often every ~5s from another LAN host or locally). That poll keeps association / presence warm; it is not a scan job.

HTTP **GET** (no SOAP) on `/WebServices/Device` correctly returns **405** — only SOAP POST is defined.

## fakescan mapping

| Piece | Value |
|-------|--------|
| Device path | `/WebServices/Device` (+ trailing slash) |
| Handler | same SOAP dispatcher as Scan (`wsd::scanner_service`), branch `short=Get` |
| Body | Host + Hosted Relationship, PnP-X HardwareId / CompatibleId, FriendlyName |

Advertise IP in `--advertise` must match what Windows will use in XAddrs / LocationInformation after install.
