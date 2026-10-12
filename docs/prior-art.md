# Prior art

## Microsoft WIA samples

- Repo: [microsoft/Windows-driver-samples](https://github.com/microsoft/Windows-driver-samples/tree/main/wia)
- Notable trees: `wiadriverex`, `ProdScan`
- License: Microsoft Public License (MS-PL)
- Installs a real Imaging Devices entry; emits synthetic bitmaps
- Needs WDK tooling; not profile-driven; not a network Scan Service

## TWAIN software DS + WIA-on-TWAIN

- [twain/twain-samples](https://github.com/twain/twain-samples)
- [twain/wia-on-twain](https://github.com/twain/wia-on-twain)
- Useful for “something shows up,” poor fit for scripting WIA property matrices and sticky feeder quirks

## App-layer mocks

- NAPS2 and similar keep `MockScanBridge`-style doubles inside the process
- Never validates WIA COM, WSDScan.sys, or capability publication

## WSD Scan Service (closest)

- [bobi/WsdScanService](https://github.com/bobi/WsdScanService) — .NET / CoreWCF WS-Scan + WS-Discovery, Docker-oriented
- Proves user-mode network scan is viable without a kernel driver
- Not built around dual USB/WSD capability profiles or a control plane for CI

## RetrieveImage MTOM (Windows accept)

Open **clients** (not Windows-faithful packagers): [sane-airscan `airscan-wsd.c`](https://github.com/alexpevzner/sane-airscan/blob/master/airscan-wsd.c) (multipart part\[1\]), [brother-scanner-scripts](https://github.com/denysvitali/brother-scanner-scripts/blob/master/retrieve-image.sh). Canonical response sketch: [RetrieveImageResponse](https://learn.microsoft.com/en-us/windows-hardware/drivers/image/retrieveimageresponse). Packaging A/B vs MS sample is **closed** (still CancelJob); next is JPEG payload — **[specs/mtom-retrieveimage.md](specs/mtom-retrieveimage.md)** · live log [e2e-wia-status.md](e2e-wia-status.md).

Protocol specs (Microsoft Scan Service Definition, DPWS, WS-Discovery, Transfer, Eventing): **[specs/](specs/README.md)**.

## Naming

Chose **fakescan** over fakeprint / printserv: this project is scan-only. Print simulation is the sibling repo **[fakeprint](../../fakeprint)** (WSD Print / WSDMON first; IPP Everywhere scaffolded for Phase 2) — not an IPP-only printserv fork inside this tree.

Ports, ground truth, and “do not copy” table: **[siblings.md](siblings.md)**. Print lab harness: [fakeprint/docs/lab-harness.md](../../fakeprint/docs/lab-harness.md).
