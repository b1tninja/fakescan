#Requires -Version 5.1
<#
.SYNOPSIS
  Tiny local smoke: Test-FakescanLooksLike never matches Brother; matches fakescan.
  No elevation, no network, no CI gate — run by hand when editing Fakescan.psm1.

.EXAMPLE
  .\scripts\smoke-looks-like.ps1
#>
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
Import-Module (Join-Path $here 'Fakescan.psm1') -Force

if (Test-FakescanLooksLike -Name 'Brother MFC-L2750DW series') {
    throw 'Brother name must not match Test-FakescanLooksLike'
}
if (Test-FakescanLooksLike -Name 'WSD Scanner' -Port 'http://192.168.1.5/ScannerService') {
    throw 'generic /ScannerService alone must not match'
}
if (-not (Test-FakescanLooksLike -Name 'Fake WSD scanner (lite caps)')) {
    throw 'expected Fake WSD scanner name to match'
}
if (-not (Test-FakescanLooksLike -XAddrs @('http://192.168.1.70:3920/'))) {
    throw 'expected :3920 XAddr to match'
}

# WhatIf path lists targets without needing elevation (may be empty on clean labs).
$targets = @(Get-FakescanPnpRemoveTargets)
Write-Host ("Get-FakescanPnpRemoveTargets count={0} (Image+SoftwareDevice; empty is OK)" -f $targets.Count)
$targets | Format-Table Kind, Status, Name, InstanceId -AutoSize | Out-Host

Write-Host 'smoke-looks-like OK'
