#Requires -Version 5.1
<#
.SYNOPSIS
  Pester-free tests for the PowerShell registry writer and `ensure` planning.
  Uses a temp FAKESCAN_REGISTRY only; starts no fakescan, never touches port 3920 or the real registry.
.EXAMPLE
  powershell -File scripts\test-registry.ps1
#>
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$tmpDir = Join-Path ([IO.Path]::GetTempPath()) ("fakescan-regtest-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $tmpDir | Out-Null
$env:FAKESCAN_REGISTRY = Join-Path $tmpDir 'devices.json'
$real = Join-Path $env:LOCALAPPDATA 'fakescan\devices.json'
if ((Resolve-Path -LiteralPath $tmpDir).Path -eq (Split-Path -Parent $real)) { throw 'refusing: temp dir is the real dir' }
Import-Module (Join-Path $here 'Fakescan.psm1') -Force

$script:fail = 0; $script:pass = 0
function Assert($cond, $msg) {
    if ($cond) { $script:pass++; Write-Host "ok   $msg" } else { $script:fail++; Write-Host "FAIL $msg" }
}

try {
    # --- ensure planning: 3 rows on one listen + 1 on another => 2 processes
    $exe = Join-Path $PSHOME 'powershell.exe'
    $rows = @(
        foreach ($n in 'tray5', 'tray50', 'duplex30') {
            [pscustomobject]@{ name = $n; listen = '127.0.0.1:43920'; advertise = ''; device_uuid = [guid]::NewGuid().ToString(); profile = 'brother-wsd'; autostart = ($n -eq 'tray5'); exe = $exe }
        }
        [pscustomobject]@{ name = 'other'; listen = '127.0.0.1:43922'; advertise = '10.0.0.9'; device_uuid = [guid]::NewGuid().ToString(); profile = 'brother-wsd'; autostart = $true; exe = $exe }
    )
    ([pscustomobject]@{ devices = $rows } | ConvertTo-Json -Depth 6) | Set-Content -LiteralPath $env:FAKESCAN_REGISTRY -Encoding UTF8

    $plans = @(Get-FakescanEnsurePlan -All)
    Assert ($plans.Count -eq 2) "ensure -All plans one process per distinct listen (got $($plans.Count))"
    $p1 = $plans | Where-Object { $_.Listen -eq '127.0.0.1:43920' }
    Assert (@($p1.Names).Count -eq 3) 'listen with 3 rows is ONE process covering all 3 rows (even the non-autostart ones)'
    Assert ($p1.Args -contains '--manage' -and $p1.Args -notcontains '--lab-name' -and $p1.Args -notcontains '--device-uuid') 'plan uses --manage, no per-row flags'
    $p2 = $plans | Where-Object { $_.Listen -eq '127.0.0.1:43922' }
    Assert ($p2.Args -contains '--advertise' -and $p2.Args -contains '10.0.0.9') 'advertise carried through'
    $byName = @(Get-FakescanEnsurePlan -Name duplex30)
    Assert ($byName.Count -eq 1 -and @($byName[0].Names).Count -eq 3) '-Name picks the row''s whole listen group'

    $out = & powershell.exe -NoProfile -File (Join-Path $here 'fakescan.ps1') ensure -All -WhatIf 2>&1 | Out-String
    $n = ([regex]::Matches($out, 'WhatIf: would start')).Count
    Assert ($n -eq 2) "ensure -All -WhatIf prints exactly 2 planned command lines (got $n)"
    Assert ($out -match '--manage') 'planned line runs --manage'

    # --- concurrent writers must not lose rows
    Remove-Item -LiteralPath $env:FAKESCAN_REGISTRY -Force
    $jobs = foreach ($w in 1..4) {
        Start-Job -ArgumentList $here, $env:FAKESCAN_REGISTRY, $w -ScriptBlock {
            param($dir, $reg, $w)
            $env:FAKESCAN_REGISTRY = $reg
            Import-Module (Join-Path $dir 'Fakescan.psm1') -Force
            foreach ($i in 1..10) {
                Add-FakescanDevice -Name "w$w-$i" -Listen '127.0.0.1:43920' -DeviceUuid ([guid]::NewGuid().ToString()) | Out-Null
            }
        }
    }
    $jobs | Wait-Job -Timeout 120 | Out-Null
    $jobs | ForEach-Object { if ($_.State -eq 'Failed') { Write-Host ($_.ChildJobs[0].JobStateInfo.Reason) } }
    $jobs | Remove-Job -Force
    $count = @((Get-FakescanRegistry).devices).Count
    Assert ($count -eq 40) "4 concurrent writers x 10 rows => 40 rows (got $count)"
    Assert (-not (Get-ChildItem $tmpDir -Filter '*.tmp.*')) 'no leftover temp files'
} finally {
    Remove-Item -LiteralPath $tmpDir -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Host "passed=$script:pass failed=$script:fail"
exit $script:fail
