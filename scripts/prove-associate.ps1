#Requires -Version 5.1
#Requires -RunAsAdministrator
<#
.SYNOPSIS
  One-shot elevated proof: non-interactive WIA install via IPNPXDeviceAssociation::Associate.

.DESCRIPTION
  Proves or disproves that IPNPXDeviceAssociation::Associate (the documented non-UI
  "Add device" equivalent) can install fakescan into WIA without a person clicking
  "Add device" in Printers & scanners.

  Sequence:
    1. Verify fakescan is healthy at $BaseUrl.
    2. DRY-RUN: show what Remove-FakescanScanner would remove (WhatIf).
    3. Capture WIA / AEP state before.
    4. Remove-FakescanScanner (Image + SoftwareDevice for f2 UUID; never Brother).
    5. Disconnect-FakescanAssociationEndpoint (clear stale IsPaired).
    6. Poll FD WSD provider until fakescan appears as Installable (up to $FdWaitSeconds).
    7. Call Invoke-FakescanPnpxAssociate (IPNPXDeviceAssociation::Associate).
    8. Poll WIA until FakescanInWia=True (up to $WiaWaitSeconds).
    9. Print verdict + all HRs.

  REQUIRES ELEVATION (UAC). Run from:
    .\scripts\fakescan.ps1 elevated        # UAC prompt fires here
    # then in the elevated shell:
    .\scripts\prove-associate.ps1
    # or with a LAN URL:
    .\scripts\prove-associate.ps1 -BaseUrl http://192.168.1.70:3920

  Sticky WIA risk after remove:
    - fakescan keeps running with same --device-uuid / --advertise / port.
    - If Associate fails, the device is temporarily uninstalled.
    - Recovery fallback (person Add):
        .\scripts\fakescan.ps1 install -BaseUrl <URL> -WaitSeconds 120
    - See docs/e2e-wia-status.md (sticky offline / re-install path).

.PARAMETER BaseUrl
  fakescan HTTP URL. Default: http://127.0.0.1:3920

.PARAMETER FdWaitSeconds
  Max seconds to poll FD WSD provider after remove for fakescan to appear as
  Installable. Default: 90.

.PARAMETER WiaWaitSeconds
  Max seconds to poll WIA after Associate call for FakescanInWia=True. Default: 30.

.PARAMETER SkipRemove
  Skip Remove-FakescanScanner (re-run Associate after a previous remove).
  Only useful if the device is already uninstalled and FD can see it.

.EXAMPLE
  # From elevated shell (.\scripts\fakescan.ps1 elevated):
  .\scripts\prove-associate.ps1
  .\scripts\prove-associate.ps1 -BaseUrl http://192.168.1.70:3920 -FdWaitSeconds 120
  .\scripts\prove-associate.ps1 -SkipRemove   # if already removed
#>
[CmdletBinding()]
param(
    [string]$BaseUrl = 'http://127.0.0.1:3920',
    [int]$FdWaitSeconds = 90,
    [int]$WiaWaitSeconds = 30,
    [switch]$SkipRemove
)

$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
Import-Module (Join-Path $here 'Fakescan.psm1') -Force

# --- Sanity: elevation check -------------------------------------------------
$elevated = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $elevated) {
    Write-Error 'prove-associate.ps1 REQUIRES ELEVATION. Run: .\scripts\fakescan.ps1 elevated'
    exit 1
}

Write-Host ''
Write-Host '=== prove-associate: IPNPXDeviceAssociation::Associate proof ==='
Write-Host ("  BaseUrl    : {0}" -f $BaseUrl)
Write-Host ("  FdWait     : {0}s" -f $FdWaitSeconds)
Write-Host ("  WiaWait    : {0}s" -f $WiaWaitSeconds)
Write-Host ("  SkipRemove : {0}" -f $SkipRemove)
Write-Host ''

# --- Step 1: fakescan health -------------------------------------------------
Write-Host '--- Step 1: fakescan health ---'
$health = $null
try {
    $health = Get-FakescanHealth -BaseUrl $BaseUrl
    Write-Host ("  /health : {0}" -f $health.StatusCode)
    if (-not $health.Ok) {
        Write-Warning ("fakescan /health not OK at {0}. Proceeding; results still logged." -f $BaseUrl)
    }
} catch {
    Write-Error ("fakescan /health failed at {0}: {1}`nIs fakescan running? Invoke-WebRequest {0}/health" -f $BaseUrl, $_)
    exit 1
}

$statusJson = $null
try {
    $statusJson = Invoke-RestMethod -Uri ($BaseUrl.TrimEnd('/') + '/v1/status') -TimeoutSec 5
    Write-Host ("  device_uuid: {0}  hellos_sent: {1}  profile: {2}" -f `
        $statusJson.device_uuid, $statusJson.wsd_hellos_sent, $statusJson.profile_id)
} catch {
    Write-Warning ("  /v1/status unavailable: {0}" -f $_)
}

# --- Step 2: DRY-RUN what would be removed -----------------------------------
Write-Host ''
Write-Host '--- Step 2: DRY-RUN remove (WhatIf) ---'
Remove-FakescanScanner -WhatIf
Write-Host '  (no changes made above)'

# --- Step 3: capture WIA / AEP state before ----------------------------------
Write-Host ''
Write-Host '--- Step 3: state before remove ---'
$wiaBefore = Get-FakescanWiaDevices
Write-Host ("  FakescanInWia={0}  FakescanPnpOnly={1}  OnlineCapable={2}  WiaConnectable={3}  StickyPresenceHint={4}" -f `
    $wiaBefore.FakescanInWia, $wiaBefore.FakescanPnpOnly, $wiaBefore.OnlineCapable, `
    $wiaBefore.WiaConnectable, $wiaBefore.StickyPresenceHint)

if (-not ($wiaBefore.FakescanInWia -or $wiaBefore.FakescanPnpOnly)) {
    if (-not $SkipRemove) {
        Write-Warning 'Fake is not installed (not in WIA, not PnP-only). Skipping remove - going straight to FD poll + Associate.'
        $SkipRemove = $true
    }
}

# --- Step 4: Remove-FakescanScanner ------------------------------------------
$removeResult = $null
$wiaClearedAfterRemove = $null
if (-not $SkipRemove) {
    Write-Host ''
    Write-Host '--- Step 4: Remove-FakescanScanner ---'
    Write-Host '  Removing Fake Image + SoftwareDevice PnP nodes (not Brother) ...'
    $removeResult = Remove-FakescanScanner
    $removeResult | Format-List Matched, Disabled, Removed, Failed | Out-Host
    if (@($removeResult.Failed).Count -gt 0 -and @($removeResult.Removed).Count -eq 0) {
        Write-Error 'Remove-FakescanScanner failed entirely (all Failed, none Removed). Cannot proceed.'
        exit 1
    }
    # Confirm WIA/PnP actually cleared. Residual presence makes later FakescanInWia
    # useless as Associate proof, and an installed SWD node blocks FD Installable.
    Write-Host '  Waiting for WIA/PnP clear (up to 30s) ...'
    $clearDeadline = [datetime]::UtcNow.AddSeconds(30)
    $wiaClearedAfterRemove = $false
    while ([datetime]::UtcNow -lt $clearDeadline) {
        try {
            $wiaMid = Get-FakescanWiaDevices
            if (-not $wiaMid.FakescanInWia -and -not $wiaMid.FakescanInPnp) {
                $wiaClearedAfterRemove = $true
                Write-Host '  WIA/PnP clear confirmed.'
                break
            }
            Write-Host ("  still present: FakescanInWia={0} FakescanInPnp={1}" -f `
                $wiaMid.FakescanInWia, $wiaMid.FakescanInPnp)
        } catch {
            Write-Warning ("  mid-remove WIA check threw: {0}" -f $_)
        }
        Start-Sleep -Seconds 3
    }
    if (-not $wiaClearedAfterRemove) {
        Write-Warning 'WIA/PnP still shows fakescan after remove. Associate cannot be proved from residual presence; FD Installable often stays empty while the SWD node remains.'
    }
} else {
    Write-Host ''
    Write-Host '--- Step 4: skipped (SkipRemove) ---'
}

# --- Step 5: Disconnect AEP (clear stale IsPaired) ---------------------------
Write-Host ''
Write-Host '--- Step 5: Disconnect-FakescanAssociationEndpoint ---'
$unpair = $null
try {
    $unpair = Disconnect-FakescanAssociationEndpoint
    $unpair | Format-List Ok, Status, Error, IsPairedAfter, Name | Out-Host
    if (-not $unpair.Ok) {
        Write-Warning '  Unpair not fully confirmed; FD may still show stale IsPaired.'
    }
} catch {
    Write-Warning ("  Disconnect-FakescanAssociationEndpoint threw: {0}" -f $_)
}

# --- Step 6: Poll FD until fakescan appears as Installable -------------------
Write-Host ''
Write-Host ('--- Step 6: Poll FD WSD provider for fakescan (up to {0}s) ---' -f $FdWaitSeconds)
$fdHit = $false
$fdDeadline = [datetime]::UtcNow.AddSeconds($FdWaitSeconds)
$fdPoll = 0
while ([datetime]::UtcNow -lt $fdDeadline) {
    $fdPoll++
    $fd = $null
    try {
        $fd = Get-FakescanFunctionDiscovery -TimeoutMs 8000
        $hits = @($fd.FakescanHits)
        if ($hits.Count -gt 0) {
            $elapsed = [int]([datetime]::UtcNow - ($fdDeadline.AddSeconds(-$FdWaitSeconds))).TotalSeconds
            Write-Host ("  poll #{0} @{1}s: fakescan found in FD ({2} hit(s)) - proceeding to Associate" -f $fdPoll, $elapsed, $hits.Count)
            $hits | Format-Table FriendlyName, XAddrs, Installable, DeviceCategory -AutoSize | Out-Host
            $fdHit = $true
            break
        } else {
            $elapsed = [int]([datetime]::UtcNow - ($fdDeadline.AddSeconds(-$FdWaitSeconds))).TotalSeconds
            Write-Host ("  poll #{0} @{1}s: FD has no fakescan hit yet (total WSD hits: {2})" -f `
                $fdPoll, $elapsed, @($fd.Hits).Count)
        }
    } catch {
        Write-Warning ("  poll #{0}: FD query threw: {1}" -f $fdPoll, $_)
    }
    Start-Sleep -Seconds 5
}
if (-not $fdHit) {
    Write-Warning ('FD did not show fakescan as Installable within {0}s. Proceeding to Associate anyway (it polls FD internally).' -f $FdWaitSeconds)
    Write-Host '  Note: fakescan must advertise on a LAN IP (not 127.0.0.1) for FD to see it.'
    Write-Host '  FD WSD cache may linger after pnputil remove; the SWD node needs to be fully gone.'
}

# --- Step 7: Invoke-FakescanPnpxAssociate ------------------------------------
Write-Host ''
Write-Host '--- Step 7: Invoke-FakescanPnpxAssociate ---'
Write-Host '  Calling IPNPXDeviceAssociation::Associate on all FD fakescan instances ...'
$assocResult = $null
try {
    $assocResult = Invoke-FakescanPnpxAssociate -QueryTimeoutMs 15000 -AssocTimeoutMs 120000
} catch {
    Write-Warning ("  Invoke-FakescanPnpxAssociate threw: {0}" -f $_)
}

# --- Step 8: Poll WIA for FakescanInWia --------------------------------------
Write-Host ''
Write-Host '--- Step 8: Poll WIA for FakescanInWia=True ---'
$wiaAfter = $null
$wiaIn = $false
if ($assocResult -and $assocResult.Ok) {
    Write-Host '  Associate reported Ok=True; polling WIA ...'
    $wiaDeadline = [datetime]::UtcNow.AddSeconds($WiaWaitSeconds)
    while ([datetime]::UtcNow -lt $wiaDeadline) {
        try {
            $wiaAfter = Get-FakescanWiaDevices
            if ($wiaAfter.FakescanInWia) {
                $wiaIn = $true
                Write-Host '  WIA: FakescanInWia=True'
                break
            }
            Write-Host '  WIA: FakescanInWia=False - waiting ...'
        } catch {
            Write-Warning ("  WIA poll threw: {0}" -f $_)
        }
        Start-Sleep -Seconds 3
    }
} else {
    Write-Host '  Associate did not report Ok=True; checking WIA once anyway ...'
    try {
        $wiaAfter = Get-FakescanWiaDevices
        $wiaIn = [bool]$wiaAfter.FakescanInWia
    } catch {
        Write-Warning ("  WIA check threw: {0}" -f $_)
    }
}

if (-not $wiaAfter) {
    try { $wiaAfter = Get-FakescanWiaDevices } catch { }
}
if ($wiaAfter) {
    Write-Host ("  WIA after: FakescanInWia={0}  OnlineCapable={1}  WiaConnectable={2}" -f `
        $wiaAfter.FakescanInWia, $wiaAfter.OnlineCapable, $wiaAfter.WiaConnectable)
}

# --- Step 9: verdict ---------------------------------------------------------
Write-Host ''
Write-Host '=== VERDICT ==='

$results = @()
if ($assocResult) { $results = @($assocResult.Results) }

if ($results.Count -gt 0) {
    Write-Host 'Associate results (per FD instance):'
    foreach ($r in $results) {
        Write-Host ("  FriendlyName   : {0}" -f $r.FriendlyName)
        Write-Host ("  QueryServiceHr : {0}" -f $r.QueryServiceHr)
        Write-Host ("  AssociateHr    : {0}" -f $r.AssociateHr)
        Write-Host ("  CallbackFired  : {0}" -f $r.CallbackFired)
        Write-Host ("  Succeeded      : {0}" -f $r.Succeeded)
        if ($r.Error) { Write-Host ("  Error          : {0}" -f $r.Error) }
        Write-Host ''
    }
} else {
    Write-Host '  0 FD instances found - Associate did not fire.'
    Write-Host '  QueryServiceHr: n/a (no instances)'
    Write-Host '  AssociateHr   : n/a (no instances)'
}

# PROVED only when Associate actually fired on an FD instance and WIA is present
# afterward. FakescanInWia alone is not enough: residual SWD/WIA after a partial
# remove would otherwise look like success when Associate never ran.
$assocFired = $results.Count -gt 0
$assocOk = [bool]($assocResult -and $assocResult.Ok)
$anySucceeded = $false
foreach ($r in $results) {
    if ($r.Succeeded) { $anySucceeded = $true; break }
}

$verdict = ''
if ($assocFired -and ($assocOk -or $anySucceeded) -and $wiaIn) {
    $verdict = 'proved'
    Write-Host 'RESULT: PROVED'
    Write-Host '  Non-interactive WIA install via IPNPXDeviceAssociation::Associate: SUCCESS.'
    Write-Host '  Associate fired on an FD instance and FakescanInWia=True afterward.'
} elseif (-not $assocFired) {
    $verdict = 'blocked-on-fd'
    Write-Host 'RESULT: BLOCKED-ON-FD'
    Write-Host '  FD returned 0 fakescan instances; Associate never fired.'
    if ($wiaIn) {
        Write-Host '  FakescanInWia=True is residual (remove did not clear WIA, or node reappeared)'
        Write-Host '  - do not treat it as Associate success.'
    }
    Write-Host '  Ensure fakescan uses --advertise <LAN-IP> and --probe-listen (ensure now'
    Write-Host '  adds --probe-listen by default), SWD nodes are gone, and FD re-discovers.'
    Write-Host '  Same-host advertise often stays FD-invisible even with Hellos+ProbeMatch;'
    Write-Host '  Brother on another host may still show Installable while fakescan does not.'
    Write-Host ("  Fallback: .\scripts\fakescan.ps1 install -BaseUrl {0} -WaitSeconds 120" -f $BaseUrl)
} elseif ($assocOk -or $anySucceeded) {
    $verdict = 'failed-wia-timeout'
    Write-Host ("RESULT: FAILED (Associate Ok but WIA not confirmed within {0}s)" -f $WiaWaitSeconds)
    Write-Host '  Check: .\scripts\fakescan.ps1 wia'
} else {
    $verdict = 'failed'
    Write-Host 'RESULT: FAILED'
    Write-Host '  Associate did not succeed. Check AssociateHr above.'
    Write-Host '  0x80070005 = E_ACCESSDENIED (not elevated?)'
    Write-Host '  0x80004005 = E_FAIL (OS rejected non-interactive Add)'
    Write-Host ("  Fallback: .\scripts\fakescan.ps1 install -BaseUrl {0} -WaitSeconds 120" -f $BaseUrl)
}

Write-Host ''
Write-Host 'Next steps:'
if ($verdict -eq 'proved') {
    Write-Host '  .\scripts\fakescan.ps1 wia-trace    # Connect test'
    Write-Host '  .\scripts\fakescan.ps1 wia          # full WIA status'
} elseif ($wiaIn) {
    Write-Host '  .\scripts\fakescan.ps1 wia-trace    # Connect test (WIA present, Associate unproved)'
    Write-Host '  .\scripts\fakescan.ps1 wia          # full WIA status'
    Write-Host ("  Re-run prove after a clean remove, or person Add: .\scripts\fakescan.ps1 install -BaseUrl {0}" -f $BaseUrl)
} else {
    Write-Host ("  .\scripts\fakescan.ps1 install -BaseUrl {0} -WaitSeconds 120  # person Add fallback" -f $BaseUrl)
    Write-Host '  .\scripts\fakescan.ps1 wia                                      # verify after Add'
}

# --- Summary object ----------------------------------------------------------
[pscustomobject]@{
    Verdict               = $verdict
    FakescanInWiaAfter    = $wiaIn
    AssocFired            = $assocFired
    AssocOk               = $assocOk
    AssocInstances        = $results.Count
    AssocResults          = $results
    UnpairOk              = if ($unpair) { $unpair.Ok } else { $null }
    RemoveMatched         = if ($removeResult) { @($removeResult.Matched).Count } else { $null }
    RemoveRemoved         = if ($removeResult) { @($removeResult.Removed).Count } else { $null }
    RemoveFailed          = if ($removeResult) { @($removeResult.Failed).Count } else { $null }
    WiaClearedAfterRemove = $wiaClearedAfterRemove
    FdHitBeforeAssoc      = $fdHit
    Elevated              = $elevated
} | Format-List
