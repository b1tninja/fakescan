# Headless: fakescan as a test device for client test suites and CI

`fakescan --headless` runs one scanner for a test suite to talk to, on loopback, and tells the
suite where it is. Nothing on the LAN learns of it.

```bash
fakescan --headless --exit-on-stdin-close --profiles profiles --profile wia-lossless --initial-pages 3
```

What headless changes:

| | Headless | Default run |
|---|---|---|
| Listen | `127.0.0.1:0` (an ephemeral port) unless `--listen` is given | `0.0.0.0:3920` |
| Scan Service (`/ScannerService`, `/WebServices/Device`) | on | on |
| WS-Discovery (Hello, Bye, Probe/Resolve on UDP 3702) | off | on |
| Tray icon, single-instance lock, named pipe | off | on (daemon runs) |
| Logs | stderr | stdout |

Without headless, `--no-discovery` alone keeps the Scan Service and turns off every
WS-Discovery message and the UDP listener.

## The ready line

Once the server accepts connections, it prints one line on stdout:

```text
FAKESCAN_READY {"base_url":"http://127.0.0.1:57262","scanner_url":"http://127.0.0.1:57262/ScannerService","port":57262,"devices":[{"id":"default","profile":"wia-lossless","scanner_url":"http://127.0.0.1:57262/ScannerService","device_url":"http://127.0.0.1:57262/WebServices/Device"}]}
```

* `scanner_url`: the default scanner's WS-Scan endpoint, for the client under test.
* `base_url`: the control plane (`/v1/pages`, `/v1/fault`, `/v1/faults/plan`, `/v1/jobs`, …), for
  the test to load paper and inject faults.
* `devices`: every scanner, including those added with `--device ID=PROFILE`.

Nothing else is written to stdout, so a harness can read the first line and parse the JSON after
`FAKESCAN_READY `.

## Stopping it

With `--exit-on-stdin-close`, closing the process's stdin shuts it down cleanly, so a harness that
owns the process needs no signals (and a harness that dies takes fakescan with it). Ctrl-C works
as well.

## A test, step by step

1. Start `fakescan --headless --exit-on-stdin-close ...` with stdin and stdout piped.
2. Read the first stdout line and take `scanner_url` and `base_url` from it.
3. Set the scene through the control plane, for example
   `POST {base_url}/v1/pages {"count": 3}` (paper in the feeder) or
   `POST {base_url}/v1/faults/plan` with a jam after the second page
   ([reference/faults.md](reference/faults.md)).
4. Scan with the client under test against `scanner_url`.
5. Check the result, and what fakescan saw (`GET {base_url}/v1/jobs`, `/v1/tickets`).
6. `POST {base_url}/v1/reset` between tests, or start one fakescan per test.
7. Close stdin.

The page fakescan returns is the same fixed size for every ticket, like the scanner its
Brother-shaped profiles model: at 300 dpi, 2480 x 3440 pixels (about 8.27 x 11.47 in), dropping to
150 dpi above a pixel budget. A client should report the size the image has, not the one it asked
for.
