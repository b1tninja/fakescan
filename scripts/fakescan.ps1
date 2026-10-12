#Requires -Version 5.1
<#
.SYNOPSIS
  CLI for fakescan detection, SOAP checks, control plane, and Windows install assist.

.EXAMPLE
  .\scripts\fakescan.ps1 status
  .\scripts\fakescan.ps1 soap -BaseUrl http://192.168.1.10:3920
  .\scripts\fakescan.ps1 discover
  .\scripts\fakescan.ps1 discoverable
  .\scripts\fakescan.ps1 pages 3
  .\scripts\fakescan.ps1 wia
  .\scripts\fakescan.ps1 wia-trace
  .\scripts\fakescan.ps1 install -BaseUrl http://192.168.1.10:3920 -ProbeDiscovery -WaitSeconds 120
  .\scripts\fakescan.ps1 remove
  .\scripts\fakescan.ps1 rebind -BaseUrl http://192.168.1.10:3920 -WaitSeconds 120
  .\scripts\fakescan.ps1 e2e -BaseUrl http://192.168.1.10:3920 -WaitSeconds 120
#>
[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [ValidateSet(
        'status', 'health', 'jobs', 'activity', 'log', 'soap', 'discover', 'discoverable', 'wia', 'wia-trace', 'fd', 'aep', 'pair', 'swd',
        'install', 'remove', 'rebind', 'unpair', 'bounce-fd', 'elevated', 'helper', 'e2e', 'add-device',
        'pages', 'feeder', 'scenarios', 'scenario', 'profile', 'reset', 'stop',
        'fault', 'lab',
        # Lab instance control
        'ensure', 'ctl', 'list', 'add', 'save', 'mark-added',
        'help'
    )]
    [string]$Command = 'help',

    [string]$BaseUrl = 'http://127.0.0.1:3920',

    [Parameter(Position = 1)]
    [string]$Arg1,

    [int]$WaitSeconds = 0,

    [switch]$ProbeDiscovery,

    [switch]$SkipUi,

    # rebind: bounce fdPHost + DeviceAssociationService even if Unpair succeeded
    [switch]$RestartDiscovery,

    [string]$PdfcraftCli,

    [switch]$Remove,

    # Attention level: -Loud adds FlashWindowEx; -Quiet suppresses beep/balloon (banner stays).
    # Default (neither flag): banner + beep + balloon.
    [switch]$Loud,
    [switch]$Quiet,

    # Lab instance name for ensure / ctl / add / save / mark-added.
    # Env: FAKESCAN_NAME.
    [string]$Name = $(if ($env:FAKESCAN_NAME) { $env:FAKESCAN_NAME } else { 'default' }),

    # add: listen address override (e.g. 0.0.0.0:3922)
    [string]$Listen,

    # add: advertise IP override
    [string]$Advertise,

    # add: device UUID override
    [string]$DeviceUuid,

    # add: profile id override
    [string]$Profile,

    # add: exe path override
    [string]$Exe,

    # add: mark as autostart
    [switch]$Autostart,

    # ensure: start all autostart rows
    [switch]$All,

    # Extra args for `lab` (passed to `python -m fakescan.lab`).
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$Rest
)

$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
Import-Module (Join-Path $here 'Fakescan.psm1') -Force

function Show-Help {
    @'
fakescan.ps1 - manage / detect a fakescan instance

Attention flags (for install / ensure / rebind / add-device):
  -Loud    FlashWindowEx on parent terminal window (opt-in; adds to default alert).
  -Quiet   Banner only — suppresses beep and balloon (banner always shown).
  Default  Banner + [Console]::Beep(1000,300) + BEL char + NotifyIcon balloon.

Lab instance control (new — see docs/lab-instance.md):
  list                               List %LOCALAPPDATA%\fakescan\devices.json
  add  [-Name NAME] [-Listen H:PORT] [-Advertise IP] [-DeviceUuid UUID]
       [-Profile ID] [-Exe PATH] [-Autostart]
                                     Add or update a registry row (even ports only)
  ensure [-Name default] [-All]      Check if running; start from registry if not
                                     Attach-first: never kills a running instance
  ctl  <cmd> [argsJson]              POST /v1/ctl {cmd, args}  e.g. ctl pages '{"count":3}'
  save [-Name NAME]                  POST /v1/ctl save (write running state to registry)
  mark-added [-Name default]         Mark registry row added.pnp=true

Control plane:
  status [-BaseUrl URL]              GET /v1/status
  jobs [-BaseUrl URL]                GET /v1/jobs (Pending / Processing / Completed)
  activity [-BaseUrl URL]            GET /v1/activity (recent WSD, job, and control calls)
  log [-BaseUrl URL]                 GET /v1/log (the same activity as plain text)
  health [-BaseUrl URL]              GET /health
  soap [-BaseUrl URL]                GetScannerElements smoke test
  discover                           WS-Discovery Probe (UDP 3702)
  discoverable [-BaseUrl URL]        SOAP + UDP + FD + WinRT AEP + installed? (verdict)
  wia                                List WIA + PnP imaging devices
  fd                                 Function Discovery WSD instances (fakescan vs others)
  aep                                WinRT WSD AssociationEndpoints (paired / CanPair)
  pair                               Experimental PairAsync on fakescan AEP (often Failed)
  swd                                SWD\DAFWSDPROVIDER / WSD PnP nodes (installed WSD)
  pages <n> [-BaseUrl URL]           POST /v1/pages
  feeder <true|false> [-BaseUrl URL] POST /v1/feeder
  fault <json> [-BaseUrl URL]        POST /v1/fault  e.g. fault {"jam_after_n":2}
  lab <fakescan-lab args…>           Python harness (docs/lab-harness.md); needs pip install -e ./python
  scenarios [-BaseUrl URL]           GET /v1/scenarios (tester progression)
  scenario <order|qr|tone|…>         POST /v1/scenario (see docs/patterns.md)
  profile <brother-wsd|brother-usb>  POST /v1/profile
  reset [-BaseUrl URL]               POST /v1/reset
  stop [-BaseUrl URL]                Ask this process to exit (discovery Bye, then the port)
  install [-BaseUrl URL] [-ProbeDiscovery] [-WaitSeconds N] [-SkipUi]
                                     Verify SOAP/FD/AEP, open Printers & scanners, optional wait
                                     (no silent install; person must click Add device)
  remove                             pnputil-remove fakescan Image + SoftwareDevice PnP nodes
                                     (needs elevation in-process, or: helper remove; never Brother)
  rebind [-BaseUrl URL] [-WaitSeconds N] [-SkipUi] [-RestartDiscovery]
                                     Sticky/offline recovery: remove → Unpair AEP → Printers & scanners → wait WIA
                                     (0x80210005 with PnP OK / no SOAP; not fixed by stisvc alone).
                                     Ghost AEP Unpair AccessDenied auto-bounces fdPHost then DAS; -RestartDiscovery forces that.
  unpair                             WinRT UnpairAsync on fakescan AEP (clears stale IsPaired so Add lists Fake)
  bounce-fd                          Restart fdPHost then DeviceAssociationService (never dasHost). Elevation or helper.
  elevated                           Interactive UAC PowerShell with Fakescan.psm1 (remove / rebind / associate)
                                     Do not run fakescan.exe elevated; Add device is still a person
  helper                             One UAC, then session helper for Fake remove/unpair/bounce-fd (suite stays unelevated)
  helper status                      Whether that helper is running (session field: process_id)
  helper whatif                      Preview the Fake nodes the helper would remove
  helper remove                      Ask the helper to remove Fake nodes (does not elevate this shell)
  helper unpair                      Ask the helper to UnpairAsync stale fakescan AEP (AccessDenied unelevated)
  helper bounce-fd                   Ask the helper to restart fdPHost then DeviceAssociationService
  helper stop                        Ask the helper to exit
  e2e [-BaseUrl URL] [-WaitSeconds N] [-SkipUi] [-ProbeDiscovery] [-PdfcraftCli PATH] [-Remove]
                                     health -> discoverable -> install -> WIA assert -> optional pdfcraft-cli scan

Examples:
  .\scripts\fakescan.ps1 status
  .\scripts\fakescan.ps1 soap -BaseUrl http://192.168.1.10:3920
  .\scripts\fakescan.ps1 discoverable -BaseUrl http://192.168.1.70:3920
  .\scripts\fakescan.ps1 fd
  .\scripts\fakescan.ps1 aep
  .\scripts\fakescan.ps1 swd
  .\scripts\fakescan.ps1 install -BaseUrl http://192.168.1.70:3920 -ProbeDiscovery -WaitSeconds 120
  .\scripts\fakescan.ps1 helper
  .\scripts\fakescan.ps1 helper remove
  .\scripts\fakescan.ps1 elevated
  .\scripts\fakescan.ps1 remove
  .\scripts\fakescan.ps1 rebind -BaseUrl http://192.168.1.70:3920 -WaitSeconds 120
  .\scripts\fakescan.ps1 e2e -BaseUrl http://192.168.1.70:3920 -WaitSeconds 120 -Remove
  .\scripts\fakescan.ps1 fault "{""jam_after_n"":2}"
  .\scripts\fakescan.ps1 lab loopback-soap page0.jpg page1.jpg
'@ | Write-Host
}

switch ($Command) {
    'help' { Show-Help }

    # ── Lab instance control ──────────────────────────────────────────────────

    'list' {
        $reg = Get-FakescanRegistry
        Write-Host ("Registry: {0}" -f $script:RegistryPath)
        $devs = @($reg.devices)
        if ($devs.Count -eq 0) {
            Write-Host '  (no devices)'
        } else {
            $devs | Format-Table name, listen, device_uuid, profile, autostart, added -AutoSize
        }
    }

    'add' {
        $addParams = @{ Name = $Name }
        if (-not [string]::IsNullOrWhiteSpace($Listen))     { $addParams.Listen     = $Listen }
        if (-not [string]::IsNullOrWhiteSpace($Advertise))  { $addParams.Advertise  = $Advertise }
        if (-not [string]::IsNullOrWhiteSpace($DeviceUuid)) { $addParams.DeviceUuid = $DeviceUuid }
        if (-not [string]::IsNullOrWhiteSpace($Profile))    { $addParams.Profile    = $Profile }
        if (-not [string]::IsNullOrWhiteSpace($Exe))        { $addParams.Exe        = $Exe }
        if ($Autostart) { $addParams.Autostart = $true }
        Add-FakescanDevice @addParams | Format-List
    }

    'ensure' {
        if ($All) {
            Start-FakescanEnsure -All -Loud:$Loud -Quiet:$Quiet
        } else {
            Start-FakescanEnsure -Name $Name -Loud:$Loud -Quiet:$Quiet
        }
    }

    'ctl' {
        # fakescan.ps1 ctl <cmd> [argsJson]
        $ctlCmd = $Arg1
        if ([string]::IsNullOrWhiteSpace($ctlCmd)) {
            # No cmd: show GET /v1/ctl info.
            Get-FakescanCtlInfo -BaseUrl $BaseUrl | ConvertTo-Json -Depth 6
        } else {
            $ctlArgs = @{}
            if ($Rest -and $Rest.Count -gt 0) {
                $jsonStr = $Rest[0]
                try { $ctlArgs = $jsonStr | ConvertFrom-Json -AsHashtable } catch {
                    throw "ctl args must be a JSON object, e.g. '{""count"":3}'"
                }
            }
            Invoke-FakescanCtl -Cmd $ctlCmd -Args $ctlArgs -BaseUrl $BaseUrl | ConvertTo-Json -Depth 6
        }
    }

    'save' {
        Invoke-FakescanCtl -Cmd save -BaseUrl $BaseUrl | ConvertTo-Json -Depth 6
    }

    'mark-added' {
        Set-FakescanDeviceAdded -Name $Name | Format-List
    }

    # ── existing commands (unchanged) ─────────────────────────────────────────

    'health' {
        Get-FakescanHealth -BaseUrl $BaseUrl | Format-List
    }
    'status' {
        Get-FakescanStatus -BaseUrl $BaseUrl | ConvertTo-Json -Depth 6
    }
    'jobs' {
        Invoke-RestMethod -Uri ($BaseUrl.TrimEnd('/') + '/v1/jobs') -TimeoutSec 5 |
            Select-Object -ExpandProperty jobs |
            Format-Table id, state, format, source, dpi, color, page_index, pages_left -AutoSize
    }
    'activity' {
        $view = Invoke-RestMethod -Uri ($BaseUrl.TrimEnd('/') + '/v1/activity') -TimeoutSec 5
        Write-Host "seq=$($view.seq)"
        @($view.events) | ForEach-Object {
            Write-Host ("{0} {1} {2} {3} {4}ms {5}" -f $_.seq, $_.kind, $_.action, $_.outcome, $_.ms, $_.detail)
        }
    }
    'log' {
        (Invoke-WebRequest -Uri ($BaseUrl.TrimEnd('/') + '/v1/log') -UseBasicParsing -TimeoutSec 5).Content
    }
    'soap' {
        Test-FakescanSoap -BaseUrl $BaseUrl | Format-List
        $el = Get-FakescanElements -BaseUrl $BaseUrl -Name ScannerDescription, ScannerConfiguration
        if ($el.Content -match 'FormatsSupported[\s\S]{0,800}') {
            Write-Host "`n--- formats / dpi excerpt ---"
            Write-Host $Matches[0]
        }
    }
    'discover' {
        $hits = Find-FakescanDiscovery -TimeoutMs 4000
        if (-not $hits -or $hits.Count -eq 0) {
            Write-Warning 'No ProbeMatch. See docs/windows.md (multicast / Docker Desktop limits).'
        } else {
            $hits | Format-Table -AutoSize
        }
    }
    'discoverable' {
        $r = Test-FakescanDiscoverable -BaseUrl $BaseUrl
        $r | Format-List BaseUrl, SoapReachable, ScannerName, UdpProbeMatch, `
            FunctionDiscoveryCom, FunctionDiscoveryHit, FunctionDiscoveryInstallable, `
            AssociationEndpointHit, AssociationEndpointCanPair, AssociationEndpointPaired, `
            AlreadyInstalled, FakescanInWia, FakescanPnpOnly, OnlineCapable, `
            WiaConnectable, StickyPresenceHint, GhostAep, StuckKind, Verdict
        if ($r.UdpMatches) {
            Write-Host 'UDP ProbeMatch / Hello:'
            $r.UdpMatches | Format-Table -AutoSize
        }
        if ($r.FunctionDiscoveryHits) {
            Write-Host 'Function Discovery fakescan hits:'
            $r.FunctionDiscoveryHits | Format-Table FriendlyName, XAddrs, Installable, DeviceCategory -AutoSize
        }
        if ($r.AssociationEndpointHits) {
            Write-Host 'WinRT AEP fakescan hits:'
            $r.AssociationEndpointHits | Format-Table Name, Id, IsPaired, CanPair -AutoSize
        }
        Write-Host 'Notes:'
        $r.Notes | ForEach-Object { Write-Host "  $_" }
    }
    'wia' {
        $d = Get-FakescanWiaDevices
        Write-Host 'WIA devices:'
        $d.Wia | Format-Table -AutoSize
        Write-Host 'PnP Image class:'
        $d.Pnp | Format-Table -AutoSize
        Write-Host ("FakescanPresent={0} FakescanInWia={1} FakescanPnpOnly={2} OnlineCapable={3} WiaConnectable={4} StickyPresenceHint={5}" -f `
            $d.FakescanPresent, $d.FakescanInWia, $d.FakescanPnpOnly, $d.OnlineCapable, $d.WiaConnectable, $d.StickyPresenceHint)
        if ($d.StickyPresenceHint -or $d.FakescanPnpOnly) {
            Write-Host 'Hint: sticky/offline PnP — .\scripts\fakescan.ps1 helper remove  (or rebind: remove + Printers & scanners)'
        }
        Write-Host 'Connect itself: .\scripts\fakescan.ps1 wia-trace'
    }
    'wia-trace' {
        Trace-FakescanWiaConnect -BaseUrl $BaseUrl
    }
    'fd' {
        $fd = Get-FakescanFunctionDiscovery -TimeoutMs 8000
        Write-Host 'Function Discovery queries:'
        $fd.Queries | Format-Table -AutoSize
        Write-Host 'All WSD / network FD hits:'
        $fd.Hits | Format-Table FriendlyName, ModelName, XAddrs, Installable, DeviceCategory, LooksLikeFakescan -AutoSize
        Write-Host ("FakescanPresent={0} InstallableFakescan={1}" -f $fd.FakescanPresent, $fd.InstallableFakescan)
        if ($fd.FakescanHits) {
            Write-Host 'Fakescan-like only:'
            $fd.FakescanHits | Format-List
        }
    }
    'aep' {
        $aep = Get-FakescanAssociationEndpoints
        if (-not $aep.Ok) {
            Write-Warning $aep.Error
        }
        Write-Host 'WinRT WSD AssociationEndpoints:'
        $aep.Hits | Format-Table Name, IsPaired, CanPair, LooksLikeFakescan, Id -AutoSize
        Write-Host ("FakescanPresent={0} CanPairFakescan={1}" -f $aep.FakescanPresent, $aep.CanPairFakescan)
    }
    'pair' {
        $r = Connect-FakescanAssociationEndpoint
        $r | Format-List
        if (-not $r.Ok) {
            Write-Warning 'PairAsync did not install. Use: .\scripts\fakescan.ps1 install (Add device UI).'
        }
    }
    'associate' {
        # IPNPXDeviceAssociation::Associate — programmatic Add device (REQUIRES ELEVATION).
        # Queries FD WSD provider, finds fakescan, calls Associate on each instance found.
        # Run from: .\scripts\fakescan.ps1 elevated  then  .\scripts\fakescan.ps1 associate
        $r = Invoke-FakescanPnpxAssociate
        $r | Format-List
        if ($r.FakescanInWiaAfter) {
            Write-Host 'Fakescan is now in WIA.'
        } elseif ($r.Ok) {
            Write-Host 'Associate call succeeded; WIA may take a moment to update. Try: fakescan.ps1 wia'
        } else {
            Write-Warning 'Associate did not succeed. Elevated? FD sees fakescan? Try: fakescan.ps1 discoverable'
        }
    }
    'swd' {
        $all = Get-FakescanSwdDevices
        Write-Host 'SWD / WSD PnP nodes (installed associations):'
        $all | Format-Table Status, Class, Name, LooksLikeFakescan, InstanceId -AutoSize
        $fake = @($all | Where-Object { $_.LooksLikeFakescan })
        Write-Host ("Fakescan-like SWD count={0} (Brother rows are expected when a real MFP is installed)" -f $fake.Count)
    }
    'add-device' {
        Open-FakescanAddDevice -Loud:$Loud -Quiet:$Quiet
    }

    'elevated' {
        Start-FakescanElevatedShell
    }
    'helper' {
        $idle = if ($WaitSeconds -gt 0) { $WaitSeconds } else { 1800 }
        switch ($Arg1) {
            '' { Start-FakescanElevatedHelper -IdleSeconds $idle | Format-List process_id, elevated, started, idle_seconds }
            'status' {
                $session = Get-FakescanElevatedHelperSession
                if (-not $session) { Write-Host 'elevated helper is not running'; break }
                $session | Format-List process_id, elevated, started, idle_seconds
            }
            'whatif' { Invoke-FakescanElevatedHelper -Cmd whatif | ConvertTo-Json -Depth 6 }
            'remove' { Invoke-FakescanElevatedHelper -Cmd remove | ConvertTo-Json -Depth 6 }
            'unpair' { Invoke-FakescanElevatedHelper -Cmd unpair | ConvertTo-Json -Depth 6 }
            'bounce-fd' { Invoke-FakescanElevatedHelper -Cmd 'bounce-fd' -TimeoutSeconds 90 | ConvertTo-Json -Depth 6 }
            'stop' { Invoke-FakescanElevatedHelper -Cmd stop | ConvertTo-Json -Depth 4 }
            default { throw 'usage: helper [status|whatif|remove|unpair|bounce-fd|stop]' }
        }
    }
    'pages' {
        if ($null -eq $Arg1 -or $Arg1 -eq '') { throw 'usage: pages <count>' }
        Set-FakescanPages -Count ([int]$Arg1) -BaseUrl $BaseUrl | ConvertTo-Json -Compress
    }
    'feeder' {
        if ($Arg1 -notin @('true', 'false', '1', '0')) { throw 'usage: feeder <true|false>' }
        $ready = $Arg1 -in @('true', '1')
        Set-FakescanFeeder -Ready:$ready -BaseUrl $BaseUrl | ConvertTo-Json -Compress
    }
    'fault' {
        if ([string]::IsNullOrWhiteSpace($Arg1)) {
            throw 'usage: fault <json>  e.g. fault {"jam_after_n":2,"empty_mid_job":0}'
        }
        $BaseUrl = (Resolve-FakescanBaseUrl $BaseUrl)
        Invoke-RestMethod -Method Post -Uri "$BaseUrl/v1/fault" `
            -ContentType 'application/json' `
            -Body $Arg1 `
            -TimeoutSec 5 | ConvertTo-Json -Depth 6
    }
    'lab' {
        $pyArgs = @()
        if (-not [string]::IsNullOrWhiteSpace($Arg1)) { $pyArgs += $Arg1 }
        if ($Rest) { $pyArgs += $Rest }
        if ($pyArgs.Count -eq 0) {
            throw 'usage: lab <fakescan-lab subcommand…>  e.g. lab status   or   lab loopback-soap python/fixtures/a.jpg'
        }
        $repo = Split-Path -Parent $here
        $env:PYTHONPATH = (Join-Path $repo 'python')
        $probe = & python -c "import fakescan, httpx; print('ok')" 2>&1
        if ($LASTEXITCODE -ne 0) {
            throw @"
fakescan Python package not importable (need httpx).
  cd $repo
  pip install -e `"./python[dev]`"
Probe: $probe
"@
        }
        & python -m fakescan.lab --base-url $BaseUrl @pyArgs
        if ($LASTEXITCODE -ne 0) {
            throw "fakescan-lab exited with code $LASTEXITCODE"
        }
    }
    'scenarios' {
        Get-FakescanScenarios -BaseUrl $BaseUrl | ConvertTo-Json -Depth 6
    }
    'scenario' {
        if (-not $Arg1) { throw 'usage: scenario <order|qr|tone|color|orient|crop|duplex>' }
        Set-FakescanScenario -Id $Arg1 -BaseUrl $BaseUrl | ConvertTo-Json -Depth 6
    }
    'profile' {
        if ($Arg1 -notin @('brother-wsd', 'brother-usb')) {
            throw 'usage: profile <brother-wsd|brother-usb>'
        }
        Set-FakescanProfile -Id $Arg1 -BaseUrl $BaseUrl | ConvertTo-Json -Compress
    }
    'reset' {
        Reset-Fakescan -BaseUrl $BaseUrl | ConvertTo-Json -Compress
    }
    'stop' {
        # yes is required so a stray ctl post cannot take the scanner offline.
        Invoke-FakescanCtl -Cmd shutdown -Args @{ yes = $true } -BaseUrl $BaseUrl | ConvertTo-Json -Compress
    }
    'install' {
        $r = Install-FakescanScanner -BaseUrl $BaseUrl -WaitSeconds $WaitSeconds `
            -ProbeDiscovery:$ProbeDiscovery -SkipUi:$SkipUi -Loud:$Loud -Quiet:$Quiet
        $r | Format-List BaseUrl, HealthOk, SoapOk, ScannerName, Installed
        Write-Host "`nGuidance:"
        $r.Guidance | ForEach-Object { Write-Host "  $_" }
        if ($r.DiscoveryMatches) {
            Write-Host "`nDiscovery:"
            $r.DiscoveryMatches | Format-Table -AutoSize
        }
    }
    'remove' {
        Remove-FakescanScanner | Format-List
    }
    'rebind' {
        $wait = if ($WaitSeconds -gt 0) { $WaitSeconds } else { 120 }
        $r = Repair-FakescanScanner -BaseUrl $BaseUrl -WaitSeconds $wait -SkipUi:$SkipUi `
            -RestartDiscovery:$RestartDiscovery -Loud:$Loud -Quiet:$Quiet
        $r | Format-List BaseUrl, HealthOk, Installed
        Write-Host "`nGuidance:"
        $r.Guidance | ForEach-Object { Write-Host "  $_" }
        if ($r.Remove) {
            Write-Host "`nRemove:"
            $r.Remove | Format-List Matched, Disabled, Removed, Failed
        }
        if ($r.DiscoveryBounce) {
            Write-Host "`nDiscovery bounce:"
            $r.DiscoveryBounce | Format-List Ok, Restarted, RestoredDependents, NeverTouched, Error
        }
    }
    'unpair' {
        Disconnect-FakescanAssociationEndpoint | Format-List
        Write-Host 'Then refresh Printers & scanners — Fake should appear under Add when IsPaired=False CanPair=True.'
        Write-Host 'Check: .\scripts\fakescan.ps1 aep'
        Write-Host 'If Unpair AccessDenied: .\scripts\fakescan.ps1 bounce-fd  then unpair again (never dasHost).'
    }
    'bounce-fd' {
        $r = Invoke-FakescanDiscoveryRestart
        $r | Format-List Ok, Restarted, RestoredDependents, NeverTouched, Error
        Write-Host "`nGuidance:"
        $r.Guidance | ForEach-Object { Write-Host "  $_" }
        Write-Host 'Then: .\scripts\fakescan.ps1 aep   (want IsPaired=False CanPair=True)'
    }
    'e2e' {
        $e2e = Join-Path $here 'e2e-wia.ps1'
        if (-not (Test-Path -LiteralPath $e2e)) {
            throw "missing $e2e"
        }
        # CLI default WaitSeconds is 0 (for install); e2e wants a real wait unless overridden.
        $wait = if ($WaitSeconds -gt 0) { $WaitSeconds } else { 120 }
        $e2eArgs = @{
            BaseUrl = $BaseUrl
            WaitSeconds = $wait
            SkipUi = $SkipUi
            ProbeDiscovery = $ProbeDiscovery
            Remove = $Remove
        }
        if (-not [string]::IsNullOrWhiteSpace($PdfcraftCli)) {
            $e2eArgs['PdfcraftCli'] = $PdfcraftCli
        }
        & $e2e @e2eArgs
    }
}
