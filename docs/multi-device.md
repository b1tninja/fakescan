# Multi-device fakescan (design contract)

One fakescan process emulates **N scanners at once**, each with its own profile, UUID, endpoints and
feeder configuration (tray size, media sizes, duplex). Goal: let pdfcraft and the harness test every
tray/feeder shape side by side (small tray, big tray, no feeder, duplex, A4-only, ...).

This file is the contract between components. If you must deviate, change this file in the same
change and say so in your report.

## 1. Device model

```rust
pub struct DeviceConfig {
    pub id: String,                 // URL-safe slug, unique: "tray5", "duplex50". "default" = first/legacy device
    pub profile: String,            // profile id (profiles/<id>.json)
    pub device_uuid: Uuid,          // distinct per device; default device keeps lab::LAB_DEVICE_UUID
    pub friendly_name: Option<String>, // shown in Windows; default "fakescan <id>"
}
pub struct ServerConfig { /* existing fields */ pub devices: Vec<DeviceConfig> }
```

**Identity is the UUID, never the name.** Every fake device UUID (fakescan and fakeprint) starts with
`lab::FAKE_UUID_PREFIX = "00000000-0000-4000-8000-"` (`fakescan::FAKE_UUID_PREFIX`, `fakescan::is_fake_uuid`;
Python `fakescan.FAKE_UUID_PREFIX` / `is_fake_uuid`). This is a lab-safety feature: `DeviceManager::add`
(and so `serve`, `--device ID=PROFILE@UUID`, `--device-uuid` and `POST /v1/devices`) rejects a device UUID
outside the prefix (HTTP 400) unless `DeviceConfig::allow_foreign_uuid` is set (CLI `--allow-foreign-uuid`;
`ServerConfig::allow_foreign_uuid` for `--manage` registry rows, which are otherwise skipped with a warning).
The override is never reachable over HTTP. The default UUID (`...00f2`) and derived UUIDs satisfy the rule;
`World`s and the Python binding get random UUIDs inside the prefix when none is given. Friendly names are
cosmetic and never decide whether a device is ours.

- `ServerConfig::lab(..)` keeps working: it yields one device, id `default`.
- Each device owns its own `World` (state, jobs, activity ring, loopback, faults). No shared mutable state.
- Instance lock, named pipe, listener: one per **server**, not per device.

## 2. HTTP surface

| Path | Meaning |
|------|---------|
| `GET /v1/devices` | `[{id, uuid, profile, friendly_name, scan_endpoint, device_endpoint}]` |
| `/v1/*`, `/ScannerService`, `/WebServices/Device` | **legacy aliases of the `default` (first) device**; unchanged behavior |
| `/d/{id}/v1/*` | the full control plane, scoped to device `{id}` |
| `/d/{id}/ScannerService`, `/d/{id}/WebServices/Device` (+ trailing-slash and `/WebServices/ScannerService` variants) | that device's WSD endpoints |

Unknown `{id}` -> 404 JSON `{"error":"unknown device"}`.

## 3. WS-Discovery

One announcer per device when `discovery` is on: its own UUID, XAddr =
`http://<adv>:<port>/d/{id}/WebServices/Device` (the `default` device keeps the legacy
`/WebServices/Device` XAddr). Hello/Bye per device; Probe/Resolve answered per device.

## 4. Feeder / tray configuration (profile schema)

Profile `feeder` block gains (all optional, backwards compatible, defaults = today's behavior):

```json
"feeder": {
  "capacity": 30,                 // sheets the tray holds; set_pages(n>capacity) is clamped/refused (see below)
  "media_sizes": ["letter","a4","legal"],   // sizes the ADF accepts; drives InputMediaSize / ADF sizes published
  "duplex": false,
  "pages_min": 0, "pages_max": 30  // existing; pages_max must be <= capacity when both set
}
```

- `capacity` absent = today's behavior (no tray limit beyond pages_max).
- Loading more sheets than `capacity` via `POST /v1/pages` -> `409` with a readable message and
  status unchanged (a real tray cannot hold them). `Status` exposes `feeder_capacity`.
- A profile with no feeder (glass only) keeps working.
- Scenario profiles shipped for tests (names are part of the contract):
  `feeder-tray5` (capacity 5, letter), `feeder-tray50` (capacity 50, letter+a4+legal),
  `feeder-duplex30` (capacity 30, duplex), `feeder-a4only` (capacity 20, a4 only), `glass-wsd` (no feeder, exists).
  All are WSD, jfif, same imaging caps as `brother-wsd`.

## 5. Python (fakescan-python)

```python
from fakescan import Server, Device
with Server(devices=[Device("tray5", profile="feeder-tray5"),
                     Device("tray50", profile="feeder-tray50")]) as srv:
    srv.devices["tray5"].client().set_pages(5)      # Client scoped to /d/tray5
    srv.devices["tray5"].scan_endpoint              # http://127.0.0.1:PORT/d/tray5/ScannerService
    srv.devices["tray5"].uuid
# Server() with no devices == one "default" device (today's behavior); srv.client() is the default device's.
Client(base_url, device="tray5")                    # explicit scoping against any server
```

## 6. Harness (pdfcraft-fakescan)

The scenario matrix is parametrized over **device configs** (the profiles in section 4). Windows needs each
emulated scanner Added in Imaging (distinct UUID => distinct WIA device); the harness picks devices by
fakescan UUID in the WIA port string, never by name guess, and refuses any device whose UUID is not
one it started. Never scan a device that is not provably fakescan's.

## 7. Ownership during implementation

| Component | Owns |
|-----------|------|
| fakescan: server | `server.rs`, `main.rs`, `discovery.rs`, `lab.rs`, `registry.rs`, `ctl.rs`, `pipe.rs` |
| fakescan: tray/profile | `profile.rs`, `state.rs`, `wsd/elements.rs`, `wsd/responses.rs`, `profiles/*.json`, tests next to them |
| fakescan-python | the whole repo |
| pdfcraft-python | the whole repo |
| pdfcraft-fakescan | the whole repo |
| pdfcraft | read-only unless a task says otherwise |

Nobody commits. Each agent builds with its own `CARGO_TARGET_DIR` (never `D:\temp\fakescan-target-optical`;
a fakescan.exe from it is running and locks the file).

## 8. Persistent device registry and install tracking

`devices.json` (`registry.rs`, `%LOCALAPPDATA%\fakescan\devices.json`) becomes **one row per emulated
device** (not per process). Rows that share a `listen` belong to one server process; `ensure` starts one
process per distinct `listen` with a `--device ID=PROFILE@UUID` for each of its rows. Old single-row files
must keep loading unchanged (`added.pnp` stays readable).

Each row records identity, configuration and **install lifecycle**, persisted after every state change:

```jsonc
{
  "name": "tray5",                       // device id (slug)
  "listen": "0.0.0.0:3920", "advertise": "192.168.1.70",
  "device_uuid": "00000000-0000-4000-8000-0000000000a5",
  "profile": "feeder-tray5",
  "friendly_name": "fakescan tray5",
  "config": {                            // what this device was when last started
    "fingerprint": "sha256:...",         // hash of the effective profile (feeder capacity, media, duplex, formats, dpi, colors)
    "feeder": {"capacity": 5, "media_sizes": ["letter"], "duplex": false}
  },
  "install": {
    "state": "installed",                // new | probed | installed | stale
    "first_probed_at": "2026-10-12T17:50:01Z",  // fakescan saw a WS-Discovery Probe/Resolve or a Get* from a peer
    "last_seen_at": "...", "last_peer": "192.168.1.40",
    "installed_at": "...",               // a WIA device bound to this UUID was observed
    "verified_at": "...",                // last successful reconcile against the Windows device list
    "wia": {"id": "{6BDD1FC6-...}\0003", "name": "fakescan tray5", "port": "urn:uuid:<uuid>/uri:<uuid>/ScannerService"},
    "fingerprint_at_install": "sha256:..."
  }
}
```

State rules:

- `new` -> `probed`: fakescan itself records the first discovery/metadata contact from a peer (it sees
  it; no Windows API needed). `last_seen_at` / `last_peer` update on any WSD request (throttled; do not
  rewrite the file on every SOAP call).
- `probed` -> `installed`: **only observed on the Windows side** and reported in via `POST /v1/ctl`
  `installed {device, wia_id, wia_name, wia_port}`. Callers: `fakescan-python`
  (`fakescan.registry.reconcile(wia_scanners)`, which matches a scanner row to a device by
  **the UUID in the WIA `port` string**, never by name) and the harness. Replaces the boolean `added.pnp`
  (keep it in sync for compatibility).
- `installed` -> `stale`: reconcile finds no WIA device with that UUID anymore, **or** the device's current
  `config.fingerprint` differs from `fingerprint_at_install` (Windows caches GetScannerElements at Add
  time, so a changed profile needs remove + re-Add; see pdfcraft-findings.md). The state carries a reason.
- Never mark anything installed on a name guess; a WIA device whose port UUID is not in the registry is
  reported as **foreign** and must not be scanned by the harness.

Writes: atomic (write temp file in the same directory, fsync, rename), serialized with an in-process
mutex and an OS file lock so the fakescan process and Python callers do not clobber each other. Reads
tolerate unknown fields (forward compat). Timestamps RFC 3339 UTC.

Surfaces: `GET /v1/devices` includes `install` and `config`; `GET /v1/registry` returns the document;
`POST /v1/ctl` gains `installed`, `forget` (drop install info, keep the row) and `probed` is internal.
`scripts/fakescan.ps1 list` shows state. Python: `fakescan.registry.load()/save()/reconcile()`.

## 9. fakescan as a manager of fake scanner devices

fakescan is a **device manager**: the registry (section 8) is the source of truth for the fleet, and
the running process reconciles itself to it.

- **Runtime device CRUD** (no restart; restarts break Windows' PnP association): `POST /v1/devices`
  `{id, profile, device_uuid?, friendly_name?}` adds and starts a device (allocates a UUID if omitted,
  announces Hello when discovery is on, persists a registry row); `DELETE /v1/devices/{id}` stops it
  (Bye), keeps or drops the row (`?forget=true`); `POST /v1/devices/{id}/reconfigure {profile}` swaps
  the profile **and marks the device `stale` if it was installed** (Windows must re-Add).
  `GET /v1/devices` lists all with `install` + `config`.
- **Implementation constraint (affects routing):** the device set is dynamic, so routes must NOT be
  built by statically nesting one router per device at startup. Use a single `/d/{id}/...` dispatch
  that looks the device up in a shared `Arc<RwLock<DeviceMap>>` at request time (404 if absent), and
  one legacy alias route that resolves to the `default` device the same way. Discovery announcers are
  likewise started/stopped per device through the manager.
- **Start-up = reconcile:** with `--manage` (and when `--device` flags are absent) the process loads the
  registry rows whose `listen` matches its own and brings each up; it never rewrites a row's UUID.
- **Manager API surface** (one place, used by HTTP, ctl, the PowerShell script and Python):
  `DeviceManager::{list, get, add, remove, reconfigure, mark_probed, mark_installed, mark_stale,
  forget, reconcile_with_windows(listing)}` in a new `manager.rs`. The HTTP handlers and `ctl.rs`
  commands are thin wrappers over it.
- **Python:** `fakescan.Manager(base_url)` mirrors it (`add`, `remove`, `reconfigure`, `devices()`,
  `reconcile(wia_scanners)`); `fakescan.Server` (in-process) exposes the same manager for its own
  devices, so tests can add and remove scanners mid-test.
- **Harness:** pdfcraft-fakescan asks the manager for devices by config (`feeder-tray5`, ...), adds
  them if missing, tells the human exactly which ones need an Imaging Add (state != installed), and
  after the Add calls `reconcile` so the registry records the install.

## 10. Orchestration: one daemon for Windows, ephemeral servers for everything else

Decision. **One process hosts all devices** (one port, one lock, one discovery loop, one registry).
Process-per-device was rejected: it multiplies ports (even-port band), firewall rules and instance
locks, and buys nothing, because the expensive thing is not the process, it is the **Windows Add**.
Windows keys a scanner to `UUID + advertise URL:port` and **caches GetScannerElements at Add time**.
So what must stay stable is identity (UUID, URL, port), and what must not change without a re-Add is
the *published* capability set. A restart with the same listen + UUIDs + profiles is invisible to Windows.

Two modes, chosen by purpose, so tests never add scanners to Windows:

| Mode | Used for | Lifetime | Persists? | Windows devices |
|------|----------|----------|-----------|-----------------|
| **Daemon** (`fakescan --manage`, autostart at logon, managed via `ctl` / HTTP / `fakescan-ctl`) | WIA / pdfcraft end-to-end | long-lived | yes: registry rows, UUIDs, install state | Added **once** per capability class, then reused forever |
| **Embedded** (`fakescan.Server(...)`, in-process, port 0) | SOAP / control-plane / matrix breadth | one test or session | no registry, no discovery | none; devices are created and destroyed per test |

- **Capability classes, not tests, are what get installed.** Two device configs can share one installed
  Windows device if they differ only in *runtime* state (pages loaded, faults, loopback images, image
  pattern, slow/effects, and tray `capacity`, which is not published). A device needs its own Add only
  if its **published fingerprint** differs (ADF max size, duplex, formats, dpi, colors, `pages_max`,
  glass vs feeder).
- Therefore the registry tracks two hashes: `config.fingerprint` (everything) and
  `config.published_fingerprint` (hash of the capability elements Windows caches, i.e. the
  GetScannerElements-visible parts). `install.published_fingerprint_at_install` replaces
  `fingerprint_at_install` for the staleness rule. **reconfigure marks an installed device `stale` only if the
  published fingerprint changed**; changing capacity/pages/faults/etc. never does. `GET /v1/devices` shows
  `needs_readd: bool` + reason.
- **Harness rule:** pdfcraft-fakescan attaches to the daemon, never starts, stops or removes installed
  devices, picks an installed device by capability class, applies the test's runtime state, and **resets that
  state in teardown**. A class that is not installed yet is `skip` with the exact Add instructions (UUID,
  URL, friendly name); it never auto-adds anything to Windows. Embedded tests clean up by dropping the Server.
- **Service component:** not a Windows service. A per-user **logon scheduled task** (unelevated; the registry
  already has `autostart`, and `ensure --autostart` installs it) is enough: the daemon needs no elevation,
  the registry lives in `%LOCALAPPDATA%`, and Imaging Add is per-user. Provide `fakescan daemon install |
  uninstall | status` as a thin wrapper over that. A real service (LocalSystem) can come later if a headless
  lab box needs it.

## 11. Implementation notes (registry pass; deviations from sections 8-10)

- `config.published_fingerprint` hashes the real `ScannerConfiguration` + `DefaultScanTicket` XML
  (`wsd::published_capability_xml`); `fingerprint_at_install` is replaced by
  `install.published_fingerprint_at_install`. `feeder-tray5` -> `feeder-tray50` IS a published change
  (`media_sizes` drives `ADFMaximumSize`: 8500x11700 -> 8500x14000); a capacity-only change is not.
- `installed` (HTTP, ctl, manager) is refused unless `wia_port` names exactly one distinct UUID (it may repeat, as in `urn:uuid:X/uri:X/...`) and it is the device UUID; a port naming a second UUID is foreign in reconcile too.
- Peer contact is recorded from the HTTP/SOAP path (`scanner_service`) and from WS-Discovery
  UDP Probe/Resolve (`discovery::handle_datagram` -> `DiscoveryConfig::on_peer` ->
  `DeviceCtx::probe`, same throttle). Each device's announcer decides independently, so a Resolve
  marks only the addressed device and a Types-matching Probe marks every device that answers it.
  Only reachable when the process receives Probes (`--probe-listen`); on Windows `dasHost` usually
  owns :3702, so in practice the HTTP path is what flips `new -> probed`.
- `POST /v1/profile` and ctl `profile` go through `DeviceManager::reconfigure` whenever the device
  has a manager context (every served device), so config/fingerprints and install state stay
  correct; bare `router()` use (no manager) keeps the old direct path.
- `scripts/fakescan.ps1 ensure` starts one `fakescan --manage --listen L [--advertise A] --profiles P
  [--probe-listen]` per distinct `listen` (all rows on L), attach-first via `/health`, never kills;
  `ensure -WhatIf` prints the plan. The PowerShell registry writers take a `LockFileEx` lock on
  `devices.json.lock` (FileShare ReadWrite|Delete + whole-range `FileStream.Lock`, not
  `FileShare.None`: that would make the daemon's own open of the lock file fail instead of wait).
  `scripts/test-registry.ps1` covers planning and concurrent writers.
- Still open: Rust<->PowerShell lock interop is by construction (same lock primitive) and not
  exercised by a cross-language test; `daemon install/uninstall` against real `schtasks` is only
  tested for command construction.
- Extra surface beyond the contract: `POST /v1/devices/{id}/installed|forget|reconfigure`,
  `POST /v1/reconcile`, `GET /v1/registry`, `DELETE /v1/devices/{id}?forget=true`, ctl `mark_added`.
  The row key of the default device of a `--lab-name` instance is the lab name, not `default`.
- `FAKESCAN_REGISTRY` overrides the registry path everywhere. Inside the crate's own unit tests
  `registry_path()` panics when it is unset, so a test cannot reach the real file by accident.
- Profile-change stale heals itself. `install.stale_from` (state before going stale) and
  `install.stale_at` (UTC, when it went stale) are optional, backward-compatible fields. Every path
  that applies a profile (`POST /v1/profile`, ctl `profile`, `/v1/devices/{id}/reconfigure`, and
  the daemon / `--manage` start-up reload) re-checks the row: `stale` with a reason starting
  `profile changed` and a published fingerprint equal to `published_fingerprint_at_install` goes
  back to `installed` (wia, installed_at, verified_at untouched, `needs_readd` false). That also
  covers a daemon that restarted while stale. `not listed by Windows` (or any other reason) never
  heals this way; only a reconcile that sees the device does. `POST /v1/reset` re-applies no
  profile and changes nothing. Rows from older builds without `stale_from` heal as if from
  `installed`. The tray marks a `profile changed` stale as "confirming" (no popup, amber icon)
  until `stale_at` is 20 s old (`tray_model::PROFILE_STALE_GRACE`); the registry and API report
  `stale` immediately.


## 12. Device manager UI and tray icon

Goal: add, remove and see the status of fake scanners without the command line, from a tray icon, and
give Python the same capabilities. The GUI is a thin client of the same HTTP API the CLI and Python
use; nothing in it is reachable any other way.

**Daemon side (fakescan crate)**

- `GET /ui` (and `/ui/`): a single self-contained HTML page (inline CSS + vanilla JS, no build step, no
  CDN) embedded in the binary. It uses only relative URLs (`/v1/...`), so it works at any host/port and
  for an embedded `fakescan.Server` too. **Loopback-only** (403 otherwise): it is an operator console.
- `GET /v1/profiles` -> `[{"id","connection","formats":[..],"feeder":{"capacity":null|n,"media_sizes":[..],"duplex":bool},"description":""}]`
  from the profiles directory, so the UI and Python can offer a profile picker.
- `POST /v1/ctl {"cmd":"open_settings","page":"printers"}` -> `{"opened":bool}`. Windows only, loopback
  only; opens `ms-settings:printers` in the daemon's own user session. Nothing else is launchable.
- The page shows, per device: id, friendly name, UUID, profile, install state (colour coded), `needs_readd`
  + reason, pages remaining, active faults, endpoints (copyable), last peer / last seen. Actions: add
  device (profile picker, optional id/UUID/name), remove (confirm; checkbox "forget registry row"),
  reconfigure profile (warns when it would make an installed device stale), set pages, toggle common
  faults, reset runtime state, "Add device in Windows" (calls open_settings) with the per-device Add
  instructions, and a Reconcile button (needs a WIA list: the page shows a note that reconcile is run from
  `lab reconcile` / Python because the daemon does not enumerate WIA).
- Mutations in the UI call the existing endpoints (`POST /v1/devices`, `DELETE /v1/devices/{id}`,
  `POST /v1/devices/{id}/reconfigure`, `/d/{id}/v1/pages`, `/d/{id}/v1/fault`, `/d/{id}/v1/reset`, ...).

**Tray (inside the daemon, Windows, cargo feature `tray`)**

Decision (revised): the daemon IS the tray; there is no separate tray process. One process, one logon
task, no polling, and a visible icon proves the daemon is running.

- Module `crates/fakescan/src/tray.rs`, behind cargo feature `tray` (on by default for Windows targets
  only, via `[target.'cfg(windows)'.dependencies]`), started by `fakescan --manage` when running in an
  interactive Windows session. Flags: `--no-tray` disables it. The embedded Python `Server`, tests,
  `--no-wsd`, Linux and CI never start a tray.
- The tray runs on its **own thread with its own Win32 message loop** and talks to the daemon in-process:
  it reads `DeviceManager::list()` (no HTTP) and wakes on a change notification or a ~2 s tick. A tray
  failure (init error, panic) is logged and the daemon carries on headless; the daemon never depends on it.
- Icon state: green = every device installed; amber = at least one device needs an Add or Re-Add
  (new / probed / stale / needs_readd); red = the manager reports an error. Tooltip summarises counts.
- Menu: Open device manager (opens `http://127.0.0.1:<port>/ui` in the default browser) · per-device
  status lines (disabled items, e.g. `default: installed`, `tray5: needs Add`) · Open "Add device"
  settings page (`ms-settings:printers`) · Quit fakescan (confirmation: it stops every emulated device
  and sends Bye for each; the Windows association survives a restart with the same listen + UUIDs).
- Notification: when a device transitions into needs-Add/Re-Add, raise a Windows notification whose
  click opens `ms-settings:printers` (the detached clickable helper approach of `Invoke-LabAttention`
  in scripts/Fakescan.psm1, or a native toast if it needs no heavy dependency). Fire on transitions
  only, at most once per device per 10 minutes, plus one summary at start if anything needs an Add.
- Autostart: the existing `fakescan daemon install` logon task already starts the daemon in the user's
  session, so it starts the tray with it. No second task.
- The pure parts (summary level, tooltip, menu text, transition detection) live in a `tray_model.rs`
  with no platform code so they are unit-tested on every platform.

**Python parity (fakescan-python)**

`Manager.profiles()`, `Manager.open_ui()` (default browser), `Manager.open_add_device_settings()` (daemon
`open_settings`, falling back to local `os.startfile` on Windows), `Manager.status_table()` (the same
table the UI/tray summarise), `Server.open_ui()` for embedded servers, a `fakescan.tray` module
(`start()` ensures the daemon, which carries the tray, is running: it spawns `fakescan --manage` found via
`FAKESCAN_EXE`, PATH or the known build dirs, attach-first and idempotent), and `fakescan-ctl` subcommands `ui`, `devices`, `add`, `remove`, `status`.

## 13. The daemon / tray as the Windows PnP manager (ghost clean-up)

The daemon already knows which UUIDs are ours (the registry) and what Windows should look like for each
device. It becomes the place where a ghosted or stale Windows entry is detected, explained and cleared.
This reuses the existing, lab-proven PowerShell implementations (scripts/Fakescan.psm1: remove, unpair,
bounce-fd, rebind, helper, PairTool) instead of rewriting them; Rust orchestrates and vets, PowerShell acts.

**Identity and safety (non-negotiable)**

- Ownership is decided by UUID only: a Windows object is ours iff its PnP instance id / AEP id / WIA port
  contains exactly one UUID and that UUID equals a registry device UUID. Names never decide.
- Protected, never touched under any circumstance: any UUID that is not a registry device, the real
  a real Brother MFC (a foreign uuid), and fakeprint (`...00f3`). Parsing is exact; a
  malformed or ambiguous id is refused, not guessed at.
- The daemon and tray are never elevated. Elevated actions run in a separate short-lived helper started
  with a UAC prompt (`Start-Process -Verb RunAs`) that receives an explicit, already-vetted list of
  instance ids and runs only those; the helper re-checks the allow-list itself. Never touch `dasHost`.
- Every action has a dry-run plan first, requires explicit confirmation (`confirm:true`; the UI/tray ask
  the human), is capped (refuse if the plan removes more than 8 objects), and is appended to an audit log
  `%LOCALAPPDATA%\fakescan\pnp-actions.log` (JSON lines: time, device, action, objects, outcome).

**Detect (read-only, unelevated)**: `pnp::scan()` returns, per registry device, the Windows objects bound to
its UUID and a classification:

| Kind | Evidence | Typical cause |
|------|----------|---------------|
| `ok` | installed WIA device + PnP node present | none |
| `ghost_pnp` | `SWD\DAFWSDPROVIDER\URN:UUID:<uuid>...` Image/SoftwareDevice nodes that are non-present, or present while the registry says the device is not installed | removed/changed device, profile changed after Add |
| `ghost_aep` | WinRT association endpoint IsPaired with no PnP node; Settings Add list empty | Unpair never completed |
| `sticky_offline` | WIA lists the device but Connect fails while the daemon is up | restart without re-Add |
| `none` | nothing bound | never installed (normal) |
| `foreign` | bound UUID not in the registry | not ours: report only |

Scan implementation: run the existing read-only PowerShell queries (Get-PnpDevice incl. non-present,
`Get-FakescanAssociationEndpoints`, `Get-FakescanWiaDevices`) as a child process returning JSON; no `unsafe`
FFI in the daemon. The classification and the allow-list vetting are pure Rust and unit-tested.

**Act**: each is a named step the plan can include; `rebind` is the ordered combination:
`unpair_aep` (unelevated WinRT) -> `remove_pnp` (elevated helper, `pnputil /remove-device` on vetted ids)
-> `pairtool_disassociate` (if Unpair is AccessDenied) -> `bounce_fd` (restart fdPHost; start DAS only if
stopped; elevated helper; never dasHost) -> mark the registry device `new` and clear `needs_readd` once the
scan comes back clean. The Add itself stays a human action.

**Surfaces**

- `GET /v1/pnp` (loopback only): the scan, per device. `POST /v1/pnp/clear {device|uuid, steps?, dry_run, confirm}`
  (loopback only): returns the plan; with `confirm:true` and `dry_run:false` it launches the UAC helper and
  returns immediately with a job id; `GET /v1/pnp/jobs/{id}` reports progress/outcome. ctl: `pnp_scan`, `pnp_clear`.
- Registry: `install` gains `ghost: {kind, since, detail}` and a short `pnp_history` (last 20 actions).
  A `ghost_*` classification surfaces as amber with the reason in the tray/UI/`lab status`.
- Tray: when a device is classified as ghost the tray icon turns amber, a notification offers it, and the
  menu gains `Fix <device> (clear ghost)...`: it shows the dry-run plan, asks for confirmation, then raises
  the UAC prompt (opening the helper), then re-scans. The web UI gets the same button with the plan shown.
- Python (`fakescan.Manager`): `pnp_scan()`, `pnp_clear(id, steps=None, dry_run=True, confirm=False)`,
  `wait_pnp_job(id)`; `fakescan-ctl pnp scan|clear`.
- Embedded servers and non-Windows: the routes exist but return `unsupported` (no PowerShell is run).

## 14. Serial Add queue (Windows installs one device at a time, ~30 s each)

Operational fact from the lab: Windows' Add device cannot cope with several new WSD devices appearing
together, and each Add takes about 30 s. So the daemon must **announce devices that still need an Add one at
a time**, never as a batch.

- Rule: a device announces itself on WS-Discovery (Hello burst + periodic Hello) iff it is **installed**
  (installed devices keep announcing: Windows' presence/online state needs it) **or it is the current head
  of the Add queue**. Every other non-installed device stays **silent** (no Hello; its SOAP endpoints still
  answer; it is registered but dormant). On shutdown, Bye is only sent for devices that were announcing.
- Queue = the registry devices with `install.state` in {new, probed, stale, needs_readd}, in registry
  order (stable; `POST /v1/queue` may reorder). Head = first element. `lab up` adds all the classes to the
  registry/daemon but only the head announces.
- Advance: when the head becomes `installed` (see detection below) the next device becomes head after a short
  settle delay (default 5 s) and gets its Hello burst. If the head is not installed after `head_timeout`
  (default 180 s) it rotates to the back so a stuck device cannot block the rest; the UI/tray show it as
  `skipped (timed out)`. A human can also `skip` or `advance`.
- Detection without Python: the daemon, while a head is waiting, polls a read-only PnP/WIA listing every ~5 s
  (the unelevated scan of section 13: `SWD\DAFWSDPROVIDER\URN:UUID:<uuid>...` Image nodes present for a
  registry UUID, matched by UUID only) and records `installed` itself; Python/`lab wait` and
  `POST /v1/reconcile` still work and also advance the queue.
- Surfaces: `GET /v1/queue` -> `{head, remaining:[ids], position, total, head_since, head_timeout_s}`;
  `POST /v1/queue {order:[ids]}`, `POST /v1/queue/advance`, `POST /v1/queue/skip`. `GET /v1/devices` gains
  `announcing: bool` and `queue_position`. Tray/UI show `Add now: tray5 (2 of 5)`; the Windows alert fires per
  head with the click-to-open-Settings behaviour; each head change re-opens/refreshes the alert.
- Python: `Manager.queue()`, `advance()`, `skip()`, `reorder(ids)`; `lab up` no longer announces everything
  and `lab wait` walks the queue one device at a time (prints `Add 2 of 5: tray5 now`), bounded by
  `--timeout` per device.
- Embedded servers never announce (discovery off), so they have no queue.
