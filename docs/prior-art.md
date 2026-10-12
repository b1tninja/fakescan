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

## Naming

Chose **fakescan** over fakeprint / printserv: this project is scan-only. Print simulation would be a different stack (IPP Everywhere / Windows print queue).
