#Requires -Version 5.1
<#
.SYNOPSIS
  Tiny self-check for elevated-helper IPC assumptions (no UAC, no PnP).

.DESCRIPTION
  Verifies: empty List @() pitfall vs ToArray; process_id session field;
  Move-Item refuses overwrite; Watch clears stale request on start (only if
  no helper session is live). Does not elevate and does not touch :3920.
#>
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
Import-Module (Join-Path $here 'Fakescan.psm1') -Force

$failed = 0
function Assert-True {
    param([bool]$Cond, [string]$Msg)
    if ($Cond) {
        Write-Host "ok  $Msg"
    } else {
        Write-Host "FAIL $Msg"
        $script:failed++
    }
}

# 1) Windows PowerShell List[object] / @() pitfall (Remove-FakescanScanner Failed list)
$list = New-Object 'System.Collections.Generic.List[object]'
$wrapOk = $true
try { $null = @($list) } catch { $wrapOk = $false }
Assert-True (-not $wrapOk) 'empty List[object] wrapped with @() throws (why ToArray is required)'
$arrOk = $true
try { $null = @($list.ToArray()) } catch { $arrOk = $false }
Assert-True $arrOk 'empty List[object].ToArray() wrapped with @() succeeds'

# 2) Session shape uses process_id (not pid)
$sessionObj = @{ process_id = 12345; elevated = $true } | ConvertTo-Json | ConvertFrom-Json
Assert-True ($null -ne $sessionObj.process_id) 'session JSON round-trips process_id'
Assert-True ($sessionObj.PSObject.Properties.Name -notcontains 'pid') 'session sample has no pid property'

# 3) Move-Item without -Force does not overwrite (publish race stays loud)
$probe = Join-Path $env:TEMP ('fakescan-elev-helper-check-' + [guid]::NewGuid().ToString('n'))
New-Item -ItemType Directory -Force -Path $probe | Out-Null
try {
    'a' | Set-Content -LiteralPath (Join-Path $probe 'request.json') -Encoding utf8
    'b' | Set-Content -LiteralPath (Join-Path $probe 'request.tmp.json') -Encoding utf8
    $moveFailed = $false
    try {
        Move-Item -LiteralPath (Join-Path $probe 'request.tmp.json') `
            -Destination (Join-Path $probe 'request.json') -ErrorAction Stop
    } catch {
        $moveFailed = $true
    }
    Assert-True $moveFailed 'Move-Item to existing request.json fails without -Force'
} finally {
    Remove-Item -LiteralPath $probe -Recurse -Force -ErrorAction SilentlyContinue
}

# 4) Request payload contract: id + cmd only (no instance ids from JSON)
$payload = @{ id = 'abc'; cmd = 'whatif' } | ConvertTo-Json | ConvertFrom-Json
Assert-True ($payload.PSObject.Properties.Name -notcontains 'instance_id') 'request sample has no instance_id'

# 5) Stale clear on Watch start — skip if a live helper already owns the IPC dir
$live = Get-FakescanElevatedHelperSession
if ($live) {
    Write-Host "skip Watch stale-clear (helper already running pid $($live.process_id))"
} else {
    $dir = Get-FakescanElevatedHelperDir
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    $req = Join-Path $dir 'request.json'
    $rep = Join-Path $dir 'reply.json'
    $sessionPath = Join-Path $dir 'session.json'
    '{ "id": "stale", "cmd": "remove" }' | Set-Content -LiteralPath $req -Encoding utf8
    '{ "id": "old", "ok": true }' | Set-Content -LiteralPath $rep -Encoding utf8
    $job = Start-Job -ScriptBlock {
        param($mod, $idle)
        Import-Module $mod -Force
        Watch-FakescanElevatedHelper -IdleSeconds $idle -AllowUnelevated
    } -ArgumentList (Join-Path $here 'Fakescan.psm1'), 1
    try {
        $saw = $false
        foreach ($i in 1..40) {
            Start-Sleep -Milliseconds 100
            if (Test-Path -LiteralPath $sessionPath) { $saw = $true; break }
        }
        Assert-True $saw 'Watch wrote session.json'
        Start-Sleep -Milliseconds 200
        Assert-True (-not (Test-Path -LiteralPath $req)) 'Watch cleared stale request.json on start'
        Assert-True (-not (Test-Path -LiteralPath $rep)) 'Watch cleared stale reply.json on start'
        $null = Wait-Job $job -Timeout 15
    } finally {
        if ($job.State -eq 'Running') { Stop-Job $job -Force -ErrorAction SilentlyContinue }
        Remove-Job $job -Force -ErrorAction SilentlyContinue
        Remove-Item -LiteralPath $sessionPath, $req, $rep -ErrorAction SilentlyContinue
    }
}

if ($failed -gt 0) {
    throw "check-elev-helper: $failed assertion(s) failed"
}
Write-Host 'check-elev-helper: all ok'
