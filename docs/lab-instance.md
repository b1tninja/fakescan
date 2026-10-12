# Lab instance control

fakescan runs as a **single stable process per port**.  Windows PnP / WIA keys
the scanner association to the advertise URL + UUID.  Restarting with different
args forces `remove` + re-Add (manual step).  This doc explains how to keep the
process alive and use the control surface instead.

## Why stable identity matters

The first time a person completes **Add device** in Settings → Printers & scanners,
Windows writes a PnP association for `<uuid> → <advertise-url>:<port>`.  If the
process restarts with a different port or UUID, the association becomes stale,
WIA returns `0x80210001` (Connect) or `0x80004005` (retrieve), and the person
must do Imaging `remove` + re-Add.

**Rule:** prefer `/v1/*` reconfigure over kill/restart. Keep `fakescan.exe`
**unelevated**; Imaging remove uses `.\scripts\fakescan.ps1 helper` (one UAC) or
an elevated shell — see [windows.md](windows.md#helper--elevated--remove--rebind).
SOAP / control / local matrix never need elevation. Add device is still a person.

## Port band

fakescan uses **even ports: 3920, 3922, 3924, …**

Odd ports (3921, 3923, …) are reserved for fakeprint.  The registry and `add`
command enforce this.  Running two fakescans on 3920 + 3922 is fine; the sibling
fakeprint on 3921 is unrelated.

## Device registry

`%LOCALAPPDATA%\fakescan\devices.json`

Example (one instance):

```json
{
  "port_policy": { "base": 3920, "step": 2 },
  "devices": [
    {
      "name": "default",
      "listen": "0.0.0.0:3920",
      "advertise": "192.168.1.70",
      "device_uuid": "00000000-0000-4000-8000-0000000000f2",
      "profile": "brother-wsd",
      "exe": "D:\\temp\\fakescan-target\\debug\\fakescan.exe",
      "autostart": true,
      "added": { "pnp": true }
    }
  ]
}
```

## Quick start (PowerShell)

```powershell
# List all registered instances
.\scripts\fakescan.ps1 list

# Add the default instance (auto-picks listen port 3920)
.\scripts\fakescan.ps1 add -Name default -Exe D:\temp\fakescan-target\debug\fakescan.exe

# Ensure the default instance is running (attach-first; never kills)
.\scripts\fakescan.ps1 ensure

# Ensure all autostart rows
.\scripts\fakescan.ps1 ensure -All

# Send a ctl command
.\scripts\fakescan.ps1 ctl pages '{"count":3}'
.\scripts\fakescan.ps1 ctl reset
.\scripts\fakescan.ps1 ctl save   # write running state to registry

# Mark as WIA-added (after person clicks Add device)
.\scripts\fakescan.ps1 mark-added

# Remove registry row (does NOT kill the process)
.\scripts\fakescan.ps1 add -Name bpl0 -Listen 0.0.0.0:3922
```

## Quick start (Python)

```bash
pip install -e "./python"        # base package

fakescan-ctl list
fakescan-ctl add --name default --exe D:\temp\fakescan-target\debug\fakescan.exe
fakescan-ctl ensure --name default
fakescan-ctl pages 3
fakescan-ctl ctl reset
fakescan-ctl save
fakescan-ctl mark-added --name default
```

## GET /v1/ctl — capabilities + identity

```bash
curl http://127.0.0.1:3920/v1/ctl
```

```json
{
  "cmds": ["status","health","profile","pattern","pages","feeder",
           "jpeg_com","effects","scenario","fault","slow",
           "loopback","reset","save"],
  "port": 3920,
  "uuid": "00000000-0000-4000-8000-0000000000f2",
  "base_url": "http://127.0.0.1:3920",
  "name": "default"
}
```

## POST /v1/ctl — unified command

All existing `/v1/*` endpoints remain for backward compat.  `/v1/ctl` is a
unified front-end over the same handlers.

```bash
curl -X POST http://127.0.0.1:3920/v1/ctl \
  -H "content-type: application/json" \
  -d '{"cmd":"pages","args":{"count":3}}'

curl -X POST http://127.0.0.1:3920/v1/ctl \
  -d '{"cmd":"reset"}'

curl -X POST http://127.0.0.1:3920/v1/ctl \
  -d '{"cmd":"save"}'
```

### `save` command

Writes the running process's current state (listen, uuid, profile) back to its
devices.json row.  Requires `--lab-name` (or `FAKESCAN_NAME`) to be set when
the process started.

```bash
# Start with name
fakescan --lab-name default --advertise 192.168.1.70
# After changing profile:
curl -X POST http://127.0.0.1:3920/v1/ctl -d '{"cmd":"profile","args":{"id":"brother-usb"}}'
curl -X POST http://127.0.0.1:3920/v1/ctl -d '{"cmd":"save"}'
```

## Windows named pipe (Windows only)

`\\.\pipe\fakescan-<name>` (default `\\.\pipe\fakescan-default`)

JSONL wire protocol — same schema as `/v1/ctl`:

```text
→ {"cmd":"pages","args":{"count":3}}\n
← {"ok":true,"result":{...status...}}\n
```

Use `fakescan-ctl pipe pages --args '{"count":3}'` or connect with any JSONL
tool.  Non-Windows: use HTTP only.

## MCP (attach-only)

```bash
pip install -e "./python[mcp]"   # installs mcp package
fakescan-mcp --base-url http://127.0.0.1:3920
```

Cursor / Claude MCP config entry (stdio):

```json
{
  "fakescan": {
    "command": "fakescan-mcp",
    "args": ["--base-url", "http://192.168.1.70:3920"]
  }
}
```

The MCP server **never starts or stops fakescan**.  It fails fast (error tool
result) if the device is not reachable.  Available tools: `ctl_info`, `status`,
`reset`, `pages`, `pattern`, `scenario`, `fault`, `ctl`.

## Running multiple instances

fakescan supports parallel named instances **within the same role**:

| Name | Port | UUID (last 4) |
|------|------|---------------|
| `default` | 3920 | `…00f2` |
| `bpl0`    | 3922 | `…00f4` |

Each needs a distinct port (even band) and UUID.  Each requires one person
Add-device pass.

```powershell
.\scripts\fakescan.ps1 add -Name bpl0 -Listen 0.0.0.0:3922 `
    -DeviceUuid 00000000-0000-4000-8000-0000000000f4 `
    -Exe D:\temp\fakescan-target\debug\fakescan.exe
.\scripts\fakescan.ps1 ensure -Name bpl0
```

## Reboot persistence

1. Add the exe path to the registry row (`add -Exe PATH`).
2. Set `autostart = true` in the row (`add -Autostart`).
3. Install a Windows logon task:

```powershell
$a = New-ScheduledTaskAction -Execute fakescan.exe `
     -Argument "--lab-name default --listen 0.0.0.0:3920 --advertise 192.168.1.70"
$t = New-ScheduledTaskTrigger -AtLogOn
Register-ScheduledTask -TaskName FakescanLab -Action $a -Trigger $t -RunLevel Highest
```

Or use `ensure` in a startup script; it does nothing if the process is already
running.

## Install-attention UX

When interactive Add device is still required (`added.pnp = false` and no
silent install has succeeded), `Open-FakescanAddDevice` fires a multi-channel
alert via the private `Invoke-LabAttention` helper.

### Default (no flag)

1. **Yellow console banner** — always printed; survives output redirection.
2. **`[Console]::Beep(1000, 300)`** — 1 kHz tone for 300 ms.
3. **BEL (`[char]0x7`)** — emitted alongside the beep.
4. **WinForms `NotifyIcon.ShowBalloonTip`** — system-tray balloon, held 3 s then
   disposed.

### Flags

| Flag | Effect |
|------|--------|
| *(default)* | Banner + beep + BEL + balloon |
| `-Loud` | All of the above **plus** `FlashWindowEx` on the first parent-process window handle found by walking the process tree. |
| `-Quiet` | **Banner only** — beep and balloon suppressed. |

### Where it fires

- **`ensure`** — only after quieter paths fail. Order when `added.pnp` is false:
  1. Already in WIA → `mark-added` (no UI)
  2. Elevated → `fakescan.ps1 associate` (`IPNPXDeviceAssociation`); mark-added on success
  3. Else attention + Settings (`Open-FakescanAddDevice`)
- **`install`** — when no silent path (PairAsync) has produced `InstalledAfter = true`.
- **`rebind`** — after the remove + AEP-unpair sequence (and fdPHost/DAS bounce on Unpair AccessDenied), before opening Settings.
- **`add-device`** — direct command.

Older binaries without `GET /v1/ctl` still attach via `/health` + `/v1/status`.

### Examples

```powershell
# Default attention (banner + beep + balloon)
.\scripts\fakescan.ps1 ensure

# Beep-free for shared offices
.\scripts\fakescan.ps1 install -BaseUrl http://192.168.1.70:3920 -Quiet

# Flash terminal window as well (good for background automation)
.\scripts\fakescan.ps1 rebind -BaseUrl http://192.168.1.70:3920 -Loud

# Direct attention signal without running install flow
.\scripts\fakescan.ps1 add-device -Loud
```

## Siblings: fakeprint

fakeprint runs independently on odd ports.  See [siblings.md](siblings.md) for
the co-existence rules.

**Never kill fakeprint processes** from fakescan scripts.  The two registries,
pipes, MCP servers, and port bands are completely separate.
