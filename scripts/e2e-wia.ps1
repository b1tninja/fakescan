#Requires -Version 5.1
<#
.SYNOPSIS
  End-to-end WIA path: discover -> install assist -> assert WIA -> optional pdfcraft-cli scan.

.DESCRIPTION
  1) /health
  2) Test-FakescanDiscoverable (Hello / on-wire OK)
  3) Install-FakescanScanner with wait (opens Add device unless -SkipUi)
  4) Assert WIA/PnP shows fakescan
  5) If pdfcraft-cli is on PATH (or -PdfcraftCli), run scan devices (filter fakescan/wsd),
     glass acquire 1 page, set feeder/pages, feeder acquire
  6) Optional -Remove calls Remove-FakescanScanner

.EXAMPLE
  .\scripts\e2e-wia.ps1 -BaseUrl http://192.168.1.10:3920 -WaitSeconds 120

.EXAMPLE
  .\scripts\e2e-wia.ps1 -BaseUrl http://192.168.1.10:3920 -SkipUi -PdfcraftCli D:\code\pdfcraft\target\release\pdfcraft-cli.exe -Remove
#>
[CmdletBinding()]
param(
    [string]$BaseUrl = 'http://127.0.0.1:3920',
    [int]$WaitSeconds = 120,
    [switch]$SkipUi,
    [switch]$ProbeDiscovery,
    [string]$PdfcraftCli,
    [switch]$Remove,
    [string]$OutDir
)

$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
Import-Module (Join-Path $here 'Fakescan.psm1') -Force

$BaseUrl = Resolve-FakescanBaseUrl $BaseUrl
if ([string]::IsNullOrWhiteSpace($OutDir)) {
    $OutDir = Join-Path ([System.IO.Path]::GetTempPath()) ("fakescan-e2e-" + [guid]::NewGuid().ToString('n'))
}

function Resolve-PdfcraftCli {
    param([string]$Explicit)
    if (-not [string]::IsNullOrWhiteSpace($Explicit)) {
        if (-not (Test-Path -LiteralPath $Explicit)) {
            throw "pdfcraft-cli not found at -PdfcraftCli path: $Explicit"
        }
        return (Resolve-Path -LiteralPath $Explicit).Path
    }
    $cmd = Get-Command 'pdfcraft-cli' -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    $cmd = Get-Command 'pdfcraft-cli.exe' -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    return $null
}

function Invoke-PdfcraftCli {
    param(
        [Parameter(Mandatory)][string]$Exe,
        [Parameter(Mandatory)][string[]]$Args
    )
    Write-Host ("pdfcraft-cli " + ($Args -join ' '))
    & $Exe @Args
    if ($LASTEXITCODE -ne 0) {
        throw "pdfcraft-cli exited with code $LASTEXITCODE"
    }
}

function Get-FakescanDeviceNameFromDevicesJson {
    param([string]$JsonText)
    $obj = $JsonText | ConvertFrom-Json
    $scanners = @($obj.scanners)
    $hit = $scanners | Where-Object {
        $name = [string]$_.name
        $id = [string]$_.id
        $port = [string]$_.port
        $conn = [string]$_.connection
        $looks = Test-FakescanLooksLike -Name $name -Port $port
        if (-not $looks -and $id) {
            $looks = Test-FakescanLooksLike -Name $id -Port $port
        }
        # Prefer WSD; allow blank connection when name/port already looks like fakescan.
        $wsdOk = ($conn -eq '' -or $conn -match '(?i)wsd')
        $looks -and $wsdOk
    } | Select-Object -First 1
    if (-not $hit) { return $null }
    return [string]$hit.name
}

Write-Host "=== e2e-wia: health ($BaseUrl) ==="
$health = Get-FakescanHealth -BaseUrl $BaseUrl
if (-not $health.Ok) {
    throw "fakescan /health failed at $BaseUrl (got '$($health.Body)')"
}
Write-Host 'health OK'

Write-Host '=== e2e-wia: discoverable (Hello / on-wire) ==='
$disc = Test-FakescanDiscoverable -BaseUrl $BaseUrl -DiscoveryTimeoutMs 5000
$disc | Format-List BaseUrl, SoapReachable, UdpProbeMatch, AlreadyInstalled, Verdict
$disc.Notes | ForEach-Object { Write-Host "  $_" }
if (-not $disc.SoapReachable) {
    throw "SOAP not reachable at $BaseUrl (verdict=$($disc.Verdict))"
}
if ($disc.Verdict -eq 'ghost-aep') {
    throw @'
Ghost AEP: PnP Image gone, AssociationEndpoint still IsPaired (Settings hides Fake).
  .\scripts\fakescan.ps1 rebind -BaseUrl <URL> -WaitSeconds 120
  (Unpair AccessDenied → bounce-fd: fdPHost then DeviceAssociationService; never dasHost)
Then Printers & scanners → Add device.
'@
}
if ($disc.Verdict -eq 'installed-offline') {
    throw @'
Sticky WIA offline (PnP residue, Connect 0x80210005 / no SOAP). stisvc bounce does not clear it.
  .\scripts\fakescan.ps1 rebind -BaseUrl <URL> -WaitSeconds 120
Then finish Printers & scanners → Add device while fakescan stays up.
'@
}
if (-not $disc.UdpProbeMatch -and -not $disc.AlreadyInstalled) {
    throw @"
WS-Discovery Hello/announce not visible (verdict=$($disc.Verdict)).
Fix: native fakescan --advertise <LAN-IP>, UDP 3702, then re-run.
"@
}
Write-Host "discoverable OK (verdict=$($disc.Verdict))"

Write-Host '=== e2e-wia: install assist ==='
$install = Install-FakescanScanner -BaseUrl $BaseUrl -WaitSeconds $WaitSeconds `
    -ProbeDiscovery:$ProbeDiscovery -SkipUi:$SkipUi
$install | Format-List BaseUrl, HealthOk, SoapOk, ScannerName, Installed

Write-Host '=== e2e-wia: assert WIA ==='
$wia = Get-FakescanWiaDevices
$wia.Wia | Format-Table -AutoSize | Out-Host
$wia.Pnp | Format-Table -AutoSize | Out-Host
if (-not $wia.FakescanInWia) {
    if ($wia.StickyPresenceHint -or $wia.FakescanPnpOnly) {
        throw @'
PnP residue without usable WIA enum (sticky/offline). Run elevated rebind:
  .\scripts\fakescan.ps1 elevated
  .\scripts\fakescan.ps1 rebind -BaseUrl <URL> -WaitSeconds 120
Then finish Printers & scanners → Add device.
'@
    }
    throw @'
No fakescan-like WIA device after install wait.
Complete Printers & scanners → Add device for "Fake WSD scanner", then re-run (or pass a longer -WaitSeconds).
'@
}
Write-Host 'WIA assert OK (FakescanInWia)'

$cli = Resolve-PdfcraftCli -Explicit $PdfcraftCli
$result = [pscustomobject]@{
    BaseUrl = $BaseUrl
    DiscoverableVerdict = $disc.Verdict
    Installed = [bool]$wia.FakescanPresent
    PdfcraftCli = $cli
    DeviceName = $null
    GlassOut = $null
    FeederOut = $null
    Removed = $false
    SkippedScan = $false
}

if (-not $cli) {
    Write-Warning 'pdfcraft-cli not found on PATH and -PdfcraftCli not set; skipping scan acquire steps.'
    $result.SkippedScan = $true
} else {
    Write-Host "=== e2e-wia: pdfcraft-cli scan devices ($cli) ==="
    $devicesJson = & $cli 'scan' 'devices' 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) {
        throw "pdfcraft-cli scan devices failed: $devicesJson"
    }
    Write-Host $devicesJson.Trim()
    $deviceName = Get-FakescanDeviceNameFromDevicesJson -JsonText $devicesJson
    if ([string]::IsNullOrWhiteSpace($deviceName)) {
        throw 'pdfcraft-cli scan devices listed no fakescan/wsd device (filter: name/id looks like fakescan and connection wsd when present).'
    }
    $result.DeviceName = $deviceName
    Write-Host "Using device: $deviceName"

    New-Item -ItemType Directory -Path $OutDir -Force | Out-Null
    $glassDir = Join-Path $OutDir 'glass'
    $feederDir = Join-Path $OutDir 'feeder'
    New-Item -ItemType Directory -Path $glassDir -Force | Out-Null
    New-Item -ItemType Directory -Path $feederDir -Force | Out-Null

    Write-Host '=== e2e-wia: glass acquire (1 page) ==='
    # Flatbed does not consume feeder pages; still reset for a clean status.
    Reset-Fakescan -BaseUrl $BaseUrl | Out-Null
    Invoke-PdfcraftCli -Exe $cli -Args @(
        'scan',
        '--out', $glassDir,
        '--yes',
        '--source', 'glass',
        '--pages', '1',
        '--device', $deviceName
    )
    $glassFiles = @(Get-ChildItem -LiteralPath $glassDir -File -ErrorAction SilentlyContinue)
    if ($glassFiles.Count -lt 1) {
        throw "glass acquire produced no files under $glassDir"
    }
    $result.GlassOut = $glassDir
    Write-Host "glass OK ($($glassFiles.Count) file(s) in $glassDir)"

    Write-Host '=== e2e-wia: feeder acquire ==='
    Set-FakescanPages -Count 2 -BaseUrl $BaseUrl | Out-Null
    Set-FakescanFeeder -Ready $true -BaseUrl $BaseUrl | Out-Null
    Invoke-PdfcraftCli -Exe $cli -Args @(
        'scan',
        '--out', $feederDir,
        '--yes',
        '--source', 'feeder',
        '--pages', '2',
        '--device', $deviceName
    )
    $feederFiles = @(Get-ChildItem -LiteralPath $feederDir -File -ErrorAction SilentlyContinue)
    if ($feederFiles.Count -lt 1) {
        throw "feeder acquire produced no files under $feederDir"
    }
    $result.FeederOut = $feederDir
    Write-Host "feeder OK ($($feederFiles.Count) file(s) in $feederDir)"
}

if ($Remove) {
    Write-Host '=== e2e-wia: Remove-FakescanScanner ==='
    $rm = Remove-FakescanScanner
    $rm | Format-List
    $result.Removed = ($rm.Removed.Count -gt 0 -or $rm.Matched.Count -eq 0)
}

Write-Host '=== e2e-wia: done ==='
$result | Format-List
$result
