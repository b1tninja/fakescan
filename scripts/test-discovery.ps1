#Requires -Version 5.1
<#
.SYNOPSIS
  Verify fakescan WS-Discovery UDP announce (Hello) and optional ProbeMatch.

.DESCRIPTION
  Windows WSD uses WS-Discovery on UDP 3702 / 239.255.255.250 (not classic SSDP :1900).

  On Windows, dasHost often monopolizes inbound :3702, so same-host ProbeMatch / Hello
  listens can fail even when announce works. This script accepts any of:

    1. /v1/status wsd_hellos_sent increases (server-side send proof)
    2. Find-FakescanHello hears a Hello with :3920
    3. Find-FakescanDiscovery gets a fakescan ProbeMatch

.EXAMPLE
  .\scripts\test-discovery.ps1 -BaseUrl http://192.168.1.70:3920
#>
[CmdletBinding()]
param(
    [int]$TimeoutMs = 8000,
    [string]$BaseUrl = 'http://127.0.0.1:3920',
    [string]$UnicastHost
)

$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
Import-Module (Join-Path $here 'Fakescan.psm1') -Force

Write-Host "SOAP check $BaseUrl ..."
$soap = Test-FakescanSoap -BaseUrl $BaseUrl
if (-not $soap.Ok) {
    throw "SOAP failed - start fakescan first (native: --listen 0.0.0.0:3920 --advertise <LAN-IP>)"
}
Write-Host "SOAP OK ($($soap.Snippet))"

$status0 = Get-FakescanStatus -BaseUrl $BaseUrl
$hellos0 = [uint64]$status0.wsd_hellos_sent
Write-Host "wsd_hellos_sent=$hellos0 device=$($status0.device_endpoint) scan=$($status0.service_endpoint)"

if ([string]::IsNullOrWhiteSpace($UnicastHost)) {
    try {
        $u = [Uri]$BaseUrl
        if ($u.Host -notmatch '^(127\.|localhost)') { $UnicastHost = $u.Host }
    } catch { }
}

Write-Host "1) Wait for Hello counter to advance (${TimeoutMs}ms) ..."
$helloSendOk = $false
$deadline = [datetime]::UtcNow.AddMilliseconds($TimeoutMs)
$hellos1 = $hellos0
while ([datetime]::UtcNow -lt $deadline) {
    Start-Sleep -Milliseconds 800
    $st = Get-FakescanStatus -BaseUrl $BaseUrl
    $hellos1 = [uint64]$st.wsd_hellos_sent
    if ($hellos1 -gt $hellos0) {
        $helloSendOk = $true
        Write-Host "OK - Hellos sent advanced $hellos0 -> $hellos1 (UDP multicast send path)"
        break
    }
}
if (-not $helloSendOk) {
    Write-Warning "wsd_hellos_sent did not increase (still $hellos1). Is WSD enabled?"
}

Write-Host "2) Listen for WS-Discovery Hello on :3702 ..."
$hellos = @(Find-FakescanHello -TimeoutMs ([Math]::Min(6000, $TimeoutMs)))
$helloHearOk = $false
if ($hellos.Count -gt 0) {
    $helloHearOk = $true
    Write-Host "OK - Hello received on wire:"
    $hellos | Format-Table Peer, DeviceUuid, XAddrs -AutoSize
} else {
    Write-Warning "No Hello heard locally (common when dasHost owns :3702). Counter proof still counts."
}

$probeLabel = 'multicast'
if ($UnicastHost) { $probeLabel = "multicast + unicast $UnicastHost" }
Write-Host "3) Probe ($probeLabel) ..."
$matches = @(Find-FakescanDiscovery -TimeoutMs ([Math]::Min(4000, $TimeoutMs)) -UnicastHost $UnicastHost)
$mine = @($matches | Where-Object { $_.LooksLikeFakescan })
$probeOk = $false
if ($mine.Count -gt 0) {
    $probeOk = $true
    Write-Host "OK - ProbeMatch received:"
    $mine | Format-Table Peer, DeviceUuid, XAddrs -AutoSize
} else {
    Write-Warning "No ProbeMatch (expected on Windows Hello-only; use --probe-listen on Linux)."
}

if (-not $helloSendOk -and -not $helloHearOk -and -not $probeOk) {
    Write-Error "WS-Discovery UDP path not visible."
    exit 1
}

if ($probeOk) {
    Write-Host "Verdict: probe-response-ok"
} elseif ($helloHearOk) {
    Write-Host "Verdict: hello-announce-ok (on-wire)"
} else {
    Write-Host "Verdict: hello-announce-ok (status counter)"
}
exit 0

