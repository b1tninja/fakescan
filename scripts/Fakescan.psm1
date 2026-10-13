#Requires -Version 5.1
<#
.SYNOPSIS
  Helpers to find, probe (SOAP), manage, and assist Windows install of fakescan.
#>

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$script:DefaultBaseUrl = 'http://127.0.0.1:3920'

function Show-FakescanBanner {
    <#
    .SYNOPSIS
      Flatbed ASCII mark so a UAC / helper window is clearly the fakescan lab.
    .DESCRIPTION
      Sets the console title and prints a small scanner tile + wordmark. Use on
      elevated helper / shell start and before person-required attention, so the
      window does not look like a random admin PowerShell.
    #>
    [CmdletBinding()]
    param(
        # One-line role under the mark (helper, scanner process, Add device, …).
        [string]$Role = 'lab fake WSD scanner — not malware'
    )
    try {
        $host.UI.RawUI.WindowTitle = 'fakescan — lab fake WSD scanner'
    } catch {}
    $art = @'

   .-----------------------------.     fakescan
  |  _________________________  |     lab fake WSD scanner
  | | ####  PAGE ON GLASS  ## | |     not malware — test harness
  | | ======================= | |     sibling of fakeprint
  | |__________ Fs ___________| |
   '-----------------------------'
'@
    $block = @"
$art
  $Role
  https://github.com/b1tninja/fakescan

"@
    try {
        $prev = $host.UI.RawUI.ForegroundColor
        $host.UI.RawUI.ForegroundColor = 'Cyan'
        Write-Host $block
        $host.UI.RawUI.ForegroundColor = $prev
    } catch {
        Write-Host $block
    }
}

# ── Invoke-LabAttention ────────────────────────────────────────────────────
# Private helper (not exported). Called from Open-FakescanAddDevice and the
# ensure flow whenever added.pnp is false and no silent install has succeeded.
#
# Default: yellow banner + [Console]::Beep(1000,300) + BEL char + WinForms
#          NotifyIcon balloon (held 3 s so Windows can show it).
# -Loud:   also FlashWindowEx on the first parent-process window handle.
# -Quiet:  banner only; suppresses beep and balloon.

function Invoke-LabAttention {
    [CmdletBinding()]
    param(
        # One-line context printed inside the banner.
        [string]$Detail = 'Add device next to "Fake WSD scanner (lite caps)".',
        # Add flashy taskbar flashing via FlashWindowEx.
        [switch]$Loud,
        # Suppress beep and balloon; banner still prints.
        [switch]$Quiet,
        # How long the clickable tray icon / notification stays alive (seconds).
        [int]$HoldSeconds = 120
    )

    # ── 1. Mark + yellow person-required banner (always) ─────────────────
    Show-FakescanBanner -Role 'PERSON REQUIRED — Add device in Printers & scanners'
    $line  = '=' * 66
    $block = @"

$line
  !!  fakescan — PERSON REQUIRED — Add device
      $Detail
      Settings -> Printers & scanners is opening now.
      Click the notification (or the tray icon) to open it; then click  Add device  next to the fakescan entry.
      Verify when done:  .\scripts\fakescan.ps1 wia
$line

"@
    try {
        $prev = $host.UI.RawUI.ForegroundColor
        $host.UI.RawUI.ForegroundColor = 'Yellow'
        Write-Host $block
        $host.UI.RawUI.ForegroundColor = $prev
    } catch {
        Write-Host $block
    }

    if (-not $Quiet) {
        # ── 2. Audible beep ─────────────────────────────────────────────
        try { [Console]::Beep(1000, 300) } catch { }
        # ── 4. BEL character alongside beep ──────────────────────────────
        try { [Console]::Write([char]0x7) } catch { }

        # ── 3. Clickable notification ────────────────────────────────────
        # A detached, hidden STA PowerShell owns the NotifyIcon so this call returns at once and
        # the notification outlives it. Clicking the balloon/toast, or the tray icon, opens
        # Settings -> Printers & scanners; the helper exits after the click or $HoldSeconds.
        try {
            $helper = @'
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
$script:clicked = $false
$ni = New-Object System.Windows.Forms.NotifyIcon
$ni.Icon = [System.Drawing.SystemIcons]::Warning
$ni.Text = 'fakescan: Add device required (click to open Settings)'
$ni.Visible = $true
# Windows 11 raises a spurious click shortly after the balloon appears; ignore the first 2.5 s.
$script:shownAt = [datetime]::UtcNow
$open = { if (([datetime]::UtcNow - $script:shownAt).TotalSeconds -lt 2.5) { return }; Start-Process 'ms-settings:printers'; $script:clicked = $true }
$ni.add_BalloonTipClicked($open)
$ni.add_Click($open)
$script:shownAt = [datetime]::UtcNow
$ni.ShowBalloonTip(15000, 'fakescan - Add device required', '__DETAIL__ Click to open Settings.', [System.Windows.Forms.ToolTipIcon]::Warning)
$end = (Get-Date).AddSeconds(__HOLD__)
while (-not $script:clicked -and (Get-Date) -lt $end) {
    [System.Windows.Forms.Application]::DoEvents()
    Start-Sleep -Milliseconds 100
}
Start-Sleep -Milliseconds 300
$ni.Visible = $false
$ni.Dispose()
'@
            $safe = ($Detail -replace "'", "''" -replace '[\r\n]+', ' ')
            $helper = $helper.Replace('__DETAIL__', $safe).Replace('__HOLD__', [string][Math]::Max(5, $HoldSeconds))
            $enc = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($helper))
            Start-Process -FilePath 'powershell.exe' -WindowStyle Hidden `
                -ArgumentList '-NoProfile', '-STA', '-ExecutionPolicy', 'Bypass', '-EncodedCommand', $enc
        } catch {
            Write-Verbose "Invoke-LabAttention notification: $_"
        }
    }

    # ── Opt-in: FlashWindowEx on parent terminal window ──────────────────
    if ($Loud) {
        try {
            if (-not ('FakescanLabFlash' -as [type])) {
                Add-Type -Language CSharp -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class FakescanLabFlash {
    [StructLayout(LayoutKind.Sequential)]
    public struct FLASHWINFO {
        public uint  cbSize;
        public IntPtr hwnd;
        public uint  dwFlags;
        public uint  uCount;
        public uint  dwTimeout;
    }
    [DllImport("user32.dll")]
    public static extern bool FlashWindowEx(ref FLASHWINFO pwfi);
    public const uint FLASHW_ALL      = 3;
    public const uint FLASHW_TIMERNOFG = 12;
    public static void Flash(IntPtr hwnd) {
        if (hwnd == IntPtr.Zero) return;
        var fi = new FLASHWINFO {
            cbSize   = (uint)System.Runtime.InteropServices.Marshal.SizeOf(typeof(FLASHWINFO)),
            hwnd     = hwnd,
            dwFlags  = FLASHW_ALL | FLASHW_TIMERNOFG,
            uCount   = 6,
            dwTimeout = 0
        };
        FlashWindowEx(ref fi);
    }
}
'@ -ErrorAction Stop
            }
            # Walk parent process tree to find the first window handle.
            $walkPid = $PID
            $seen    = [System.Collections.Generic.HashSet[int]]::new()
            $hwnd    = [IntPtr]::Zero
            while ($walkPid -gt 0 -and $seen.Add($walkPid)) {
                try {
                    $p = [System.Diagnostics.Process]::GetProcessById($walkPid)
                    if ($p.MainWindowHandle -ne [IntPtr]::Zero) {
                        $hwnd = $p.MainWindowHandle; break
                    }
                } catch { }
                try {
                    $cimRow  = Get-CimInstance -ClassName Win32_Process `
                                   -Filter "ProcessId = $walkPid" -ErrorAction Stop
                    $walkPid = [int]$cimRow.ParentProcessId
                } catch { break }
            }
            [FakescanLabFlash]::Flash($hwnd)
        } catch {
            Write-Verbose "Invoke-LabAttention FlashWindowEx: $_"
        }
    }
}

$script:ScanNs = 'http://schemas.microsoft.com/windows/2006/01/wdp/scan'
$script:DiscoveryMcast = [System.Net.IPAddress]::Parse('239.255.255.250')
$script:DiscoveryPort = 3702
$script:ScannerType = 'http://schemas.microsoft.com/windows/2006/08/wdp/scan/ScannerServiceType'

function Resolve-FakescanBaseUrl {
    param([string]$BaseUrl)
    if ([string]::IsNullOrWhiteSpace($BaseUrl)) { return $script:DefaultBaseUrl }
    return $BaseUrl.TrimEnd('/')
}

function Test-FakescanLooksLike {
    <#
    .SYNOPSIS
      True only for fakescan identity hints: name contains "fakescan" / "Fake WSD scanner",
      a port/URL contains :3920, or a lab **scan** AEP/PnP id (`DAFWSD…000fN`). Does NOT
      treat generic /ScannerService as fakescan (Brother uses that path too), and does NOT
      match fakeprint IPP nodes that share the same lab UUID prefix (`SWD\IPP\…000fN`).
    .NOTES
      After pnputil remove, WinRT AEP can keep IsPaired=true with an **empty Name**.
      Match the lab UUID only on DAFWSDProvider / AssociationEndpoint ids so
      Disconnect-FakescanAssociationEndpoint still Unpairs; bare UUID alone would
      falsely match fakeprint and remove a printer.
    #>
    [CmdletBinding()]
    param(
        [string]$Name,
        [string]$Port,
        [string[]]$XAddrs
    )
    $parts = New-Object System.Collections.Generic.List[string]
    if (-not [string]::IsNullOrWhiteSpace($Name)) { [void]$parts.Add($Name) }
    if (-not [string]::IsNullOrWhiteSpace($Port)) { [void]$parts.Add($Port) }
    foreach ($x in @($XAddrs)) {
        if (-not [string]::IsNullOrWhiteSpace($x)) { [void]$parts.Add($x) }
    }
    if ($parts.Count -eq 0) { return $false }
    $joined = ($parts -join ' ')
    # Never treat fakeprint / IPP / WSD print as fakescan (shared lab UUID family).
    if ($joined -match '(?i)\\IPP\\|Fake WSD Print|Fake WSD printer|fakeprint|PrinterService|:3921|00000000-0000-4000-8000-0000000000f3|00000000-0000-4000-8000-0000000000f4') {
        return $false
    }
    return [bool](
        ($joined -match '(?i)fakescan') -or
        ($joined -match '(?i)Fake WSD scanner') -or
        ($joined -match ':3920') -or
        # Lab scan UUIDs …f2/f5/f6… — exclude …f3/f4 (fakeprint / IPP).
        ($joined -match '(?i)00000000-0000-4000-8000-0000000000f(?!3|4)[0-9a-f]')
    )
}

function Get-FakescanHealth {
    <#
    .SYNOPSIS
      GET /health on a fakescan host.
    #>
    [CmdletBinding()]
    param(
        [string]$BaseUrl = $script:DefaultBaseUrl
    )
    $BaseUrl = Resolve-FakescanBaseUrl $BaseUrl
    $r = Invoke-WebRequest -Uri "$BaseUrl/health" -UseBasicParsing -TimeoutSec 5
    [pscustomobject]@{
        BaseUrl = $BaseUrl
        StatusCode = [int]$r.StatusCode
        Body = $r.Content.Trim()
        Ok = ($r.StatusCode -eq 200 -and $r.Content.Trim() -eq 'ok')
    }
}

function Get-FakescanStatus {
    <#
    .SYNOPSIS
      GET /v1/status (profile, feeder, pages, caps).
    #>
    [CmdletBinding()]
    param(
        [string]$BaseUrl = $script:DefaultBaseUrl
    )
    $BaseUrl = Resolve-FakescanBaseUrl $BaseUrl
    Invoke-RestMethod -Uri "$BaseUrl/v1/status" -TimeoutSec 5
}

function Set-FakescanProfile {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)][ValidateSet('brother-wsd', 'brother-usb')]
        [string]$Id,
        [string]$BaseUrl = $script:DefaultBaseUrl
    )
    $BaseUrl = Resolve-FakescanBaseUrl $BaseUrl
    Invoke-RestMethod -Method Post -Uri "$BaseUrl/v1/profile" `
        -ContentType 'application/json' `
        -Body (@{ id = $Id } | ConvertTo-Json -Compress) `
        -TimeoutSec 5
}

function Set-FakescanFeeder {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)][bool]$Ready,
        [string]$BaseUrl = $script:DefaultBaseUrl
    )
    $BaseUrl = Resolve-FakescanBaseUrl $BaseUrl
    Invoke-RestMethod -Method Post -Uri "$BaseUrl/v1/feeder" `
        -ContentType 'application/json' `
        -Body (@{ ready = $Ready } | ConvertTo-Json -Compress) `
        -TimeoutSec 5
}

function Set-FakescanPages {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)][ValidateRange(0, 30)]
        [int]$Count,
        [ValidateSet('front', 'back')]
        [string]$Side,
        [ValidateSet('forward', 'reverse')]
        [string]$Order,
        [string]$BaseUrl = $script:DefaultBaseUrl
    )
    $BaseUrl = Resolve-FakescanBaseUrl $BaseUrl
    $body = @{ count = $Count }
    if ($PSBoundParameters.ContainsKey('Side') -and $Side) { $body.side = $Side }
    if ($PSBoundParameters.ContainsKey('Order') -and $Order) { $body.order = $Order }
    Invoke-RestMethod -Method Post -Uri "$BaseUrl/v1/pages" `
        -ContentType 'application/json' `
        -Body ($body | ConvertTo-Json -Compress) `
        -TimeoutSec 5
}

function Set-FakescanFault {
    <#
    .SYNOPSIS
      POST /v1/fault (jam, cover_open, jam_after_n, empty_mid_job, …).
    .NOTES
      Prefer the Python lab client for scripted harnesses (docs/lab-harness.md).
    #>
    [CmdletBinding()]
    param(
        [bool]$Jam,
        [bool]$CoverOpen,
        [int]$Ms,
        [int]$JamAfterN,
        [int]$EmptyMidJob,
        [int]$LampWarmupMs,
        [string]$BaseUrl = $script:DefaultBaseUrl
    )
    $BaseUrl = Resolve-FakescanBaseUrl $BaseUrl
    $body = @{}
    if ($PSBoundParameters.ContainsKey('Jam')) { $body.jam = $Jam }
    if ($PSBoundParameters.ContainsKey('CoverOpen')) { $body.cover_open = $CoverOpen }
    if ($PSBoundParameters.ContainsKey('Ms')) { $body.ms = $Ms }
    if ($PSBoundParameters.ContainsKey('JamAfterN')) { $body.jam_after_n = $JamAfterN }
    if ($PSBoundParameters.ContainsKey('EmptyMidJob')) { $body.empty_mid_job = $EmptyMidJob }
    if ($PSBoundParameters.ContainsKey('LampWarmupMs')) { $body.lamp_warmup_ms = $LampWarmupMs }
    Invoke-RestMethod -Method Post -Uri "$BaseUrl/v1/fault" `
        -ContentType 'application/json' `
        -Body ($body | ConvertTo-Json -Compress) `
        -TimeoutSec 5
}

function Reset-Fakescan {
    [CmdletBinding()]
    param([string]$BaseUrl = $script:DefaultBaseUrl)
    $BaseUrl = Resolve-FakescanBaseUrl $BaseUrl
    Invoke-RestMethod -Method Post -Uri "$BaseUrl/v1/reset" -TimeoutSec 5
}

function Get-FakescanScenarios {
    [CmdletBinding()]
    param([string]$BaseUrl = $script:DefaultBaseUrl)
    $BaseUrl = Resolve-FakescanBaseUrl $BaseUrl
    Invoke-RestMethod -Uri "$BaseUrl/v1/scenarios" -TimeoutSec 5
}

function Set-FakescanScenario {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [ValidateSet('order', 'qr', 'tone', 'color', 'orient', 'crop', 'duplex')]
        [string]$Id,
        [string]$BaseUrl = $script:DefaultBaseUrl
    )
    $BaseUrl = Resolve-FakescanBaseUrl $BaseUrl
    Invoke-RestMethod -Method Post -Uri "$BaseUrl/v1/scenario" `
        -ContentType 'application/json' `
        -Body (@{ id = $Id } | ConvertTo-Json -Compress) `
        -TimeoutSec 5
}

function New-FakescanSoapEnvelope {
    param(
        [Parameter(Mandatory)][string]$ActionLocalName,
        [Parameter(Mandatory)][string]$BodyXml
    )
    $msgId = [guid]::NewGuid().ToString()
    $action = "$script:ScanNs/$ActionLocalName"
    @"
<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope"
  xmlns:wsa="http://schemas.xmlsoap.org/ws/2004/08/addressing"
  xmlns:wscn="$script:ScanNs">
  <soap:Header>
    <wsa:Action>$action</wsa:Action>
    <wsa:MessageID>uuid:$msgId</wsa:MessageID>
  </soap:Header>
  <soap:Body>
$BodyXml
  </soap:Body>
</soap:Envelope>
"@
}

function Invoke-FakescanSoap {
    <#
    .SYNOPSIS
      POST a SOAP envelope to /ScannerService.
    #>
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)][string]$Body,
        [string]$BaseUrl = $script:DefaultBaseUrl,
        [string]$Path = '/ScannerService'
    )
    $BaseUrl = Resolve-FakescanBaseUrl $BaseUrl
    $uri = "$BaseUrl$Path"
    $r = Invoke-WebRequest -Uri $uri -Method Post `
        -ContentType 'application/soap+xml; charset=utf-8' `
        -Body $Body `
        -UseBasicParsing `
        -TimeoutSec 30
    [pscustomobject]@{
        StatusCode = [int]$r.StatusCode
        ContentType = $r.Headers['Content-Type']
        Content = $r.Content
        Raw = $r
    }
}

function Get-FakescanElements {
    <#
    .SYNOPSIS
      WS-Scan GetScannerElements (description / configuration / status / default ticket).
    #>
    [CmdletBinding()]
    param(
        [string]$BaseUrl = $script:DefaultBaseUrl,
        [ValidateSet('ScannerDescription', 'ScannerConfiguration', 'ScannerStatus', 'DefaultScanTicket', 'All')]
        [string[]]$Name = @('ScannerDescription', 'ScannerConfiguration')
    )
    if ($Name -contains 'All') {
        $names = @(
            'wscn:ScannerDescription',
            'wscn:ScannerConfiguration',
            'wscn:ScannerStatus',
            'wscn:DefaultScanTicket'
        )
    } else {
        $names = $Name | ForEach-Object { "wscn:$_" }
    }
    $nameXml = ($names | ForEach-Object { "        <wscn:Name>$_</wscn:Name>" }) -join "`n"
    $body = @"
    <wscn:GetScannerElementsRequest>
      <wscn:RequestedElements>
$nameXml
      </wscn:RequestedElements>
    </wscn:GetScannerElementsRequest>
"@
    $env = New-FakescanSoapEnvelope -ActionLocalName 'GetScannerElements' -BodyXml $body
    Invoke-FakescanSoap -BaseUrl $BaseUrl -Body $env
}

function Test-FakescanSoap {
    <#
    .SYNOPSIS
      Quick SOAP reachability check (GetScannerElements -> ScannerDescription).
    #>
    [CmdletBinding()]
    param([string]$BaseUrl = $script:DefaultBaseUrl)
    $r = Get-FakescanElements -BaseUrl $BaseUrl -Name ScannerDescription
    $ok = $r.StatusCode -eq 200 -and $r.Content -match 'ScannerDescription'
    [pscustomobject]@{
        BaseUrl = (Resolve-FakescanBaseUrl $BaseUrl)
        Ok = [bool]$ok
        StatusCode = $r.StatusCode
        HasDescription = [bool]($r.Content -match 'ScannerDescription')
        Snippet = if ($r.Content -match '<wscn:ScannerName[^>]*>([^<]+)') { $Matches[1].Trim() } else { $null }
    }
}

function Find-FakescanDiscovery {
    <#
    .SYNOPSIS
      Send a WS-Discovery Probe for ScannerServiceType; collect ProbeMatch XAddrs.
    .NOTES
      WS-Discovery uses UDP 3702 (not classic SSDP :1900). On Docker Desktop bridge
      this often finds nothing; use a native binary with --advertise <LAN-IP>.
      Pass -UnicastHost to also Probe the host:3702 directly (helps when multicast
      is eaten by Windows dasHost).
    #>
    [CmdletBinding()]
    param(
        [int]$TimeoutMs = 3000,
        [string]$Types = $script:ScannerType,
        [string]$UnicastHost
    )
    $msgId = [guid]::NewGuid().ToString()
    $probe = @"
<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope"
  xmlns:wsa="http://schemas.xmlsoap.org/ws/2004/08/addressing"
  xmlns:wsd="http://schemas.xmlsoap.org/ws/2005/04/discovery"
  xmlns:wsdp="http://schemas.xmlsoap.org/ws/2006/02/devprof">
  <soap:Header>
    <wsa:To>urn:schemas-xmlsoap-org:ws:2005:04:discovery</wsa:To>
    <wsa:Action>http://schemas.xmlsoap.org/ws/2005/04/discovery/Probe</wsa:Action>
    <wsa:MessageID>urn:uuid:$msgId</wsa:MessageID>
  </soap:Header>
  <soap:Body>
    <wsd:Probe>
      <wsd:Types>wsdp:Device $Types</wsd:Types>
    </wsd:Probe>
  </soap:Body>
</soap:Envelope>
"@

    # Ephemeral port; ProbeMatch is typically unicast back to this socket.
    $udp = New-Object System.Net.Sockets.UdpClient 0
    try {
        $udp.Client.ReceiveTimeout = [Math]::Max(500, [int]($TimeoutMs / 4))
        $udp.Client.SetSocketOption(
            [System.Net.Sockets.SocketOptionLevel]::Socket,
            [System.Net.Sockets.SocketOptionName]::ReuseAddress,
            $true
        )
        try {
            $udp.JoinMulticastGroup($script:DiscoveryMcast)
        } catch {
            Write-Verbose "JoinMulticastGroup skipped: $_"
        }
        $bytes = [System.Text.Encoding]::UTF8.GetBytes($probe)
        $mcastEp = New-Object System.Net.IPEndPoint($script:DiscoveryMcast, $script:DiscoveryPort)
        [void]$udp.Send($bytes, $bytes.Length, $mcastEp)
        Start-Sleep -Milliseconds 150
        [void]$udp.Send($bytes, $bytes.Length, $mcastEp)
        if (-not [string]::IsNullOrWhiteSpace($UnicastHost)) {
            $ip = [System.Net.IPAddress]::Parse($UnicastHost)
            $uniEp = New-Object System.Net.IPEndPoint($ip, $script:DiscoveryPort)
            [void]$udp.Send($bytes, $bytes.Length, $uniEp)
            Start-Sleep -Milliseconds 100
            [void]$udp.Send($bytes, $bytes.Length, $uniEp)
        }

        $matches = New-Object System.Collections.Generic.List[object]
        $deadline = [datetime]::UtcNow.AddMilliseconds($TimeoutMs)
        while ([datetime]::UtcNow -lt $deadline) {
            try {
                $remote = New-Object System.Net.IPEndPoint([System.Net.IPAddress]::Any, 0)
                $recv = $udp.Receive([ref]$remote)
                $text = [System.Text.Encoding]::UTF8.GetString($recv)
                if ($text -notmatch 'ProbeMatch') { continue }
                $xaddrs = @()
                if ($text -match '<[^:>]*:?XAddrs>([^<]+)</') {
                    $xaddrs = $Matches[1].Trim() -split '\s+' | Where-Object { $_ }
                }
                $uuid = $null
                if ($text -match 'urn:uuid:([0-9a-fA-F-]{36})') {
                    $uuid = $Matches[1]
                }
                $matches.Add([pscustomobject]@{
                    Peer = $remote.ToString()
                    DeviceUuid = $uuid
                    XAddrs = $xaddrs
                    LooksLikeFakescan = (Test-FakescanLooksLike -XAddrs $xaddrs)
                    RawLength = $text.Length
                }) | Out-Null
            } catch [System.Net.Sockets.SocketException] {
                break
            }
        }
        return $matches.ToArray()
    } finally {
        $udp.Close()
    }
}

function Find-FakescanHello {
    <#
    .SYNOPSIS
      Listen on UDP 3702 for WS-Discovery Hello from fakescan (XAddrs contain :3920).

    .DESCRIPTION
      On Windows, dasHost often eats inbound Probes, so ProbeMatch tests fail even when
      fakescan is correctly announcing. Hearing Hello proves the UDP multicast send path.
    #>
    [CmdletBinding()]
    param(
        [int]$TimeoutMs = 8000
    )

    $udp = New-Object System.Net.Sockets.UdpClient
    try {
        $udp.Client.SetSocketOption(
            [System.Net.Sockets.SocketOptionLevel]::Socket,
            [System.Net.Sockets.SocketOptionName]::ReuseAddress,
            $true
        )
        $udp.Client.Bind((New-Object System.Net.IPEndPoint([System.Net.IPAddress]::Any, $script:DiscoveryPort)))
        try { $udp.JoinMulticastGroup($script:DiscoveryMcast) } catch { }
        $udp.Client.ReceiveTimeout = [Math]::Max(500, [int]($TimeoutMs / 5))

        $hits = New-Object System.Collections.Generic.List[object]
        $deadline = [datetime]::UtcNow.AddMilliseconds($TimeoutMs)
        while ([datetime]::UtcNow -lt $deadline) {
            try {
                $remote = New-Object System.Net.IPEndPoint([System.Net.IPAddress]::Any, 0)
                $recv = $udp.Receive([ref]$remote)
                $text = [System.Text.Encoding]::UTF8.GetString($recv)
                if ($text -notmatch 'Hello') { continue }
                $xaddrs = @()
                if ($text -match '<[^:>]*:?XAddrs>([^<]+)</') {
                    $xaddrs = $Matches[1].Trim() -split '\s+' | Where-Object { $_ }
                }
                $uuid = $null
                if ($text -match 'urn:uuid:([0-9a-fA-F-]{36})') { $uuid = $Matches[1] }
                $looks = (Test-FakescanLooksLike -XAddrs $xaddrs)
                if (-not $looks) { continue }
                $hits.Add([pscustomobject]@{
                    Peer = $remote.ToString()
                    DeviceUuid = $uuid
                    XAddrs = $xaddrs
                    LooksLikeFakescan = $looks
                    Kind = 'Hello'
                }) | Out-Null
                break
            } catch [System.Net.Sockets.SocketException] {
                continue
            }
        }
        return $hits.ToArray()
    } finally {
        $udp.Close()
    }
}

function Get-FakescanWiaDevices {
    <#
    .SYNOPSIS
      List Imaging / WIA devices Windows already knows (COM + PnP).

    .DESCRIPTION
      Distinguishes usable WIA enum presence from PnP-only residue:

        FakescanInWia     - listed by WIA.DeviceManager (apps can try Connect)
        FakescanInPnp     - Image-class PnP node looks like fakescan
        FakescanPnpOnly   - PnP yes, WIA enum no (Error/Disconnected or sticky)
        OnlineCapable     - fakescan PnP Status is OK/Started
        WiaConnectable    - same as FakescanInWia (enum only; Connect may still
                            return 0x80210005 with zero SOAP — sticky presence)
        StickyPresenceHint- OnlineCapable but not in WIA, or PnP Error/Disconnected

      FakescanPresent is true when either surface shows fakescan (legacy). Prefer
      FakescanInWia for "usable install" and StickyPresenceHint for rebind.

    .EXAMPLE
      $d = Get-FakescanWiaDevices
      if ($d.StickyPresenceHint) { Write-Host 'rebind: helper remove (or elevated) + person Add device' }
      if ($d.FakescanInWia) { 'usable in WIA enum' }
    #>
    [CmdletBinding()]
    param()

    $wia = @()
    try {
        $mgr = New-Object -ComObject WIA.DeviceManager
        foreach ($info in @($mgr.DeviceInfos)) {
            $props = @{}
            foreach ($p in @($info.Properties)) {
                try { $props[$p.Name] = $p.Value } catch { }
            }
            $name = $props['Name']
            if (-not $name) { $name = $props['Device Name'] }
            $wia += [pscustomobject]@{
                Source = 'WIA'
                DeviceId = $info.DeviceID
                Name = $name
                Port = $props['Port']
                LooksLikeFakescan = (Test-FakescanLooksLike -Name $name -Port $props['Port'])
            }
        }
    } catch {
        Write-Warning "WIA.DeviceManager unavailable: $_"
    }

    $pnp = @()
    try {
        $pnp = @(Get-PnpDevice -Class Image -ErrorAction SilentlyContinue | ForEach-Object {
            [pscustomobject]@{
                Source = 'PnP'
                DeviceId = $_.InstanceId
                Name = $_.FriendlyName
                Status = $_.Status
                LooksLikeFakescan = (Test-FakescanLooksLike -Name $_.FriendlyName -Port $_.InstanceId)
            }
        })
    } catch {
        Write-Warning "Get-PnpDevice failed: $_"
    }

    $wiaFake = @($wia | Where-Object { $_.LooksLikeFakescan })
    $pnpFake = @($pnp | Where-Object { $_.LooksLikeFakescan })
    $inWia = $wiaFake.Count -gt 0
    $inPnp = $pnpFake.Count -gt 0
    $onlineCapable = [bool](@($pnpFake | Where-Object {
        $_.Status -match '(?i)^(OK|Started)$'
    }).Count)
    $pnpBad = [bool](@($pnpFake | Where-Object {
        $_.Status -match '(?i)Error|Disconnected|Unknown|Degraded|No start'
    }).Count)
    $sticky = ($onlineCapable -and -not $inWia) -or ($inPnp -and $pnpBad -and -not $inWia)

    [pscustomobject]@{
        Wia = $wia
        Pnp = $pnp
        FakescanPresent = ($inWia -or $inPnp)
        FakescanInWia = $inWia
        FakescanInPnp = $inPnp
        FakescanPnpOnly = ($inPnp -and -not $inWia)
        OnlineCapable = $onlineCapable
        WiaConnectable = $inWia
        StickyPresenceHint = $sticky
    }
}

function Open-FakescanAddDevice {
    <#
    .SYNOPSIS
      Open Windows Settings → Printers & scanners for Add device.
    .DESCRIPTION
      Fires Invoke-LabAttention (banner + beep + balloon by default) before
      opening ms-settings:printers. Only called when added.pnp is false and
      no silent install path has already succeeded.
      -Loud:  also flash the parent terminal window (FlashWindowEx).
      -Quiet: banner only; no beep or balloon.
    .NOTES
      Opens only `ms-settings:printers` (do not also open Bluetooth — that steals
      focus). Silent CLI install is unsupported (PairAsync returns Failed on inbox
      WSDScan).
    #>
    [CmdletBinding()]
    param(
        [switch]$Loud,
        [switch]$Quiet
    )
    Invoke-LabAttention -Detail 'Add device next to "Fake WSD scanner (lite caps)".' `
        -Loud:$Loud -Quiet:$Quiet
    Start-Process 'ms-settings:printers'
}

function Get-FakescanSwdDevices {
    <#
    .SYNOPSIS
      List SoftWARe Device (SWD) PnP nodes related to WSD / DAFWSDPROVIDER / Image.
    .DESCRIPTION
      Installed inbox WSD scanners show as SWD\DAFWSDPROVIDER\urn:uuid:...\ScannerService
      under class Image (driver wsdscdrv.inf). Discovered-but-not-installed devices do
      NOT appear here - use Find-FakescanFunctionDiscovery for that stage.
      Never matches Brother by name alone when filtering with -FakescanOnly.
    #>
    [CmdletBinding()]
    param(
        [switch]$FakescanOnly
    )
    $rows = @()
    try {
        $rows = @(Get-PnpDevice -ErrorAction SilentlyContinue | Where-Object {
            $_.InstanceId -match '(?i)DAFWSDPROVIDER|\\WSD' -or
            ($_.Class -eq 'Image' -and $_.InstanceId -match '(?i)^SWD\\')
        } | ForEach-Object {
            [pscustomobject]@{
                Source = 'SWD'
                Status = $_.Status
                Class = $_.Class
                Name = $_.FriendlyName
                InstanceId = $_.InstanceId
                LooksLikeFakescan = (Test-FakescanLooksLike -Name $_.FriendlyName -Port $_.InstanceId)
            }
        })
    } catch {
        Write-Warning "Get-PnpDevice failed: $_"
    }
    if ($FakescanOnly) {
        return @($rows | Where-Object { $_.LooksLikeFakescan })
    }
    return $rows
}

function Initialize-FakescanFunctionDiscoveryTypes {
    if ('Fakescan.Fd.FdQuery' -as [type]) { return $true }
    $cs = Join-Path $PSScriptRoot 'Fakescan.FunctionDiscovery.cs'
    if (-not (Test-Path -LiteralPath $cs)) {
        Write-Warning "Missing $cs"
        return $false
    }
    try {
        Add-Type -Path $cs -ErrorAction Stop
        return $true
    } catch {
        # Already loaded in this process from a prior Import-Module -Force attempt.
        if ('Fakescan.Fd.FdQuery' -as [type]) { return $true }
        Write-Warning "Add-Type Function Discovery helpers failed: $_"
        return $false
    }
}

function Get-FakescanFunctionDiscovery {
    <#
    .SYNOPSIS
      Query Windows Function Discovery for WSD / network device instances.
    .DESCRIPTION
      Uses COM IFunctionDiscovery against:
        Provider\Microsoft.Networking.WSD
        Layered\Microsoft.Networking.Devices (sub WSD)
      WSD queries are asynchronous (Execute returns E_PENDING); this waits for
      FD_EVENTID_SEARCHCOMPLETE. A live Brother WSD scanner typically appears with
      Installable=true and DeviceCategory including Scanners. That does NOT mean
      fakescan is listed - run while fakescan Hellos are advancing.

      Discovered-but-not-installed: FD hit with Installable=true and no matching
      SWD\DAFWSDPROVIDER Image node. There is still no API to complete Add device.
    #>
    [CmdletBinding()]
    param(
        [int]$TimeoutMs = 8000,
        [string]$DirectedAddress,
        [string]$TypeConstraint,
        [switch]$WsdOnly
    )

    if (-not (Initialize-FakescanFunctionDiscoveryTypes)) {
        return [pscustomobject]@{
            Ok = $false
            Error = 'Function Discovery types unavailable'
            Hits = @()
            FakescanHits = @()
            Queries = @()
        }
    }

    $categories = @(
        @{ Cat = 'Provider\Microsoft.Networking.WSD'; Sub = '' },
        @{ Cat = 'Layered\Microsoft.Networking.Devices'; Sub = 'WSD' }
    )
    if ($WsdOnly) {
        $categories = @(@{ Cat = 'Provider\Microsoft.Networking.WSD'; Sub = '' })
    }

    $all = New-Object System.Collections.Generic.List[object]
    $queries = New-Object System.Collections.Generic.List[object]
    # PS 5.1 binds $null poorly to C# string parameters ("Argument types do not match").
    $typeArg = if ([string]::IsNullOrWhiteSpace($TypeConstraint)) { '' } else { [string]$TypeConstraint }
    $dirArg = if ([string]::IsNullOrWhiteSpace($DirectedAddress)) { '' } else { [string]$DirectedAddress }
    foreach ($c in $categories) {
        try {
            $catArg = [string]$c['Cat']
            $subArg = [string]$c['Sub']
            if ($null -eq $subArg) { $subArg = '' }
            if ($null -eq $catArg) { throw "missing category" }
            # Invoke via MethodInfo to avoid PS 5.1 binder quirks with empty strings.
            $mi = [Fakescan.Fd.FdQuery].GetMethod('QueryAsync')
            try {
                $r = $mi.Invoke($null, [object[]]@(
                    $catArg,
                    $subArg,
                    $typeArg,
                    $dirArg,
                    [int]$TimeoutMs
                ))
            } catch [System.Reflection.TargetInvocationException] {
                throw $_.Exception.InnerException
            }
            [void]$queries.Add([pscustomobject]@{
                Category = $catArg
                SubCategory = $subArg
                ExecuteHr = ('0x{0:X8}' -f [int]$r.ExecuteHr)
                LastError = $r.LastError
                HitCount = @($r.Hits).Count
            })
            foreach ($h in @($r.Hits)) {
                $xaddrs = New-Object System.Collections.Generic.List[string]
                if (-not [string]::IsNullOrWhiteSpace([string]$h.XAddrs)) {
                    foreach ($p in ([string]$h.XAddrs -split '\s+')) {
                        if ($p) { [void]$xaddrs.Add($p) }
                    }
                }
                if (-not [string]::IsNullOrWhiteSpace([string]$h.ServiceAddress)) {
                    foreach ($p in ([string]$h.ServiceAddress -split '\s+')) {
                        if ($p) { [void]$xaddrs.Add($p) }
                    }
                }
                $xarr = @($xaddrs.ToArray())
                $looks = Test-FakescanLooksLike -Name ([string]$h.FriendlyName) -Port ([string]$h.ModelName) -XAddrs $xarr
                if (-not $looks) {
                    $extra = @()
                    if ($h.GlobalIdentity) { $extra += [string]$h.GlobalIdentity }
                    if ($h.IpAddress) { $extra += [string]$h.IpAddress }
                    $looks = Test-FakescanLooksLike -Name ([string]$h.Id) -XAddrs $extra
                }
                [void]$all.Add([pscustomobject]@{
                    Category = [string]$h.Category
                    Source = [string]$h.Source
                    Id = [string]$h.Id
                    FriendlyName = [string]$h.FriendlyName
                    Manufacturer = [string]$h.Manufacturer
                    ModelName = [string]$h.ModelName
                    GlobalIdentity = [string]$h.GlobalIdentity
                    Types = [string]$h.Types
                    XAddrs = [string]$h.XAddrs
                    ServiceAddress = [string]$h.ServiceAddress
                    IpAddress = [string]$h.IpAddress
                    DeviceCategory = [string]$h.DeviceCategory
                    Installable = [string]$h.Installable
                    LooksLikeFakescan = [bool]$looks
                })
            }
        } catch {
            [void]$queries.Add([pscustomobject]@{
                Category = [string]$c['Cat']
                SubCategory = [string]$c['Sub']
                ExecuteHr = $null
                LastError = "$_"
                HitCount = 0
            })
        }
    }

    # Flatten List -> array once; avoid @($genericList) edge cases under StrictMode.
    $allArr = $all.ToArray()
    $queryArr = $queries.ToArray()
    $uniq = New-Object System.Collections.Generic.List[object]
    $seen = New-Object 'System.Collections.Generic.HashSet[string]'
    foreach ($row in $allArr) {
        $idPart = ''
        $namePart = ''
        if ($null -ne $row.Id) { $idPart = $row.Id.ToString() }
        if ($null -ne $row.FriendlyName) { $namePart = $row.FriendlyName.ToString() }
        $key = $idPart + '|' + $namePart
        if (-not $seen.Add($key)) { continue }
        [void]$uniq.Add($row)
    }
    $fakeArr = @($uniq.ToArray() | Where-Object { $_.LooksLikeFakescan -eq $true })
    $installableFake = $false
    foreach ($row in $fakeArr) {
        if ($row.Installable -eq 'true') { $installableFake = $true; break }
    }
    [pscustomobject]@{
        Ok = $true
        Error = $null
        Hits = $uniq.ToArray()
        FakescanHits = $fakeArr
        FakescanPresent = ($fakeArr.Count -gt 0)
        InstallableFakescan = $installableFake
        Queries = $queryArr
    }
}

function Find-FakescanFunctionDiscovery {
    <#
    .SYNOPSIS
      Return Function Discovery instances that look like fakescan (:3920 / name).
    .DESCRIPTION
      Wrapper over Get-FakescanFunctionDiscovery that returns only LooksLikeFakescan
      rows (name contains fakescan / Fake WSD scanner, or XAddrs/Id contain :3920).
      Brother WSD devices are not returned.
    #>
    [CmdletBinding()]
    param(
        [int]$TimeoutMs = 8000,
        [string]$DirectedAddress,
        [string]$TypeConstraint
    )
    $params = @{ TimeoutMs = $TimeoutMs }
    if (-not [string]::IsNullOrWhiteSpace($DirectedAddress)) {
        $params['DirectedAddress'] = $DirectedAddress
    }
    if (-not [string]::IsNullOrWhiteSpace($TypeConstraint)) {
        $params['TypeConstraint'] = $TypeConstraint
    }
    $r = Get-FakescanFunctionDiscovery @params
    return @($r.FakescanHits)
}

function Get-FakescanAssociationEndpoints {
    <#
    .SYNOPSIS
      Enumerate WinRT WSD AssociationEndpoint (AEP) devices (paired + unpaired).
    .DESCRIPTION
      Uses Windows.Devices.Enumeration with protocol id
      {782232aa-a2f9-4993-971b-aedc551346b0} (Web Services on Devices).
      This is another discovery surface next to Function Discovery. A fakescan
      row with CanPair=true is a candidate for experimental PairAsync; inbox
      Add device still requires a person when Pair fails or CanPair is false.
    #>
    [CmdletBinding()]
    param(
        [int]$TimeoutMs = 25000
    )

    try {
        Add-Type -AssemblyName System.Runtime.WindowsRuntime -ErrorAction Stop
    } catch {
        return [pscustomobject]@{
            Ok = $false
            Error = "System.Runtime.WindowsRuntime unavailable: $_"
            Hits = @()
            FakescanHits = @()
            FakescanPresent = $false
            CanPairFakescan = $false
            PairedFakescan = $false
        }
    }

    $asTaskGeneric = [System.WindowsRuntimeSystemExtensions].GetMethods() |
        Where-Object {
            $_.Name -eq 'AsTask' -and $_.IsGenericMethod -and $_.GetParameters().Count -eq 1
        } |
        Select-Object -First 1
    if (-not $asTaskGeneric) {
        return [pscustomobject]@{
            Ok = $false
            Error = 'AsTask extension method not found'
            Hits = @()
            FakescanHits = @()
            FakescanPresent = $false
            CanPairFakescan = $false
            PairedFakescan = $false
        }
    }

    $waitMs = [Math]::Max(1000, [int]$TimeoutMs)
    try {
        $null = [Windows.Devices.Enumeration.DeviceInformation, Windows, ContentType = WindowsRuntime]
        $aqs = 'System.Devices.Aep.ProtocolId:="{782232aa-a2f9-4993-971b-aedc551346b0}"'
        $kind = [Windows.Devices.Enumeration.DeviceInformationKind]::AssociationEndpoint
        $op = [Windows.Devices.Enumeration.DeviceInformation]::FindAllAsync($aqs, $null, $kind)
        $asTask = $asTaskGeneric.MakeGenericMethod([Windows.Devices.Enumeration.DeviceInformationCollection])
        $netTask = $asTask.Invoke($null, @($op))
        if (-not $netTask.Wait($waitMs)) {
            throw "WinRT FindAllAsync timed out after ${waitMs}ms"
        }
        if ($netTask.IsFaulted) { throw $netTask.Exception.InnerException }
        $devs = $netTask.Result
        $hits = @(foreach ($d in $devs) {
            $name = [string]$d.Name
            $id = [string]$d.Id
            $looks = Test-FakescanLooksLike -Name $name -Port $id
            [pscustomobject]@{
                Name = $name
                Id = $id
                IsPaired = [bool]$d.Pairing.IsPaired
                CanPair = [bool]$d.Pairing.CanPair
                LooksLikeFakescan = [bool]$looks
            }
        })
        $fake = @($hits | Where-Object { $_.LooksLikeFakescan })
        $canPair = @($fake | Where-Object { $_.CanPair -and -not $_.IsPaired })
        $paired = @($fake | Where-Object { $_.IsPaired })
        [pscustomobject]@{
            Ok = $true
            Error = $null
            Hits = $hits
            FakescanHits = $fake
            FakescanPresent = ($fake.Count -gt 0)
            CanPairFakescan = ($canPair.Count -gt 0)
            PairedFakescan = ($paired.Count -gt 0)
        }
    } catch {
        [pscustomobject]@{
            Ok = $false
            Error = "$_"
            Hits = @()
            FakescanHits = @()
            FakescanPresent = $false
            CanPairFakescan = $false
            PairedFakescan = $false
        }
    }
}

function Start-FakescanElevatedShell {
    <#
    .SYNOPSIS
      Open an elevated PowerShell window with Fakescan.psm1 imported.
    .DESCRIPTION
      Interactive UAC shell for Remove-FakescanScanner / rebind / associate /
      pnputil on already-installed Imaging devices. Prefer Start-FakescanElevatedHelper
      when a WIA lab suite should stay unelevated and only need Fake remove.
      Does NOT unlock silent WSD install - Add device still needs a person.
      Do not run fakescan.exe elevated.
    #>
    [CmdletBinding()]
    param(
        [string]$Command = 'Write-Host "Elevated fakescan shell. Try: Get-FakescanWiaDevices; Remove-FakescanScanner -WhatIf. Prefer: fakescan.ps1 helper for session remove."'
    )
    $mod = Join-Path $PSScriptRoot 'Fakescan.psm1'
    $escapedMod = $mod.Replace("'", "''")
    $escapedCmd = $Command.Replace("'", "''")
    $arg = "-NoExit -NoProfile -ExecutionPolicy Bypass -Command `"Import-Module '$escapedMod' -Force; Show-FakescanBanner -Role 'elevated shell — Fake Imaging remove / rebind / associate'; $escapedCmd`""
    Start-Process -FilePath 'powershell.exe' -Verb RunAs -ArgumentList $arg | Out-Null
    Write-Host 'Launched elevated PowerShell (UAC). For Fake remove only, prefer: .\scripts\fakescan.ps1 helper. Not for silent Add device; do not elevate fakescan.exe.'
}

function Get-FakescanElevatedHelperDir {
    Join-Path $env:TEMP 'fakescan-elev-helper'
}

function Test-FakescanProcessElevated {
    ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
        [Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Get-FakescanElevatedHelperSession {
    $path = Join-Path (Get-FakescanElevatedHelperDir) 'session.json'
    if (-not (Test-Path -LiteralPath $path)) { return $null }
    try {
        $session = Get-Content -LiteralPath $path -Raw -ErrorAction Stop | ConvertFrom-Json
    } catch {
        return $null
    }
    # ConvertFrom-Json yields Int64; reject missing/zero before Get-Process.
    $helperPid = 0
    try { $helperPid = [int]$session.process_id } catch { $helperPid = 0 }
    if ($helperPid -le 0) {
        Remove-Item -LiteralPath $path -ErrorAction SilentlyContinue
        return $null
    }
    $proc = Get-Process -Id $helperPid -ErrorAction SilentlyContinue
    if (-not $proc) {
        Remove-Item -LiteralPath $path -ErrorAction SilentlyContinue
        return $null
    }
    $session
}

function Watch-FakescanElevatedHelper {
    <#
    .SYNOPSIS
      Session helper for Fake device removal / AEP Unpair. Accepts remove, whatif, unpair, bounce-fd, and stop.
    .DESCRIPTION
      Started elevated by Start-FakescanElevatedHelper. The command file may
      name only those commands. Instance ids are never read from it.
      remove still goes through Remove-FakescanScanner. bounce-fd restarts
      fdPHost then DeviceAssociationService (never dasHost). The process
      exits on stop or after IdleSeconds with no request.
    #>
    [CmdletBinding()]
    param(
        [int]$IdleSeconds = 1800,
        [switch]$AllowUnelevated
    )
    if ($IdleSeconds -lt 1) { $IdleSeconds = 1800 }
    $elevated = Test-FakescanProcessElevated
    if (-not $elevated -and -not $AllowUnelevated) {
        throw 'Watch-FakescanElevatedHelper must run elevated. Start it with: .\scripts\fakescan.ps1 helper'
    }
    $dir = Get-FakescanElevatedHelperDir
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    $sessionPath = Join-Path $dir 'session.json'
    $requestPath = Join-Path $dir 'request.json'
    $replyPath = Join-Path $dir 'reply.json'
    # Another live helper already owns the slot (concurrent Start-Process -Verb RunAs).
    $prior = Get-FakescanElevatedHelperSession
    if ($prior -and [int]$prior.process_id -ne $PID) {
        throw "elevated helper already running (pid $($prior.process_id)); this Watch exits"
    }
    # Drop leftovers from a dead helper / timed-out Invoke so we never act on a stale remove.
    Remove-Item -LiteralPath $requestPath, $replyPath -ErrorAction SilentlyContinue
    Show-FakescanBanner -Role ("elevated helper pid={0} — Fake remove only; idle {1}s" -f $PID, $IdleSeconds)
    Write-Host 'Waiting for: .\scripts\fakescan.ps1 helper remove|whatif|unpair|bounce-fd|stop'
    Write-Host ''
    # process_id, not pid: keeps session JSON clear of the automatic $PID name.
    @{
        process_id = $PID
        elevated = $elevated
        started = [datetime]::UtcNow.ToString('o')
        idle_seconds = $IdleSeconds
    } | ConvertTo-Json | Set-Content -LiteralPath $sessionPath -Encoding utf8
    $idleSince = [datetime]::UtcNow
    try {
        while ($true) {
            if (([datetime]::UtcNow - $idleSince).TotalSeconds -ge $IdleSeconds) { break }
            if (-not (Test-Path -LiteralPath $requestPath)) {
                Start-Sleep -Milliseconds 250
                continue
            }
            $idleSince = [datetime]::UtcNow
            $raw = Get-Content -LiteralPath $requestPath -Raw -ErrorAction SilentlyContinue
            Remove-Item -LiteralPath $requestPath -ErrorAction SilentlyContinue
            $cmd = ''
            $id = ''
            try {
                if ([string]::IsNullOrWhiteSpace($raw)) {
                    throw 'empty request'
                }
                $msg = $raw | ConvertFrom-Json
                $cmd = [string]$msg.cmd
                $id = [string]$msg.id
                if ($cmd -notin @('remove', 'whatif', 'unpair', 'bounce-fd', 'stop')) {
                    throw "unsupported command: $cmd"
                }
                $result = $null
                if ($cmd -eq 'remove') {
                    if (-not $elevated) { throw 'helper is not elevated' }
                    $result = Remove-FakescanScanner
                } elseif ($cmd -eq 'whatif') {
                    # Explicit -WhatIf (not Preference); return value still carries Matched.
                    $result = Remove-FakescanScanner -WhatIf
                } elseif ($cmd -eq 'unpair') {
                    # Stale AEP IsPaired after pnputil remove; UnpairAsync often AccessDenied unelevated.
                    if (-not $elevated) { throw 'helper is not elevated' }
                    $result = Disconnect-FakescanAssociationEndpoint
                } elseif ($cmd -eq 'bounce-fd') {
                    if (-not $elevated) { throw 'helper is not elevated' }
                    $result = Restart-FakescanDiscoveryServices
                }
                @{ id = $id; cmd = $cmd; ok = $true; error = $null; result = $result } |
                    ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $replyPath -Encoding utf8
                if ($cmd -eq 'stop') {
                    # Handshake: the reply is on disk; give the caller a moment to read and remove it
                    # before the cleanup below deletes the session. Bounded so we never hang here.
                    $ackBy = [datetime]::UtcNow.AddSeconds(5)
                    while ((Test-Path -LiteralPath $replyPath) -and [datetime]::UtcNow -lt $ackBy) {
                        Start-Sleep -Milliseconds 100
                    }
                    break
                }
            } catch {
                @{ id = $id; cmd = $cmd; ok = $false; error = "$_"; result = $null } |
                    ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $replyPath -Encoding utf8
            }
        }
    } finally {
        # Leave no request behind for the next helper to execute.
        Remove-Item -LiteralPath $sessionPath, $requestPath, $replyPath -ErrorAction SilentlyContinue
    }
}

function Start-FakescanElevatedHelper {
    <#
    .SYNOPSIS
      One UAC prompt, then a helper that can remove Fake devices until it is stopped.
    .DESCRIPTION
      Preferred for WIA lab teardown: the suite stays unelevated; only Imaging
      remove/unpair/bounce-fd is elevated. Request file accepts
      remove|whatif|unpair|bounce-fd|stop only (never instance ids). Session
      field is process_id. Idle default 1800s.
      SOAP/local matrix needs no helper. Add device is still a person.
      Do not run fakescan.exe elevated.
    #>
    [CmdletBinding()]
    param(
        [int]$IdleSeconds = 1800
    )
    $existing = Get-FakescanElevatedHelperSession
    if ($existing) {
        Write-Host "Elevated helper already running (pid $($existing.process_id))."
        return $existing
    }
    $mod = Join-Path $PSScriptRoot 'Fakescan.psm1'
    $escapedMod = $mod.Replace("'", "''")
    $arg = "-NoProfile -ExecutionPolicy Bypass -Command `"Import-Module '$escapedMod' -Force; Watch-FakescanElevatedHelper -IdleSeconds $IdleSeconds`""
    try {
        Start-Process -FilePath 'powershell.exe' -Verb RunAs -ArgumentList $arg | Out-Null
    } catch {
        throw "UAC was not accepted, so no elevated helper is running. $_"
    }
    $deadline = [datetime]::UtcNow.AddSeconds(20)
    while ([datetime]::UtcNow -lt $deadline) {
        $session = Get-FakescanElevatedHelperSession
        if ($session) {
            Write-Host "Elevated helper ready (pid $($session.process_id)). It removes Fake devices only. Stop it with: .\scripts\fakescan.ps1 helper stop"
            return $session
        }
        Start-Sleep -Milliseconds 250
    }
    throw 'Elevated helper did not start. Accept the UAC prompt, then run: .\scripts\fakescan.ps1 helper'
}

function Invoke-FakescanElevatedHelper {
    <#
    .SYNOPSIS
      Ask the session helper to remove Fake devices, preview that, bounce FD, or exit.
    .DESCRIPTION
      Does not elevate. If the helper is not running, this fails and names
      the start command. The command is one of remove, whatif, unpair,
      bounce-fd, stop.
    #>
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [ValidateSet('remove', 'whatif', 'unpair', 'bounce-fd', 'stop')]
        [string]$Cmd,
        [int]$TimeoutSeconds = 60
    )
    $session = Get-FakescanElevatedHelperSession
    if (-not $session) {
        throw 'elevated helper is not running. Start it with: .\scripts\fakescan.ps1 helper'
    }
    $dir = Get-FakescanElevatedHelperDir
    $requestPath = Join-Path $dir 'request.json'
    $replyPath = Join-Path $dir 'reply.json'
    if (Test-Path -LiteralPath $requestPath) {
        throw 'elevated helper already has a request waiting'
    }
    Remove-Item -LiteralPath $replyPath -ErrorAction SilentlyContinue
    $id = [guid]::NewGuid().ToString('n')
    $tmp = Join-Path $dir "request.$id.json"
    @{ id = $id; cmd = $Cmd } | ConvertTo-Json | Set-Content -LiteralPath $tmp -Encoding utf8
    # No -Force: destination must be absent (checked above). Overwrite would drop another client's request.
    try {
        Move-Item -LiteralPath $tmp -Destination $requestPath -ErrorAction Stop
    } catch {
        Remove-Item -LiteralPath $tmp -ErrorAction SilentlyContinue
        throw "elevated helper request publish failed (is another request racing?): $_"
    }
    $deadline = [datetime]::UtcNow.AddSeconds($TimeoutSeconds)
    try {
        while ([datetime]::UtcNow -lt $deadline) {
            if (Test-Path -LiteralPath $replyPath) {
                $reply = Get-Content -LiteralPath $replyPath -Raw | ConvertFrom-Json
                Remove-Item -LiteralPath $replyPath -ErrorAction SilentlyContinue
                if ([string]$reply.id -ne $id) { throw "elevated helper replied to another request ($($reply.id))" }
                if (-not $reply.ok) { throw "elevated helper ${Cmd} failed: $($reply.error)" }
                return $reply
            }
            if (-not (Get-FakescanElevatedHelperSession)) {
                # The reply may have landed between the check above and the helper exiting.
                if (Test-Path -LiteralPath $replyPath) { continue }
                if ($Cmd -eq 'stop') {
                    # The request was consumed and the helper is gone: that is a successful stop.
                    return [pscustomobject]@{ id = $id; cmd = 'stop'; ok = $true; error = $null; result = 'helper exited' }
                }
                throw 'elevated helper exited before answering'
            }
            Start-Sleep -Milliseconds 200
        }
    } finally {
        # If we timed out or the helper died while our request still sat on disk, reclaim it
        # so the next helper start does not execute a stale remove/whatif/stop.
        if (Test-Path -LiteralPath $requestPath) {
            try {
                $leftover = Get-Content -LiteralPath $requestPath -Raw -ErrorAction Stop | ConvertFrom-Json
                if ([string]$leftover.id -eq $id) {
                    Remove-Item -LiteralPath $requestPath -ErrorAction SilentlyContinue
                }
            } catch {
                # Helper may have the file open mid-read; ignore.
            }
        }
        Remove-Item -LiteralPath $tmp -ErrorAction SilentlyContinue
    }
    throw "elevated helper did not answer $Cmd within ${TimeoutSeconds}s"
}

function Connect-FakescanAssociationEndpoint {
    <#
    .SYNOPSIS
      Experimental WinRT PairAsync on a fakescan WSD AssociationEndpoint.
    .DESCRIPTION
      Lab note (2026-10-11): after Hellos + metadata poll, WinRT can list
      "Fake WSD scanner" with CanPair=true. PairAsync still returns Failed on
      inbox WSDScan without a person completing Add device. Use this to probe;
      fall back to Install-FakescanScanner (Settings UI).

      Named Connect- (approved verb); CLI alias remains `fakescan.ps1 pair`.
    #>
    [CmdletBinding()]
    param(
        [int]$TimeoutMs = 25000,
        [int]$PairTimeoutMs = 120000
    )

    $aep = Get-FakescanAssociationEndpoints -TimeoutMs $TimeoutMs
    if (-not $aep.Ok) {
        return [pscustomobject]@{
            Ok = $false
            Error = $aep.Error
            Status = $null
            DeviceId = $null
            Name = $null
            InstalledAfter = $false
        }
    }
    $cand = @($aep.FakescanHits | Where-Object { $_.CanPair -and -not $_.IsPaired } | Select-Object -First 1)
    if (-not $cand) {
        $any = @($aep.FakescanHits | Select-Object -First 1)
        return [pscustomobject]@{
            Ok = $false
            Error = if ($any) {
                "fakescan AEP present but not pairable (IsPaired=$($any.IsPaired) CanPair=$($any.CanPair))"
            } else {
                'no fakescan AssociationEndpoint (wait for Hellos / try fresh --device-uuid)'
            }
            Status = $null
            DeviceId = if ($any) { $any.Id } else { $null }
            Name = if ($any) { $any.Name } else { $null }
            InstalledAfter = $false
        }
    }

    try {
        Add-Type -AssemblyName System.Runtime.WindowsRuntime -ErrorAction Stop
        $asTaskGeneric = [System.WindowsRuntimeSystemExtensions].GetMethods() |
            Where-Object {
                $_.Name -eq 'AsTask' -and $_.IsGenericMethod -and $_.GetParameters().Count -eq 1
            } |
            Select-Object -First 1
        $null = [Windows.Devices.Enumeration.DeviceInformation, Windows, ContentType = WindowsRuntime]
        $aqs = 'System.Devices.Aep.ProtocolId:="{782232aa-a2f9-4993-971b-aedc551346b0}"'
        $kind = [Windows.Devices.Enumeration.DeviceInformationKind]::AssociationEndpoint
        $findOp = [Windows.Devices.Enumeration.DeviceInformation]::FindAllAsync($aqs, $null, $kind)
        $findTask = $asTaskGeneric.MakeGenericMethod(
            [Windows.Devices.Enumeration.DeviceInformationCollection]
        ).Invoke($null, @($findOp))
        if (-not $findTask.Wait([Math]::Max(1000, $TimeoutMs))) {
            throw "FindAllAsync timed out"
        }
        if ($findTask.IsFaulted) { throw $findTask.Exception.InnerException }
        $devs = $findTask.Result
        $target = $null
        foreach ($d in $devs) {
            if ([string]$d.Id -eq [string]$cand.Id) { $target = $d; break }
        }
        if (-not $target) {
            throw "AEP id vanished before PairAsync: $($cand.Id)"
        }
        $pl = [Windows.Devices.Enumeration.DevicePairingProtectionLevel]::None
        $pairOp = $target.Pairing.PairAsync($pl)
        $pairTask = $asTaskGeneric.MakeGenericMethod(
            [Windows.Devices.Enumeration.DevicePairingResult]
        ).Invoke($null, @($pairOp))
        if (-not $pairTask.Wait([Math]::Max(1000, $PairTimeoutMs))) {
            throw "PairAsync timed out"
        }
        if ($pairTask.IsFaulted) { throw $pairTask.Exception.InnerException }
        $result = $pairTask.Result
        $status = [string]$result.Status
        Start-Sleep -Seconds 2
        $wia = Get-FakescanWiaDevices
        [pscustomobject]@{
            Ok = ($status -eq 'Paired')
            Error = if ($status -eq 'Paired') { $null } else { "PairAsync status=$status (Add device UI still required)" }
            Status = $status
            DeviceId = [string]$cand.Id
            Name = [string]$cand.Name
            InstalledAfter = [bool]$wia.FakescanInWia
        }
    } catch {
        [pscustomobject]@{
            Ok = $false
            Error = "$_"
            Status = $null
            DeviceId = [string]$cand.Id
            Name = [string]$cand.Name
            InstalledAfter = $false
        }
    }
}

function Invoke-FakescanPnpxAssociate {
    <#
    .SYNOPSIS
      Programmatic PnP-X association via IPNPXDeviceAssociation::Associate.
    .DESCRIPTION
      Uses COM IPNPXDeviceAssociation (pnpxassoc.h) on FD WSD provider instances that
      look like fakescan. This is the documented non-UI "Add device" equivalent when
      Function Discovery lists the device.

      REQUIRES ELEVATION — returns AssociateHr=0x80070005 (E_ACCESSDENIED) without it.

      Flow:
        1. Queries Provider\Microsoft.Networking.WSD via IFunctionDiscovery async.
        2. Finds instances whose FriendlyName / XAddrs match fakescan.
        3. For each, calls IFunctionInstance::QueryService(SID_PNPXDeviceAssociation)
           to get IPNPXDeviceAssociation, then calls Associate(hwnd=0, callback).
        4. Waits for IPNPXAssociationCallback::OnAssociated or times out.
        5. Returns one result per instance found.

      If the device is already installed, QueryService may succeed but Associate
      may return E_ALREADY_EXISTS or the callback Hr != 0. If FD has no fakescan
      instance at all, the result array is empty — run 'discoverable' first.

      Lab sequence (after Remove-FakescanScanner with fakescan still running):
        1. .\scripts\fakescan.ps1 discoverable      # confirm aep-pairable / fd hit
        2. Start elevated shell: .\scripts\fakescan.ps1 elevated
        3. In elevated shell: Invoke-FakescanPnpxAssociate
        4. .\scripts\fakescan.ps1 wia               # verify FakescanInWia=True
    #>
    [CmdletBinding()]
    param(
        [int]$QueryTimeoutMs = 10000,
        [int]$AssocTimeoutMs = 60000
    )

    if (-not (Initialize-FakescanFunctionDiscoveryTypes)) {
        return [pscustomobject]@{
            Ok = $false
            Error = 'Function Discovery COM types unavailable'
            Results = @()
            FakescanInWiaAfter = $null
        }
    }

    $elevated = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
        [Security.Principal.WindowsBuiltInRole]::Administrator)
    if (-not $elevated) {
        Write-Warning 'Invoke-FakescanPnpxAssociate: NOT running elevated. Associate will likely return 0x80070005 (E_ACCESSDENIED). Run from an elevated shell.'
    }

    Write-Host ("FD query for fakescan instances (timeout {0}ms) ..." -f $QueryTimeoutMs)
    $mi = [Fakescan.Fd.FdAssoc].GetMethod('AssociateFakescan')
    $rawResults = $mi.Invoke($null, [object[]]@([int]$QueryTimeoutMs, [int]$AssocTimeoutMs))
    $results = @($rawResults | ForEach-Object {
        [pscustomobject]@{
            FriendlyName    = [string]$_.FriendlyName
            InstanceId      = [string]$_.InstanceId
            QueryServiceHr  = ('0x{0:X8}' -f [int]$_.QueryServiceHr)
            AssociateHr     = if ($_.AssociateHr -ne [int]::MinValue) { '0x{0:X8}' -f [int]$_.AssociateHr } else { 'n/a' }
            CallbackFired   = [bool]$_.CallbackFired
            Succeeded       = [bool]$_.Succeeded
            Error           = [string]$_.Error
        }
    })

    $any = $results.Count -gt 0
    $succeeded = @($results | Where-Object { $_.Succeeded }).Count -gt 0

    if (-not $any) {
        Write-Warning @'
No fakescan-like instance found in FD WSD provider.
  - Run: .\scripts\fakescan.ps1 discoverable   (need aep-pairable or fd-installable)
  - FD may not have discovered the device yet; retry after Hello burst.
  - If already installed, use: .\scripts\fakescan.ps1 wia
'@
    } else {
        $results | Format-List
        if ($succeeded) {
            Write-Host 'Associate succeeded. Verifying WIA ...'
            Start-Sleep -Seconds 3
        } else {
            Write-Warning 'Associate did not succeed; person Add device may still be required.'
        }
    }

    $wia = $null
    try { $wia = Get-FakescanWiaDevices } catch { }
    $inWia = if ($wia) { [bool]$wia.FakescanInWia } else { $null }

    [pscustomobject]@{
        Ok               = $succeeded
        Error            = if ($succeeded) { $null } else { 'no Associate succeeded; see Results' }
        Results          = $results
        FakescanInWiaAfter = $inWia
        Elevated         = $elevated
    }
}

function Disconnect-FakescanAssociationEndpoint {
    <#
    .SYNOPSIS
      WinRT UnpairAsync on a paired fakescan WSD AssociationEndpoint.
    .DESCRIPTION
      After `pnputil /remove-device` the Imaging node can be gone while AEP
      IsPaired=true. Settings then hides Fake under "Add a printer or scanner".
      Unpair clears the stale association so Add device can list it again.
    #>
    [CmdletBinding()]
    param(
        [int]$TimeoutMs = 25000,
        [int]$UnpairTimeoutMs = 120000
    )

    $aep = Get-FakescanAssociationEndpoints -TimeoutMs $TimeoutMs
    if (-not $aep.Ok) {
        return [pscustomobject]@{
            Ok = $false
            Error = $aep.Error
            Status = $null
            DeviceId = $null
            Name = $null
            IsPairedAfter = $null
        }
    }
    # Prefer scanner AEP (name / …f2); never Unpair the fakeprint …f3 row.
    $cand = @($aep.FakescanHits | Where-Object {
        $_.IsPaired -and (
            $_.Name -match '(?i)scanner' -or
            $_.Id -match '(?i)00000000-0000-4000-8000-0000000000f2'
        )
    } | Select-Object -First 1)
    if (-not $cand) {
        $cand = @($aep.FakescanHits | Where-Object {
            $_.IsPaired -and $_.Id -notmatch '(?i)00000000-0000-4000-8000-0000000000f3'
        } | Select-Object -First 1)
    }
    if (-not $cand) {
        return [pscustomobject]@{
            Ok = $true
            Error = $null
            Status = 'AlreadyUnpaired'
            DeviceId = $null
            Name = $null
            IsPairedAfter = $false
        }
    }

    try {
        Add-Type -AssemblyName System.Runtime.WindowsRuntime -ErrorAction Stop
        $asTaskGeneric = [System.WindowsRuntimeSystemExtensions].GetMethods() |
            Where-Object {
                $_.Name -eq 'AsTask' -and $_.IsGenericMethod -and $_.GetParameters().Count -eq 1
            } |
            Select-Object -First 1
        $null = [Windows.Devices.Enumeration.DeviceInformation, Windows, ContentType = WindowsRuntime]
        $aqs = 'System.Devices.Aep.ProtocolId:="{782232aa-a2f9-4993-971b-aedc551346b0}"'
        $kind = [Windows.Devices.Enumeration.DeviceInformationKind]::AssociationEndpoint
        $findOp = [Windows.Devices.Enumeration.DeviceInformation]::FindAllAsync($aqs, $null, $kind)
        $findTask = $asTaskGeneric.MakeGenericMethod(
            [Windows.Devices.Enumeration.DeviceInformationCollection]
        ).Invoke($null, @($findOp))
        if (-not $findTask.Wait([Math]::Max(1000, $TimeoutMs))) {
            throw 'FindAllAsync timed out'
        }
        if ($findTask.IsFaulted) { throw $findTask.Exception.InnerException }
        $target = $null
        foreach ($d in $findTask.Result) {
            if ([string]$d.Id -eq [string]$cand.Id) { $target = $d; break }
        }
        if (-not $target) {
            throw "AEP id vanished before UnpairAsync: $($cand.Id)"
        }
        $unpairOp = $target.Pairing.UnpairAsync()
        $unpairTask = $asTaskGeneric.MakeGenericMethod(
            [Windows.Devices.Enumeration.DeviceUnpairingResult]
        ).Invoke($null, @($unpairOp))
        if (-not $unpairTask.Wait([Math]::Max(1000, $UnpairTimeoutMs))) {
            throw 'UnpairAsync timed out'
        }
        if ($unpairTask.IsFaulted) { throw $unpairTask.Exception.InnerException }
        $status = [string]$unpairTask.Result.Status
        Start-Sleep -Seconds 2
        $after = Get-FakescanAssociationEndpoints -TimeoutMs $TimeoutMs
        $still = @($after.FakescanHits | Where-Object { $_.Id -eq $cand.Id -and $_.IsPaired })
        [pscustomobject]@{
            Ok = ($status -eq 'Unpaired' -or $still.Count -eq 0)
            Error = if ($status -eq 'Unpaired' -or $still.Count -eq 0) { $null } else { "UnpairAsync status=$status" }
            Status = $status
            DeviceId = [string]$cand.Id
            Name = [string]$cand.Name
            IsPairedAfter = ($still.Count -gt 0)
        }
    } catch {
        [pscustomobject]@{
            Ok = $false
            Error = "$_"
            Status = $null
            DeviceId = [string]$cand.Id
            Name = [string]$cand.Name
            IsPairedAfter = $true
        }
    }
}

function Install-FakescanScanner {
    <#
    .SYNOPSIS
      Assist installing fakescan as a Windows Imaging device (WSDScan).

    .DESCRIPTION
      Windows has no supported silent "add this WSD scanner URL" API for inbox WSDScan.
      This command:
        1) Verifies HTTP + SOAP against -BaseUrl
        2) Optionally probes WS-Discovery
        3) Opens the Add device UI
        4) Optionally waits until a matching WIA device appears

      Prefer running fakescan with a LAN --advertise address (or Linux --network host)
      so Windows can discover it. Docker Desktop bridge alone is usually not enough
      for install - only for SOAP testing.

    .PARAMETER BaseUrl
      Control/SOAP base, e.g. http://192.168.1.10:3920

    .PARAMETER WaitSeconds
      After opening Add device, poll WIA for a fakescan-like device.
    #>
    [CmdletBinding()]
    param(
        [string]$BaseUrl = $script:DefaultBaseUrl,
        [int]$WaitSeconds = 0,
        [switch]$SkipUi,
        [switch]$ProbeDiscovery,
        # Passed through to Open-FakescanAddDevice / Invoke-LabAttention.
        [switch]$Loud,
        [switch]$Quiet
    )

    $BaseUrl = Resolve-FakescanBaseUrl $BaseUrl
    Write-Host "Checking control plane at $BaseUrl ..."
    $health = Get-FakescanHealth -BaseUrl $BaseUrl
    if (-not $health.Ok) {
        throw "fakescan /health failed at $BaseUrl (got '$($health.Body)')"
    }
    $status = Get-FakescanStatus -BaseUrl $BaseUrl
    Write-Host "Profile=$($status.profile_id) connection=$($status.connection)"

    Write-Host 'Checking SOAP GetScannerElements ...'
    $soap = Test-FakescanSoap -BaseUrl $BaseUrl
    if (-not $soap.Ok) {
        throw "SOAP GetScannerElements failed (HTTP $($soap.StatusCode))"
    }
    Write-Host "SOAP OK - ScannerName='$($soap.Snippet)'"

    $discoveries = @()
    if ($ProbeDiscovery) {
        Write-Host 'WS-Discovery (Hello / ProbeMatch / hellos counter) ...'
        $disc = Test-FakescanDiscoverable -BaseUrl $BaseUrl -DiscoveryTimeoutMs 4000
        $discoveries = @($disc.UdpMatches)
        if ($disc.UdpProbeMatch) {
            Write-Host "UDP announce OK (verdict=$($disc.Verdict))"
            $discoveries | Format-Table -AutoSize | Out-Host
            $disc.Notes | ForEach-Object { Write-Host "  $_" }
        } else {
            Write-Warning @'
No WS-Discovery Hello/ProbeMatch visible. Common causes:
  - fakescan behind Docker Desktop bridge (multicast not on LAN)
  - firewall blocking UDP 3702
  - need: native binary --advertise <LAN-IP> (or Linux --network host)
SOAP still works for protocol tests; Windows Add device needs Hello on the LAN.
'@
        }
    }

    $before = Get-FakescanWiaDevices
    $fdBefore = $null
    try {
        Write-Host 'Function Discovery (WSD provider) ...'
        $fdBefore = Get-FakescanFunctionDiscovery -TimeoutMs 6000
        if ($fdBefore.FakescanPresent) {
            $inst = if ($fdBefore.InstallableFakescan) { 'Installable=true' } else { 'Installable!=true' }
            Write-Host "FD sees fakescan ($inst). Add device may list it; waiting still needs a person click."
            $fdBefore.FakescanHits | Format-Table FriendlyName, ModelName, XAddrs, Installable, DeviceCategory -AutoSize | Out-Host
        } else {
            Write-Warning @'
Function Discovery did not return a fakescan-like instance.
  - SOAP Get / Hello can still happen (dasHost metadata poll) without an FD installable hit.
  - Add device will usually stay empty until FD lists it with Installable=true.
  - Compare: Get-FakescanFunctionDiscovery  (Brother often appears; fakescan may not).
  - Same-host advertise (this PC's own LAN IP) often stays FD-invisible; try another machine on the LAN.
'@
            if ($fdBefore.Hits.Count -gt 0) {
                Write-Host ("FD other WSD hits: {0}" -f $fdBefore.Hits.Count)
                $fdBefore.Hits | Select-Object -First 5 FriendlyName, XAddrs, Installable, DeviceCategory |
                    Format-Table -AutoSize | Out-Host
            }
        }
    } catch {
        Write-Warning "Function Discovery query failed: $_"
    }

    $aepBefore = $null
    try {
        Write-Host 'WinRT WSD AssociationEndpoints ...'
        $aepBefore = Get-FakescanAssociationEndpoints
        if ($aepBefore.Ok -and $aepBefore.FakescanPresent) {
            Write-Host ("AEP sees fakescan (CanPair={0})." -f $aepBefore.CanPairFakescan)
            $aepBefore.FakescanHits | Format-Table Name, Id, IsPaired, CanPair -AutoSize | Out-Host
        } elseif ($aepBefore.Ok) {
            Write-Host ("AEP WSD count={0}; no fakescan-like row." -f $aepBefore.Hits.Count)
        } else {
            Write-Warning "AEP enumerate failed: $($aepBefore.Error)"
        }
    } catch {
        Write-Warning "AEP enumerate failed: $_"
    }

    $pairAttempt = $null
    if ($aepBefore -and $aepBefore.CanPairFakescan) {
        Write-Host 'Trying experimental WinRT PairAsync (often Failed; Add device still needed) ...'
        $pairAttempt = Connect-FakescanAssociationEndpoint
        Write-Host ("Pair status={0} Ok={1} InstalledAfter={2}" -f $pairAttempt.Status, $pairAttempt.Ok, $pairAttempt.InstalledAfter)
        if ($pairAttempt.Error) { Write-Host "  $($pairAttempt.Error)" }
        if ($pairAttempt.InstalledAfter) {
            $before = Get-FakescanWiaDevices
        }
    }

    if (-not $SkipUi -and -not ($pairAttempt -and $pairAttempt.InstalledAfter)) {
        # Attention fires inside Open-FakescanAddDevice (added.pnp is false
        # at this point; silent install is unsupported for inbox WSDScan).
        Open-FakescanAddDevice -Loud:$Loud -Quiet:$Quiet
    }

    $result = [pscustomobject]@{
        BaseUrl = $BaseUrl
        HealthOk = $health.Ok
        SoapOk = $soap.Ok
        ScannerName = $soap.Snippet
        DiscoveryMatches = $discoveries
        FunctionDiscovery = $fdBefore
        AssociationEndpoints = $aepBefore
        PairAttempt = $pairAttempt
        WiaBefore = $before
        WiaAfter = $null
        FdAfter = $null
        Installed = [bool]$before.FakescanInWia
        Guidance = @(
            '1. Run fakescan with a LAN-reachable --advertise IP and a stable --device-uuid.',
            '2. Allow UDP 3702 / TCP 3920 in the firewall (try --probe-listen on Windows labs).',
            '3. Wait until aep shows Fake WSD scanner with CanPair=true (stronger than Hello alone).',
            '4. Settings -> Printers & scanners -> Add device next to Fake WSD scanner (person click).',
            '5. Connect-FakescanAssociationEndpoint is experimental - PairAsync often Failed without UI.',
            '6. Confirm with: Get-FakescanWiaDevices (FakescanInWia) / Get-FakescanSwdDevices.',
            '7. Sticky WIA offline (0x80210005, no SOAP): helper remove (or elevated) + rebind — not stisvc alone. Ghost AEP / Unpair AccessDenied: bounce-fd (fdPHost then DAS; never dasHost).',
            '8. Elevation is for Fake Imaging remove only (helper or elevated shell) — not silent install; do not elevate fakescan.exe.'
        )
    }

    if ($WaitSeconds -gt 0) {
        Write-Host "Waiting up to ${WaitSeconds}s (FD installable candidate vs WIA Image install) ..."
        $deadline = [datetime]::UtcNow.AddSeconds($WaitSeconds)
        $lastFdMsg = ''
        while ([datetime]::UtcNow -lt $deadline) {
            Start-Sleep -Seconds 3
            $after = Get-FakescanWiaDevices
            if ($after.FakescanInWia) {
                $result.WiaAfter = $after
                $result.Installed = $true
                Write-Host 'WIA enum: fakescan-like device present (install complete).'
                break
            }
            if ($after.StickyPresenceHint -or $after.FakescanPnpOnly) {
                Write-Host 'PnP residue without usable WIA enum — consider: .\scripts\fakescan.ps1 rebind'
            }
            try {
                $fdNow = Find-FakescanFunctionDiscovery -TimeoutMs 4000
                $result.FdAfter = $fdNow
                if ($fdNow.Count -gt 0) {
                    $msg = "FD: fakescan visible (Installable=$($fdNow[0].Installable)) but not yet in WIA - finish Add device UI."
                } else {
                    $msg = 'FD: no fakescan instance yet - Add device list likely empty; Hellos alone are not enough.'
                }
                if ($msg -ne $lastFdMsg) {
                    Write-Host $msg
                    $lastFdMsg = $msg
                }
            } catch {
                Write-Verbose "FD wait poll: $_"
            }
        }
        if (-not $result.WiaAfter) {
            $result.WiaAfter = Get-FakescanWiaDevices
        }
        if (-not $result.Installed) {
            Write-Warning 'Timed out without a fakescan WIA/PnP Image device. Brother (if present) was left alone.'
        }
    }

    $result
}

function Get-FakescanPnpRemoveTargets {
    <#
    .SYNOPSIS
      Image + parent SoftwareDevice SWD\DAFWSDPROVIDER nodes that look like fakescan.
    .DESCRIPTION
      Never matches Brother by name alone. Parents are included when the Image
      InstanceId is under SWD\DAFWSDPROVIDER\urn:uuid:… (with or without
      /ScannerService or \ScannerService suffix), even if the SoftwareDevice
      friendly name is blank.
    #>
    [CmdletBinding()]
    param()

    # Hashtable keys = InstanceId (PS 5.1-safe; avoid HashSet ctor quirks).
    $seen = @{}
    $rows = New-Object System.Collections.Generic.List[object]

    $all = @()
    try {
        $all = @(Get-PnpDevice -ErrorAction SilentlyContinue)
    } catch {
        throw "Get-PnpDevice failed: $_"
    }

    foreach ($d in @($all | Where-Object {
        $_.Class -eq 'Image' -and (
            Test-FakescanLooksLike -Name $_.FriendlyName -Port $_.InstanceId
        )
    })) {
        $id = [string]$d.InstanceId
        if ([string]::IsNullOrWhiteSpace($id) -or $seen.ContainsKey($id)) { continue }
        $seen[$id] = $true
        [void]$rows.Add([pscustomobject]@{
            Name = [string]$d.FriendlyName
            InstanceId = $id
            Status = [string]$d.Status
            Class = [string]$d.Class
            Kind = 'Image'
        })
    }

    foreach ($d in @($all | Where-Object {
        (
            $_.Class -eq 'SoftwareDevice' -or
            $_.InstanceId -match '(?i)DAFWSDPROVIDER'
        ) -and (
            Test-FakescanLooksLike -Name $_.FriendlyName -Port $_.InstanceId
        )
    })) {
        $id = [string]$d.InstanceId
        if ([string]::IsNullOrWhiteSpace($id) -or $seen.ContainsKey($id)) { continue }
        $seen[$id] = $true
        $kind = if ($d.Class -eq 'Image') { 'Image' } else { 'SoftwareDevice' }
        [void]$rows.Add([pscustomobject]@{
            Name = [string]$d.FriendlyName
            InstanceId = $id
            Status = [string]$d.Status
            Class = [string]$d.Class
            Kind = $kind
        })
    }

    # Parent SoftwareDevice for each matched Image UUID (may lack fakescan in name).
    foreach ($img in @($rows.ToArray() | Where-Object { $_.Kind -eq 'Image' })) {
        $parentId = $null
        if ($img.InstanceId -match '(?i)^(SWD\\DAFWSDPROVIDER\\[^\\]+?)(?:[/\\]ScannerService)?$') {
            $parentId = $Matches[1]
        }
        if ([string]::IsNullOrWhiteSpace($parentId) -or $seen.ContainsKey($parentId)) { continue }
        $parent = @($all | Where-Object { $_.InstanceId -eq $parentId } | Select-Object -First 1)
        $seen[$parentId] = $true
        if ($parent) {
            [void]$rows.Add([pscustomobject]@{
                Name = [string]$parent[0].FriendlyName
                InstanceId = $parentId
                Status = [string]$parent[0].Status
                Class = [string]$parent[0].Class
                Kind = 'SoftwareDevice'
            })
        } else {
            [void]$rows.Add([pscustomobject]@{
                Name = '(parent SoftwareDevice)'
                InstanceId = $parentId
                Status = ''
                Class = 'SoftwareDevice'
                Kind = 'SoftwareDevice'
            })
        }
    }

    # Image children first, then SoftwareDevice parents.
    return @(
        @($rows.ToArray() | Where-Object { $_.Kind -eq 'Image' }) +
        @($rows.ToArray() | Where-Object { $_.Kind -ne 'Image' })
    )
}

function Get-FakescanGhostNodes {
    <#
    .SYNOPSIS
      Fakescan PnP nodes that are ghosts: not OK in Windows AND unknown to the daemon. Read-only.

    .DESCRIPTION
      Groups the nodes from Get-FakescanPnpRemoveTargets by uuid. A uuid is a ghost only when
      (a) every leaf node for it has a Status other than OK, (b) it is in the fakescan family
      (00000000-0000-4000-8000-*), and (c) the daemon at -DaemonUrl does NOT list it. The daemon
      list is required: when it cannot be read, nothing is reported (fail closed). Parent nodes
      carry no status and follow their uuid. The real Brother can never match.

    .PARAMETER DaemonUrl
      Fakescan control plane. Default http://127.0.0.1:3920.
    #>
    [CmdletBinding()]
    param([string]$DaemonUrl = 'http://127.0.0.1:3920')

    $family = '00000000-0000-4000-8000-'
    try {
        $devs = Invoke-RestMethod -Uri ($DaemonUrl.TrimEnd('/') + '/v1/devices') -TimeoutSec 5
    } catch {
        Write-Warning "Cannot read the daemon device list at ${DaemonUrl}: $_ (refusing to guess what is a ghost)"
        return @()
    }
    $known = @($devs | ForEach-Object { "$($_.uuid)".ToLowerInvariant() })

    $rows = @(Get-FakescanPnpRemoveTargets | ForEach-Object {
        $m = [regex]::Match($_.InstanceId, 'URN:UUID:([0-9A-Fa-f-]{36})')
        if ($m.Success) {
            [pscustomobject]@{ Uuid = $m.Groups[1].Value.ToLowerInvariant(); Kind = $_.Kind; Name = $_.Name
                               InstanceId = $_.InstanceId; Status = $_.Status; IsParent = ($_.Name -like '(parent*') }
        }
    })
    $ghosts = foreach ($group in ($rows | Group-Object Uuid)) {
        $u = $group.Name
        if (-not $u.StartsWith($family) -or ($known -contains $u)) { continue }
        $leaves = @($group.Group | Where-Object { -not $_.IsParent })
        if ($leaves.Count -eq 0 -or @($leaves | Where-Object { $_.Status -eq 'OK' }).Count -gt 0) { continue }
        $group.Group
    }
    # Children first: Image leaf, then the SoftwareDevice leaf, then the parent node.
    $rank = @{ 'Image' = 0 }
    return @($ghosts | Sort-Object @{ e = { if ($_.IsParent) { 2 } elseif ($rank.ContainsKey($_.Kind)) { 0 } else { 1 } } }, Uuid)
}

function Remove-FakescanGhost {
    <#
    .SYNOPSIS
      Remove ghost fakescan PnP nodes (see Get-FakescanGhostNodes). Dry run unless -Apply.

    .DESCRIPTION
      Without -Apply this only lists what would be removed. With -Apply it needs an elevated
      shell (it does not elevate itself, and fakescan.exe is never elevated) and removes each
      ghost node with `pnputil /remove-device`. Only uuids the daemon does not know and Windows
      does not report OK are touched; the installed fake devices and the Brother are never matched.

    .PARAMETER Uuid
      Limit to these uuids (must still qualify as ghosts).

    .EXAMPLE
      Remove-FakescanGhost                 # lists, changes nothing
      Remove-FakescanGhost -Apply          # elevated shell: removes them
    #>
    [CmdletBinding(SupportsShouldProcess = $true)]
    param([string[]]$Uuid, [string]$DaemonUrl = 'http://127.0.0.1:3920', [switch]$Apply)

    $nodes = @(Get-FakescanGhostNodes -DaemonUrl $DaemonUrl)
    if ($Uuid) {
        $want = @($Uuid | ForEach-Object { $_.ToLowerInvariant() })
        $nodes = @($nodes | Where-Object { $want -contains $_.Uuid })
    }
    if ($nodes.Count -eq 0) {
        Write-Host 'No ghost fakescan PnP nodes (every fake node is OK or known to the daemon).'
        return [pscustomobject]@{ Matched = @(); Removed = @(); Failed = @() }
    }
    foreach ($n in $nodes) { Write-Host ("Ghost [{0}] {1} {2}" -f $n.Kind, $n.Name, $n.InstanceId) }
    if (-not $Apply) {
        Write-Host 'Dry run: nothing changed. Re-run with -Apply in an elevated shell to remove these nodes.'
        return [pscustomobject]@{ Matched = $nodes; Removed = @(); Failed = @() }
    }
    if (-not (Test-FakescanProcessElevated)) {
        Write-Warning 'Not elevated: pnputil cannot remove devices. Open an elevated PowerShell (Start-FakescanElevatedShell) and re-run with -Apply.'
        return [pscustomobject]@{ Matched = $nodes; Removed = @(); Failed = @() }
    }
    $removed = New-Object System.Collections.Generic.List[string]
    $failed = New-Object System.Collections.Generic.List[object]
    foreach ($n in $nodes) {
        if (-not $PSCmdlet.ShouldProcess("$($n.Name) ($($n.InstanceId))", 'pnputil /remove-device')) { continue }
        $out = & pnputil.exe /remove-device $n.InstanceId 2>&1 | Out-String
        if ($LASTEXITCODE -eq 0) { [void]$removed.Add($n.InstanceId); Write-Host "  Removed $($n.InstanceId)" }
        else { [void]$failed.Add([pscustomobject]@{ InstanceId = $n.InstanceId; Error = $out.Trim() }); Write-Warning "  Failed $($n.InstanceId): $($out.Trim())" }
    }
    return [pscustomobject]@{ Matched = $nodes; Removed = @($removed); Failed = @($failed) }
}

function Remove-FakescanScanner {
    <#
    .SYNOPSIS
      Disable and remove fakescan Image + SoftwareDevice PnP nodes via pnputil.

    .DESCRIPTION
      Matches Image and SWD\DAFWSDPROVIDER SoftwareDevice nodes that look like
      fakescan (name contains fakescan / Fake WSD scanner, or :3920), plus parent
      SoftwareDevice nodes for matched Image urn:uuid ids. Never matches a
      generic Brother WSD entry.

      Remove-PnpDevice is missing on some Windows builds; primary path is
      `pnputil /remove-device`. Requires elevation for disable/remove.
      From an unelevated suite, prefer: .\scripts\fakescan.ps1 helper then
      helper remove (Invoke-FakescanElevatedHelper). Or run this in an elevated
      shell (fakescan.ps1 elevated). Do not elevate fakescan.exe.

    .PARAMETER WhatIf
      List matching Image + SoftwareDevice nodes without changing them.

    .EXAMPLE
      Remove-FakescanScanner -WhatIf
      # Lists Image SWD\DAFWSDPROVIDER\...\ScannerService and parent SoftwareDevice

    .EXAMPLE
      # Unelevated suite: one UAC helper, then remove without elevating this shell
      Start-FakescanElevatedHelper
      Invoke-FakescanElevatedHelper -Cmd remove
    #>
    [CmdletBinding(SupportsShouldProcess = $true)]
    param()

    $devices = @(Get-FakescanPnpRemoveTargets)

    if ($devices.Count -eq 0) {
        Write-Host 'No fakescan-like Image/SoftwareDevice PnP nodes found (Brother left alone).'
        return [pscustomobject]@{
            Matched = @()
            Disabled = @()
            Removed = @()
            Failed = @()
        }
    }

    $disabled = New-Object System.Collections.Generic.List[string]
    $removed = New-Object System.Collections.Generic.List[string]
    $failed = New-Object System.Collections.Generic.List[object]

    foreach ($d in $devices) {
        $id = $d.InstanceId
        $name = $d.Name
        $kind = $d.Kind
        Write-Host "Matched [$kind]: $name ($id)"
        if (-not $PSCmdlet.ShouldProcess("$name ($id)", "Disable and remove fakescan $kind via pnputil")) {
            continue
        }
        try {
            Disable-PnpDevice -InstanceId $id -Confirm:$false -ErrorAction Stop
            [void]$disabled.Add($id)
            Write-Host "  Disabled $id"
        } catch {
            Write-Warning "  Disable-PnpDevice failed for ${id}: $_"
            [void]$failed.Add([pscustomobject]@{ InstanceId = $id; Kind = $kind; Step = 'Disable'; Error = "$_" })
        }
        # Remove-PnpDevice is missing on some Windows builds; pnputil is reliable.
        $pnputilOk = $false
        try {
            $pnpOut = & pnputil.exe /remove-device $id 2>&1 | Out-String
            if ($LASTEXITCODE -eq 0 -or $pnpOut -match '(?i)removed successfully') {
                [void]$removed.Add($id)
                $pnputilOk = $true
                Write-Host "  Removed via pnputil $id"
            } else {
                Write-Warning "  pnputil /remove-device failed for ${id}: $pnpOut"
            }
        } catch {
            Write-Warning "  pnputil /remove-device failed for ${id}: $_"
        }
        if (-not $pnputilOk) {
            try {
                if (Get-Command Remove-PnpDevice -ErrorAction SilentlyContinue) {
                    Remove-PnpDevice -InstanceId $id -Confirm:$false -ErrorAction Stop
                    [void]$removed.Add($id)
                    Write-Host "  Removed $id"
                } else {
                    [void]$failed.Add([pscustomobject]@{
                        InstanceId = $id
                        Kind = $kind
                        Step = 'Remove'
                        Error = 'Remove-PnpDevice missing; pnputil failed'
                    })
                }
            } catch {
                Write-Warning "  Remove-PnpDevice failed for ${id}: $_"
                [void]$failed.Add([pscustomobject]@{ InstanceId = $id; Kind = $kind; Step = 'Remove'; Error = "$_" })
            }
        }
    }

    # Windows PowerShell throws "Argument types do not match" if List[object]
    # (Failed) is wrapped with @(), including empty after -WhatIf. ToArray on
    # all three keeps the return shape uniform for ConvertTo-Json / helper IPC.
    [pscustomobject]@{
        Matched = @($devices)
        Disabled = @($disabled.ToArray())
        Removed = @($removed.ToArray())
        Failed = @($failed.ToArray())
    }
}

function Restart-FakescanDiscoveryServices {
    <#
    .SYNOPSIS
      Bounce Function Discovery Provider Host then Device Association Service.

    .DESCRIPTION
      Layer for a ghost WSD AssociationEndpoint: PnP Image gone, AEP still
      IsPaired=True, Settings Add list empty, WinRT UnpairAsync AccessDenied.

      Order (Microsoft):
        1. fdPHost — WSD Function Discovery cache. KB 973560: stop/start
           fdPHost so providers reload WSD devices. This is the primary bounce;
           lab cleared a ghost AEP after fdPHost alone (2026-10-12).
        2. DeviceAssociationService — pairing-store consumers (Add list /
           IsPaired). Empty Add is a documented DAS-*stopped* symptom.
           **Never Stop / Restart-Service -Force on DAS**: on current Windows
           that wedges StopPending for minutes, hangs WinRT AEP FindAllAsync,
           and blocks Settings Add. Only Start-Service when Status is Stopped
           (or after an external unstick of StopPending).

      Never bounce dasHost (Device Association Framework provider host): it
      owns UDP 3702; a restart drops Hellos/Probes for fakescan and fakeprint.
      Never bounce FDResPub as the scan fix: that service publishes *this* PC.
      If Restart-Service -Force on fdPHost stopped FDResPub as a dependent,
      this function starts it again only to restore the previous state.

      stisvc (WIA) is opt-in (-StiSvc). Lab already showed stisvc + DAS
      restart does **not** clear sticky Connect 0x80210005 with PnP OK.
      Use this after pnputil remove + Unpair AccessDenied, not instead of
      remove. Needs elevation (or: helper bounce-fd).
    #>
    [CmdletBinding()]
    param(
        [switch]$StiSvc
    )

    if (-not (Test-FakescanProcessElevated)) {
        throw 'Restart-FakescanDiscoveryServices needs elevation. Use: .\scripts\fakescan.ps1 helper bounce-fd'
    }

    # dasHost is a process name, not a service — listed only so After/Before never imply we touched it.
    $watch = @('fdPHost', 'DeviceAssociationService', 'FDResPub', 'stisvc')
    $snapshot = @{}
    foreach ($n in $watch) {
        $svc = Get-Service -Name $n -ErrorAction SilentlyContinue
        $snapshot[$n] = if ($svc) { [string]$svc.Status } else { 'Missing' }
    }
    $snapshot['dasHost'] = 'NeverTouched'

    $restarted = New-Object 'System.Collections.Generic.List[string]'
    $restored = New-Object 'System.Collections.Generic.List[string]'
    $errors = New-Object 'System.Collections.Generic.List[string]'

    try {
        Restart-Service -Name fdPHost -Force -ErrorAction Stop
        [void]$restarted.Add('fdPHost')
    } catch {
        [void]$errors.Add("fdPHost: $_")
    }
    Start-Sleep -Seconds 2

    $fdres = Get-Service -Name FDResPub -ErrorAction SilentlyContinue
    if ($fdres -and $snapshot['FDResPub'] -eq 'Running' -and [string]$fdres.Status -ne 'Running') {
        try {
            Start-Service -Name FDResPub -ErrorAction Stop
            [void]$restored.Add('FDResPub')
        } catch {
            [void]$errors.Add("FDResPub restore: $_")
        }
    }

    # DeviceAssociationService: Start only — never Stop/Restart-Service.
    # Restart-Service -Force waits on Stop and can leave Status=StopPending forever
    # (lab 2026-10-12: helper bounce-fd hung; Add/AEP dead until DAS unstuck).
    $dasSoftFail = $null
    $das = Get-Service -Name DeviceAssociationService -ErrorAction SilentlyContinue
    if (-not $das) {
        $dasSoftFail = 'DeviceAssociationService missing'
        [void]$errors.Add("DeviceAssociationService (soft): $dasSoftFail")
    } elseif ([string]$das.Status -eq 'Running') {
        # Healthy — leave alone. Ghost AEP clears from fdPHost; forcing Stop wedges Add.
    } elseif ([string]$das.Status -eq 'Stopped') {
        try {
            Start-Service -Name DeviceAssociationService -ErrorAction Stop
            [void]$restarted.Add('DeviceAssociationService')
        } catch {
            $dasSoftFail = "$_"
            [void]$errors.Add("DeviceAssociationService (soft): $dasSoftFail")
        }
    } else {
        # StopPending / StartPending — do not call Stop or Restart-Service.
        $dasSoftFail = "Status=$($das.Status) (will not Stop/Restart; unstick outside bounce-fd if needed)"
        [void]$errors.Add("DeviceAssociationService (soft): $dasSoftFail")
        try {
            Start-Service -Name DeviceAssociationService -ErrorAction SilentlyContinue
        } catch { }
    }
    Start-Sleep -Seconds 1

    if ($StiSvc) {
        try {
            Restart-Service -Name stisvc -Force -ErrorAction Stop
            [void]$restarted.Add('stisvc')
        } catch {
            [void]$errors.Add("stisvc: $_")
        }
        Start-Sleep -Seconds 2
    }

    $after = @{}
    foreach ($n in $watch) {
        $svc = Get-Service -Name $n -ErrorAction SilentlyContinue
        $after[$n] = if ($svc) { [string]$svc.Status } else { 'Missing' }
    }
    $after['dasHost'] = 'NeverTouched'

    # Success = fdPHost running after bounce. DAS soft notes are not fatal.
    $hardErrors = @($errors.ToArray() | Where-Object { $_ -notmatch 'DeviceAssociationService \(soft\)' })
    $ok = ($hardErrors.Count -eq 0 -and $after['fdPHost'] -eq 'Running')
    [pscustomobject]@{
        Ok = $ok
        Restarted = $restarted.ToArray()
        RestoredDependents = $restored.ToArray()
        NeverTouched = @('dasHost')
        StiSvc = [bool]$StiSvc
        Before = $snapshot
        After = $after
        DasStopRefused = [bool]$dasSoftFail
        Error = if ($errors.Count) { ($errors.ToArray() -join '; ') } else { $null }
        Guidance = @(
            'fdPHost: WSD Function Discovery cache (KB 973560) — primary bounce.',
            'DeviceAssociationService: Start only when Stopped — never Restart-Service -Force (StopPending wedges Add/AEP).',
            'Never bounce dasHost (UDP 3702 — fakescan and fakeprint Hellos).',
            'stisvc is WIA enum only; it does not unglue a ghost AEP or sticky 0x80210005 association.'
        )
    }
}

function Invoke-FakescanDiscoveryRestart {
    <#
    .SYNOPSIS
      Restart-FakescanDiscoveryServices in-process if elevated, else via helper bounce-fd.
    #>
    [CmdletBinding()]
    param(
        [switch]$StiSvc
    )
    if ($StiSvc -and -not (Test-FakescanProcessElevated)) {
        throw 'bounce-fd -StiSvc needs an elevated shell (helper bounce-fd does not pass -StiSvc).'
    }
    if (Test-FakescanProcessElevated) {
        return Restart-FakescanDiscoveryServices -StiSvc:$StiSvc
    }
    if (-not (Get-FakescanElevatedHelperSession)) {
        Write-Host 'Not elevated — starting session helper (one UAC), then bounce-fd ...'
        [void](Start-FakescanElevatedHelper)
    } else {
        Write-Host 'Using session elevated helper for bounce-fd ...'
    }
    $reply = Invoke-FakescanElevatedHelper -Cmd 'bounce-fd' -TimeoutSeconds 90
    if ($reply -and $reply.ok -eq $false) {
        throw "helper bounce-fd: $($reply.error)"
    }
    if ($reply.result) { return $reply.result }
    $reply
}

function Invoke-FakescanPairToolDisassociate {
    <#
    .SYNOPSIS
      Official WDK/Win11 PairTool /disassociate on a WSD AssociationEndpoint id.
    .DESCRIPTION
      WinRT UnpairAsync AccessDenied means the caller lacks pairing permission
      (DeviceUnpairingResultStatus.AccessDenied), even when elevated. PairTool
      is Microsoft's protocol-device unpair. Present in later Windows 11
      System32; otherwise WDK. Never pass a fakeprint (...f3) id.
    #>
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$DeviceId
    )
    if ([string]::IsNullOrWhiteSpace($DeviceId)) {
        return [pscustomobject]@{ Ok = $false; Skipped = $true; Error = 'no DeviceId'; Output = $null }
    }
    if ($DeviceId -match '(?i)00000000-0000-4000-8000-0000000000f3') {
        return [pscustomobject]@{ Ok = $false; Skipped = $true; Error = 'refusing fakeprint ...f3 AEP'; Output = $null }
    }
    $exe = @(
        (Join-Path $env:SystemRoot 'System32\pairtool.exe'),
        (Join-Path $env:SystemRoot 'Sysnative\pairtool.exe')
    ) | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
    if (-not $exe) {
        return [pscustomobject]@{
            Ok = $false
            Skipped = $true
            Error = 'PairTool.exe not present (later Win11 System32, or WDK)'
            Output = $null
        }
    }
    $out = & $exe /disassociate $DeviceId 2>&1 | Out-String
    $code = $LASTEXITCODE
    [pscustomobject]@{
        Ok = ($code -eq 0)
        Skipped = $false
        Error = if ($code -eq 0) { $null } else { "pairtool exit $code" }
        Output = $out.Trim()
        Exe = $exe
        DeviceId = $DeviceId
    }
}

function Repair-FakescanScanner {
    <#
    .SYNOPSIS
      Rebind sticky/offline fakescan: remove → Unpair → Printers & scanners → wait WIA.

    .DESCRIPTION
      Two stuck states, two layers:

        A) Sticky WIA offline: PnP OK, Connect 0x80210005, no /ScannerService
           SOAP. stisvc + DeviceAssociationService restart did **not** clear
           it in lab. Sequence starts with pnputil remove + person re-Add.
        B) Ghost AEP: PnP gone, AEP IsPaired=True, Settings Add empty, Unpair
           AccessDenied. fdPHost was never bounced in lab. After Unpair
           AccessDenied (or -RestartDiscovery), bounce fdPHost then DAS
           (Restart-FakescanDiscoveryServices). Never dasHost.

      Sequence:

        1) Remove-FakescanScanner (Image + SoftwareDevice via pnputil; needs elevation
           in-process, or start helper first and remove via helper)
        2) Disconnect-FakescanAssociationEndpoint (clear stale IsPaired so Add lists Fake)
        3) If Unpair AccessDenied (or -RestartDiscovery): bounce fdPHost + DAS,
           retry Unpair; PairTool /disassociate if present and still denied
        4) Open-FakescanAddDevice (ms-settings:printers only)
        5) Optional wait until FakescanInWia

      Last resort (not this command): new --device-uuid so the stuck AEP id
      is no longer the one Windows is pairing. Add device is still a person.
      Do not run fakescan.exe elevated.
      HTTP checks use Invoke-WebRequest / Invoke-RestMethod (not curl.exe).

    .EXAMPLE
      # Prefer helper so this shell stays unelevated for remove:
      .\scripts\fakescan.ps1 helper
      .\scripts\fakescan.ps1 helper remove
      .\scripts\fakescan.ps1 rebind -BaseUrl http://192.168.1.70:3920 -WaitSeconds 120 -SkipRemove
      # Person: Printers & scanners → Add device next to Fake WSD scanner

    .EXAMPLE
      .\scripts\fakescan.ps1 elevated
      Repair-FakescanScanner -BaseUrl http://192.168.1.70:3920 -WaitSeconds 120

    .EXAMPLE
      .\scripts\fakescan.ps1 rebind -BaseUrl http://192.168.1.70:3920 -WaitSeconds 120
    #>
    [CmdletBinding()]
    param(
        [string]$BaseUrl = $script:DefaultBaseUrl,
        [int]$WaitSeconds = 120,
        [switch]$SkipUi,
        [switch]$SkipRemove,
        # Bounce fdPHost + DeviceAssociationService even if Unpair succeeded.
        [switch]$RestartDiscovery,
        # Passed through to Open-FakescanAddDevice / Invoke-LabAttention.
        [switch]$Loud,
        [switch]$Quiet
    )

    $BaseUrl = Resolve-FakescanBaseUrl $BaseUrl
    $before = Get-FakescanWiaDevices
    Write-Host ("Before: FakescanInWia={0} FakescanInPnp={1} StickyPresenceHint={2} OnlineCapable={3}" -f `
        $before.FakescanInWia, $before.FakescanInPnp, $before.StickyPresenceHint, $before.OnlineCapable)

    $health = $null
    try {
        $health = Get-FakescanHealth -BaseUrl $BaseUrl
        if (-not $health.Ok) {
            Write-Warning "fakescan /health not ok at $BaseUrl — continue rebind anyway (sticky PnP may still need remove)."
        }
    } catch {
        Write-Warning "fakescan /health failed at ${BaseUrl}: $_ — continue rebind anyway."
    }

    $removeResult = $null
    if (-not $SkipRemove) {
        Write-Host 'Removing fakescan Image + SoftwareDevice nodes (pnputil; elevation required) ...'
        if (Test-FakescanProcessElevated) {
            $removeResult = Remove-FakescanScanner
        } elseif (Get-FakescanElevatedHelperSession) {
            Write-Host 'Using session elevated helper for remove ...'
            $reply = Invoke-FakescanElevatedHelper -Cmd remove
            $removeResult = $reply.result
        } else {
            Write-Host 'Not elevated — starting session helper (one UAC), then remove ...'
            [void](Start-FakescanElevatedHelper)
            $reply = Invoke-FakescanElevatedHelper -Cmd remove
            $removeResult = $reply.result
        }
        $removeResult | Format-List Matched, Disabled, Removed, Failed | Out-Host
        if ($null -eq $removeResult -or (@($removeResult.Failed).Count -gt 0 -and @($removeResult.Removed).Count -eq 0)) {
            Write-Warning 'Remove failed — run: .\scripts\fakescan.ps1 helper   (or elevated) then helper remove / rebind again.'
        }
    }

    Write-Host 'Unpairing stale WSD AssociationEndpoint (so Add device can list Fake) ...'
    $unpairResult = Disconnect-FakescanAssociationEndpoint
    if (-not $unpairResult.Ok -and (
            [string]$unpairResult.Status -match 'AccessDenied' -or
            [string]$unpairResult.Error -match '(?i)AccessDenied|access is denied'
        )) {
        Write-Host 'Unpair AccessDenied unelevated — retrying via elevated helper ...'
        if (-not (Get-FakescanElevatedHelperSession)) {
            [void](Start-FakescanElevatedHelper)
        }
        try {
            $reply = Invoke-FakescanElevatedHelper -Cmd unpair
            $unpairResult = $reply.result
        } catch {
            Write-Warning "elevated unpair failed: $_"
        }
    }

    $bounceResult = $null
    $pairToolResult = $null
    $stillDenied = $unpairResult -and -not $unpairResult.Ok -and (
        [string]$unpairResult.Status -match 'AccessDenied' -or
        [string]$unpairResult.Error -match '(?i)AccessDenied|access is denied'
    )
    if ($RestartDiscovery -or $stillDenied) {
        Write-Host 'Bouncing fdPHost then DeviceAssociationService (WSD FD cache + pairing store; never dasHost) ...'
        try {
            $bounceResult = Invoke-FakescanDiscoveryRestart
            $bounceResult | Format-List Ok, Restarted, RestoredDependents, Error | Out-Host
        } catch {
            Write-Warning "discovery bounce failed: $_"
        }
        Write-Host 'Retrying Unpair after fdPHost/DAS bounce ...'
        $unpairResult = Disconnect-FakescanAssociationEndpoint
        $stillDenied = $unpairResult -and -not $unpairResult.Ok -and (
            [string]$unpairResult.Status -match 'AccessDenied' -or
            [string]$unpairResult.Error -match '(?i)AccessDenied|access is denied'
        )
        if ($stillDenied -and $unpairResult.DeviceId -and (Test-FakescanProcessElevated)) {
            Write-Host 'Unpair still AccessDenied — PairTool /disassociate if present ...'
            $pairToolResult = Invoke-FakescanPairToolDisassociate -DeviceId ([string]$unpairResult.DeviceId)
            $pairToolResult | Format-List Ok, Skipped, Error, Output | Out-Host
            $unpairResult = Disconnect-FakescanAssociationEndpoint
        }
    }
    $unpairResult | Format-List Ok, Status, Error, IsPairedAfter, Name | Out-Host
    if (-not $unpairResult.Ok) {
        Write-Warning 'Unpair failed — Fake may stay hidden under Add until association clears. Last resort: new --device-uuid (stuck AEP id).'
    }

    if (-not $SkipUi) {
        Open-FakescanAddDevice -Loud:$Loud -Quiet:$Quiet
    }

    $after = $null
    $installed = $false
    if ($WaitSeconds -gt 0) {
        Write-Host "Waiting up to ${WaitSeconds}s for WIA enum (FakescanInWia) ..."
        $deadline = [datetime]::UtcNow.AddSeconds($WaitSeconds)
        while ([datetime]::UtcNow -lt $deadline) {
            Start-Sleep -Seconds 3
            $after = Get-FakescanWiaDevices
            if ($after.FakescanInWia) {
                $installed = $true
                Write-Host 'WIA enum: fakescan present after rebind.'
                break
            }
        }
        if (-not $after) { $after = Get-FakescanWiaDevices }
        if (-not $installed) {
            Write-Warning 'Timed out waiting for FakescanInWia. Finish Add device in Printers & scanners, then: fakescan.ps1 wia'
        }
    } else {
        $after = Get-FakescanWiaDevices
        $installed = [bool]$after.FakescanInWia
    }

    [pscustomobject]@{
        BaseUrl = $BaseUrl
        HealthOk = if ($health) { [bool]$health.Ok } else { $false }
        Remove = $removeResult
        Unpair = $unpairResult
        DiscoveryBounce = $bounceResult
        PairTool = $pairToolResult
        WiaBefore = $before
        WiaAfter = $after
        Installed = $installed
        Guidance = @(
            'Sticky 0x80210005 with PnP OK and no SOAP: remove + person Add device (this command).',
            'stisvc restart alone does not clear sticky presence (tried in lab).',
            'Ghost AEP (PnP gone, IsPaired, Add empty, Unpair AccessDenied): fdPHost then DeviceAssociationService — never dasHost.',
            'Confirm: .\scripts\fakescan.ps1 aep  (IsPaired=False CanPair=True) then wia (FakescanInWia=True).',
            'After ghost clear, keep fakescan --probe-listen (registry probe_listen=true); Hello-only may not republish AEP.'
        )
    }
}

function Test-FakescanFunctionDiscoveryRegistered {
    <#
    .SYNOPSIS
      True if the Windows Function Discovery COM class is registered (WSD stack present).
    #>
    [CmdletBinding()]
    param()
    try {
        $t = [type]::GetTypeFromCLSID([guid]'C72BE2EC-8E90-452c-B29A-AB8FF1C071FC')
        $o = [Activator]::CreateInstance($t)
        [void]$o
        return $true
    } catch {
        return $false
    }
}

function Test-FakescanDiscoverable {
    <#
    .SYNOPSIS
      Report whether fakescan SOAP works and whether its WS-Discovery announcement is on the wire.

    .DESCRIPTION
      These are different things:

        SoapReachable      - TCP GetScannerElements works (scan protocol up)
        UdpProbeMatch      - ProbeMatch and/or Hello with :3920 seen on UDP 3702
        FunctionDiscoveryCom - Windows FD COM class is registered
        FunctionDiscoveryHit - live FD WSD query returned a fakescan-like instance
        FunctionDiscoveryInstallable - that hit has PKEY_PNPX_Installable=true (Add-device candidate)
        AlreadyInstalled   - FakescanInWia (usable WIA enum)
        Sticky / PnP-only  - PnP OK/Error without WIA enum (rebind)
        Ghost AEP          - PnP gone, AEP still IsPaired (Settings hides Add)

      Verdict:
        installed              - WIA enum has fakescan (FakescanInWia)
        installed-offline      - PnP residue / sticky presence; WIA missing or not usable
        ghost-aep              - SOAP OK, no Imaging node, AEP IsPaired (Add list empty)
        fd-installable         - SOAP + FD Installable=true (best pre-Add-device signal)
        aep-pairable           - SOAP + unpaired AEP CanPair=true
        on-wire-discoverable   - SOAP + Hello/ProbeMatch (UDP announce OK; FD may still miss it)
        soap-only              - SOAP works but no UDP announce visible
        unreachable            - SOAP failed

      Note: SOAP reachable alone does NOT mean Windows can discover/install the scanner.
      dasHost may SOAP-Get after Hello even when Function Discovery does not return an
      installable instance (Add device stays empty). On Windows, Hello is often the only
      automated UDP signal (dasHost eats Probes). Sticky 0x80210005 with PnP OK and
      zero SOAP needs helper/elevated remove + person Add device (rebind), not stisvc alone.
    #>
    [CmdletBinding()]
    param(
        [string]$BaseUrl = $script:DefaultBaseUrl,
        [int]$DiscoveryTimeoutMs = 4000
    )

    $BaseUrl = Resolve-FakescanBaseUrl $BaseUrl
    $notes = New-Object System.Collections.Generic.List[string]

    $soap = $null
    $soapOk = $false
    try {
        $soap = Test-FakescanSoap -BaseUrl $BaseUrl
        $soapOk = [bool]$soap.Ok
        if (-not $soapOk) { [void]$notes.Add('SOAP GetScannerElements failed - service not usable yet.') }
    } catch {
        [void]$notes.Add("SOAP error: $_")
    }

    $udpHits = @()
    $udpOk = $false
    $hellosSent = $null
    try {
        $st0 = Get-FakescanStatus -BaseUrl $BaseUrl
        $hellosSent = [uint64]$st0.wsd_hellos_sent
        $uni = $null
        try {
            $u = [Uri]$BaseUrl
            if ($u.Host -notmatch '^(127\.|localhost)') { $uni = $u.Host }
        } catch { }
        $udpHits = @(Find-FakescanDiscovery -TimeoutMs $DiscoveryTimeoutMs -UnicastHost $uni)
        # Only count ProbeMatch rows that look like fakescan (name or :3920).
        # A real Brother WSD ProbeMatch with /ScannerService must not count.
        $fakescanHits = @($udpHits | Where-Object { $_.LooksLikeFakescan })
        $udpOk = [bool]($fakescanHits.Count -gt 0)
        if ($udpHits.Count -gt 0 -and -not $udpOk) {
            [void]$notes.Add("ProbeMatch(s) seen ($($udpHits.Count)) but none look like fakescan (need name or :3920; /ScannerService alone is ignored).")
        }
        if ($udpOk) {
            [void]$notes.Add('fakescan ProbeMatch received - full WS-Discovery probe/response path works.')
        } else {
            # dasHost often eats inbound Probes; Hello multicast / status counter still proves announce.
            $helloHits = @(Find-FakescanHello -TimeoutMs $DiscoveryTimeoutMs)
            if ($helloHits | Where-Object { $_.LooksLikeFakescan }) {
                $udpHits = $helloHits
                $udpOk = $true
                [void]$notes.Add('WS-Discovery Hello seen on wire (announce OK).')
            } else {
                Start-Sleep -Milliseconds 5500
                $st1 = Get-FakescanStatus -BaseUrl $BaseUrl
                $hellos1 = [uint64]$st1.wsd_hellos_sent
                if ($hellos1 -gt $hellosSent) {
                    $udpOk = $true
                    $udpHits = @([pscustomobject]@{
                        Peer = 'status'
                        DeviceUuid = $st1.device_uuid
                        XAddrs = @(
                            $(if ($st1.device_endpoint) { $st1.device_endpoint } else { $st1.service_endpoint })
                        )
                        LooksLikeFakescan = $true
                        Kind = 'HelloCounter'
                        HellosSent = $hellos1
                    })
                    [void]$notes.Add("WS-Discovery Hellos advancing ($hellosSent -> $hellos1) - UDP send path OK (same-host :3702 listen may be blocked by dasHost).")
                } else {
                    [void]$notes.Add('No fakescan WS-Discovery Hello or ProbeMatch - UDP announce not visible on this PC.')
                    [void]$notes.Add('Fix: native fakescan --advertise <LAN-IP>, allow UDP 3702, avoid Docker Desktop bridge.')
                }
            }
        }
    } catch {
        [void]$notes.Add("UDP discovery error: $_")
    }

    $fdCom = Test-FakescanFunctionDiscoveryRegistered
    $fdHit = $false
    $fdInstallable = $false
    $fdHits = @()
    if (-not $fdCom) {
        [void]$notes.Add('Function Discovery COM class not registered on this SKU.')
    } else {
        try {
            $directed = $null
            try {
                $u = [Uri]$BaseUrl
                if ($u.Host -notmatch '^(127\.|localhost)') {
                    $directed = $BaseUrl.TrimEnd('/') + '/'
                }
            } catch { }
            $fdParams = @{ TimeoutMs = [Math]::Max(5000, $DiscoveryTimeoutMs) }
            if (-not [string]::IsNullOrWhiteSpace($directed)) {
                $fdParams['DirectedAddress'] = $directed
            }
            $fd = Get-FakescanFunctionDiscovery @fdParams
            $fdHits = @($fd.FakescanHits)
            $fdHit = [bool]$fd.FakescanPresent
            $fdInstallable = [bool]$fd.InstallableFakescan
            if ($fdInstallable) {
                [void]$notes.Add('Function Discovery returned fakescan with Installable=true (Add device candidate).')
            } elseif ($fdHit) {
                [void]$notes.Add('Function Discovery returned fakescan but Installable!=true - Add device may omit it.')
            } else {
                [void]$notes.Add('Function Discovery WSD query returned no fakescan-like instance (Brother/other WSD may still appear).')
                [void]$notes.Add('SOAP Get after Hello can still occur without an FD installable hit - that is not install.')
            }
        } catch {
            [void]$notes.Add("Function Discovery live query failed: $_")
        }
    }

    $wia = Get-FakescanWiaDevices
    $installed = [bool]$wia.FakescanInWia
    $stickyOrPnpOnly = [bool]($wia.StickyPresenceHint -or $wia.FakescanPnpOnly)
    if ($installed) {
        [void]$notes.Add('Already present in WIA enum (FakescanInWia).')
        if ($wia.OnlineCapable) {
            [void]$notes.Add('PnP OnlineCapable=True; if Connect still returns 0x80210005 with no SOAP, run: fakescan.ps1 rebind')
        }
    } elseif ($stickyOrPnpOnly) {
        [void]$notes.Add('Installed but WIA offline / sticky: PnP residue without usable WIA enum (or Error/Disconnected).')
        [void]$notes.Add('Recovery: helper remove (or elevated) + person Add device (fakescan.ps1 rebind). stisvc restart alone is not enough. Ghost AEP: bounce-fd after Unpair AccessDenied.')
    }

    $aepHit = $false
    $aepCanPair = $false
    $aepPaired = $false
    $aepHits = @()
    try {
        $aep = Get-FakescanAssociationEndpoints -TimeoutMs ([Math]::Max(15000, $DiscoveryTimeoutMs))
        if ($aep.Ok) {
            $aepHits = @($aep.FakescanHits)
            $aepHit = [bool]$aep.FakescanPresent
            $aepCanPair = [bool]$aep.CanPairFakescan
            $aepPaired = [bool]$aep.PairedFakescan
            if ($aepHit) {
                [void]$notes.Add("WinRT AEP fakescan hit (CanPair=$aepCanPair IsPaired=$aepPaired).")
            } else {
                [void]$notes.Add("WinRT AEP WSD count=$($aep.Hits.Count); no fakescan-like row.")
            }
        } else {
            [void]$notes.Add("WinRT AEP enumerate failed: $($aep.Error)")
        }
    } catch {
        [void]$notes.Add("WinRT AEP enumerate failed: $_")
    }

    $ghostAep = [bool]($soapOk -and -not $installed -and -not $wia.FakescanInPnp -and $aepPaired)
    if ($ghostAep) {
        [void]$notes.Add('Ghost AEP: PnP Image gone, AssociationEndpoint still IsPaired — Settings hides Fake under Add.')
        [void]$notes.Add('Recovery: fakescan.ps1 rebind (Unpair; on AccessDenied bounce-fd = fdPHost then DAS, never dasHost). Last resort: new --device-uuid.')
    }

    $stuckKind = if ($installed) {
        'none'
    } elseif ($stickyOrPnpOnly) {
        'sticky-wia-offline'
    } elseif ($ghostAep) {
        'ghost-aep'
    } else {
        'none'
    }

    $verdict = if ($installed) {
        'installed'
    } elseif ($stickyOrPnpOnly) {
        'installed-offline'
    } elseif ($ghostAep) {
        'ghost-aep'
    } elseif ($fdInstallable -and $soapOk) {
        'fd-installable'
    } elseif ($aepCanPair -and $soapOk) {
        'aep-pairable'
    } elseif (($aepHit -or $udpOk) -and $soapOk) {
        'on-wire-discoverable'
    } elseif ($soapOk) {
        'soap-only'
    } else {
        'unreachable'
    }

    [pscustomobject]@{
        BaseUrl = $BaseUrl
        SoapReachable = $soapOk
        ScannerName = if ($soap) { $soap.Snippet } else { $null }
        UdpProbeMatch = $udpOk
        UdpMatches = $udpHits
        WsdHellosSent = $hellosSent
        FunctionDiscoveryCom = $fdCom
        FunctionDiscoveryHit = $fdHit
        FunctionDiscoveryInstallable = $fdInstallable
        FunctionDiscoveryHits = $fdHits
        AssociationEndpointHit = $aepHit
        AssociationEndpointCanPair = $aepCanPair
        AssociationEndpointPaired = $aepPaired
        AssociationEndpointHits = $aepHits
        AlreadyInstalled = $installed
        FakescanInWia = [bool]$wia.FakescanInWia
        FakescanInPnp = [bool]$wia.FakescanInPnp
        FakescanPnpOnly = [bool]$wia.FakescanPnpOnly
        OnlineCapable = [bool]$wia.OnlineCapable
        WiaConnectable = [bool]$wia.WiaConnectable
        StickyPresenceHint = [bool]$wia.StickyPresenceHint
        GhostAep = $ghostAep
        StuckKind = $stuckKind
        Verdict = $verdict
        Notes = @($notes.ToArray())
    }
}

# ── Registry (devices.json) ──────────────────────────────────────────────────

# FAKESCAN_REGISTRY overrides the registry file (tests and sandboxes); same rule as fakescan.exe.
$script:RegistryPath = if ($env:FAKESCAN_REGISTRY) { $env:FAKESCAN_REGISTRY } `
    else { Join-Path $env:LOCALAPPDATA 'fakescan\devices.json' }

function Get-FakescanRegistry {
    <#
    .SYNOPSIS
      Load %LOCALAPPDATA%\fakescan\devices.json (empty registry if absent).
    .NOTES
      Port policy: fakescan uses EVEN ports (3920, 3922, …).
      Odd ports (3921, 3923, …) are fakeprint — never listed here.
    #>
    [CmdletBinding()]
    param()
    if (-not (Test-Path -LiteralPath $script:RegistryPath)) {
        return [pscustomobject]@{
            advertise_default = $null
            port_policy = [pscustomobject]@{ base = 3920; step = 2 }
            devices = @()
        }
    }
    $raw = Get-Content -LiteralPath $script:RegistryPath -Raw -Encoding UTF8
    $r = $raw | ConvertFrom-Json
    # Normalize devices to an array so downstream code works under StrictMode.
    if ($null -eq $r.devices) { $r | Add-Member -NotePropertyName devices -NotePropertyValue @() }
    return $r
}

$script:RegistryLockHeld = $false

function Invoke-FakescanRegistryLocked {
    <#
    .SYNOPSIS
      Run a script block holding the cross-process registry lock (devices.json.lock).
    .DESCRIPTION
      Compatible with the Rust store (registry.rs RegistryStore::update), which takes std
      File::lock() = LockFileEx exclusive over the whole range of `<registry>.lock`. Here the lock
      file is opened with FileShare ReadWrite|Delete (NOT FileShare.None: that would make the
      daemon's own open of the lock file fail with a sharing violation instead of waiting) and the
      whole range is locked with FileStream.Lock, retried with backoff for ~5s. Re-entrant within
      this process.
    #>
    [CmdletBinding()]
    param([Parameter(Mandatory)][scriptblock]$ScriptBlock, [int]$TimeoutMs = 5000)
    if ($script:RegistryLockHeld) { return & $ScriptBlock }
    $dir = Split-Path -Parent $script:RegistryPath
    if ($dir -and -not (Test-Path -LiteralPath $dir)) {
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
    }
    $lockPath = "$($script:RegistryPath).lock"
    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMs)
    $delay = 15
    $fs = $null
    while ($true) {
        try {
            $fs = [System.IO.FileStream]::new($lockPath, [System.IO.FileMode]::OpenOrCreate,
                [System.IO.FileAccess]::ReadWrite,
                ([System.IO.FileShare]::ReadWrite -bor [System.IO.FileShare]::Delete))
            try {
                $fs.Lock(0, [long]::MaxValue)
                break
            } catch {
                $fs.Dispose(); $fs = $null
                throw
            }
        } catch [System.IO.IOException] {
            if ([DateTime]::UtcNow -ge $deadline) {
                throw "registry lock busy for ${TimeoutMs}ms: $lockPath ($($_.Exception.Message))"
            }
            Start-Sleep -Milliseconds $delay
            $delay = [Math]::Min($delay * 2, 250)
        }
    }
    $script:RegistryLockHeld = $true
    try {
        return & $ScriptBlock
    } finally {
        $script:RegistryLockHeld = $false
        try { $fs.Unlock(0, [long]::MaxValue) } catch { }
        $fs.Dispose()
    }
}

function Save-FakescanRegistry {
    <#
    .SYNOPSIS
      Write a registry object back to the registry file (atomic temp+rename, under the lock).
    .NOTES
      Callers doing read-modify-write must hold Invoke-FakescanRegistryLocked around the read too
      (Add/Remove/Set-FakescanDevice* do); a bare Save only serializes the write itself.
    #>
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [pscustomobject]$Registry
    )
    Invoke-FakescanRegistryLocked -ScriptBlock {
        $dir = Split-Path -Parent $script:RegistryPath
        if (-not (Test-Path -LiteralPath $dir)) {
            New-Item -ItemType Directory -Path $dir -Force | Out-Null
        }
        $tmp = "$($script:RegistryPath).tmp.$PID"
        $Registry | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $tmp -Encoding UTF8
        Move-Item -LiteralPath $tmp -Destination $script:RegistryPath -Force
    } | Out-Null
}

function Get-FakescanDeviceList {
    <#
    .SYNOPSIS
      List all rows in the fakescan device registry.
    #>
    [CmdletBinding()]
    param()
    $reg = Get-FakescanRegistry
    return @($reg.devices)
}

function Add-FakescanDevice {
    <#
    .SYNOPSIS
      Add or update a row in the fakescan device registry.
    .NOTES
      Port must be even (3920, 3922, …); odd ports cause an error (those are fakeprint).
    #>
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)][string]$Name,
        [string]$Listen,
        [string]$Advertise = '',
        [string]$DeviceUuid = '00000000-0000-4000-8000-0000000000f2',
        [string]$Profile = 'brother-wsd',
        [string]$Exe,
        [switch]$Autostart
    )

    return Invoke-FakescanRegistryLocked -ScriptBlock {
        $reg = Get-FakescanRegistry
        $devices = [System.Collections.Generic.List[object]]::new()
        foreach ($d in @($reg.devices)) { [void]$devices.Add($d) }
    
        if ([string]::IsNullOrWhiteSpace($Listen)) {
            $used = @($devices | ForEach-Object {
                if ($_.listen -match ':(\d+)$') { [int]$Matches[1] }
            })
            $base = if ($reg.port_policy -and $reg.port_policy.base) { [int]$reg.port_policy.base } else { 3920 }
            $step = if ($reg.port_policy -and $reg.port_policy.step) { [int]$reg.port_policy.step } else { 2 }
            $port = $base
            while ($used -contains $port) { $port += $step }
            $Listen = "0.0.0.0:$port"
        }
    
        # Validate even port.
        if ($Listen -match ':(\d+)$') {
            $p = [int]$Matches[1]
            if ($p % 2 -ne 0) {
                throw "fakescan uses even ports (3920, 3922, …); $p is odd — odd ports are fakeprint"
            }
        }
    
        $row = [pscustomobject]@{
            name        = $Name
            listen      = $Listen
            advertise   = $Advertise
            device_uuid = $DeviceUuid
            profile     = $Profile
            autostart   = [bool]$Autostart
            added       = [pscustomobject]@{ pnp = $false }
        }
        if (-not [string]::IsNullOrWhiteSpace($Exe)) {
            $row | Add-Member -NotePropertyName exe -NotePropertyValue $Exe
        }
    
        $replaced = $false
        for ($i = 0; $i -lt $devices.Count; $i++) {
            if ($devices[$i].name -eq $Name) {
                $devices[$i] = $row
                $replaced = $true
                break
            }
        }
        if (-not $replaced) { [void]$devices.Add($row) }
        $reg | Add-Member -NotePropertyName devices -NotePropertyValue $devices.ToArray() -Force
        Save-FakescanRegistry -Registry $reg
        [pscustomobject]@{ Action = if ($replaced) { 'updated' } else { 'added' }; Name = $Name; Listen = $Listen }
    }
}

function Remove-FakescanDevice {
    <#
    .SYNOPSIS
      Remove a row from the fakescan device registry (never touches fakeprint).
    #>
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)][string]$Name
    )

    return Invoke-FakescanRegistryLocked -ScriptBlock {
        $reg = Get-FakescanRegistry
        $before = @($reg.devices).Count
        $reg | Add-Member -NotePropertyName devices `
            -NotePropertyValue @($reg.devices | Where-Object { $_.name -ne $Name }) `
            -Force
        if (@($reg.devices).Count -lt $before) {
            Save-FakescanRegistry -Registry $reg
            Write-Host "removed: $Name"
            return $true
        }
        Write-Host "not found: $Name"
        return $false
    }
}

function Set-FakescanDeviceAdded {
    <#
    .SYNOPSIS
      Mark a registry row as PnP-added (added.pnp = $true).
    #>
    [CmdletBinding()]
    param(
        [string]$Name = 'default',
        [bool]$Pnp = $true
    )

    return Invoke-FakescanRegistryLocked -ScriptBlock {
        $reg = Get-FakescanRegistry
        $found = $false
        foreach ($d in @($reg.devices)) {
            if ($d.name -eq $Name) {
                # Keep the install lifecycle in sync (legacy mark-added claims no WIA details).
                if ($null -eq $d.install) {
                    $d | Add-Member -NotePropertyName install -NotePropertyValue ([pscustomobject]@{ state = 'new' })
                }
                if ($Pnp) {
                    $d.install | Add-Member -NotePropertyName state -NotePropertyValue 'installed' -Force
                    $d.install | Add-Member -NotePropertyName note -NotePropertyValue 'legacy mark-added' -Force
                    $d.install | Add-Member -NotePropertyName installed_at -NotePropertyValue ([DateTime]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ssZ')) -Force
                } else {
                    $d.install | Add-Member -NotePropertyName state -NotePropertyValue 'stale' -Force
                    $d.install | Add-Member -NotePropertyName reason -NotePropertyValue 'marked not added' -Force
                }
                if ($null -eq $d.added) {
                    $d | Add-Member -NotePropertyName added -NotePropertyValue ([pscustomobject]@{ pnp = $Pnp })
                } else {
                    $d.added | Add-Member -NotePropertyName pnp -NotePropertyValue $Pnp -Force
                }
                $found = $true
                break
            }
        }
        if (-not $found) {
            throw "registry row not found: $Name"
        }
        Save-FakescanRegistry -Registry $reg
        [pscustomobject]@{ Name = $Name; Pnp = $Pnp }
    }
}

# ── Unified ctl endpoint ──────────────────────────────────────────────────────

function Get-FakescanCtlInfo {
    <#
    .SYNOPSIS
      GET /v1/ctl — return available commands and running instance identity.
    #>
    [CmdletBinding()]
    param([string]$BaseUrl = $script:DefaultBaseUrl)
    $BaseUrl = Resolve-FakescanBaseUrl $BaseUrl
    Invoke-RestMethod -Uri "$BaseUrl/v1/ctl" -TimeoutSec 5
}

function Invoke-FakescanCtl {
    <#
    .SYNOPSIS
      POST /v1/ctl {"cmd": CMD, "args": ARGS} — send a unified ctl command.
    .EXAMPLE
      Invoke-FakescanCtl -Cmd pages -Args @{count=3}
      Invoke-FakescanCtl -Cmd reset
      Invoke-FakescanCtl -Cmd save
    #>
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)][string]$Cmd,
        [hashtable]$Args = @{},
        [string]$BaseUrl = $script:DefaultBaseUrl
    )
    $BaseUrl = Resolve-FakescanBaseUrl $BaseUrl
    $body = @{ cmd = $Cmd }
    if ($Args.Count -gt 0) { $body.args = $Args }
    Invoke-RestMethod -Method Post -Uri "$BaseUrl/v1/ctl" `
        -ContentType 'application/json' `
        -Body ($body | ConvertTo-Json -Depth 6 -Compress) `
        -TimeoutSec 15
}

# ── ensure ────────────────────────────────────────────────────────────────────

function Get-FakescanRowValue {
    # Strict-mode-safe property read (registry rows omit optional fields).
    param($Row, [string]$Prop)
    if ($null -eq $Row) { return $null }
    $pr = $Row.PSObject.Properties[$Prop]
    if ($null -eq $pr) { return $null }
    return $pr.Value
}

function Get-FakescanEnsurePlan {
    <#
    .SYNOPSIS
      Pure planner for ensure: one process per distinct `listen` in the registry.

    .DESCRIPTION
      Registry rows are one per emulated device; rows sharing a `listen` belong to ONE
      `fakescan --manage` process (docs/multi-device.md section 8). Returns one plan object per
      distinct listen: Listen, Port, Names (every row on that listen), Exe, Args, Skip (reason or $null).
      With -All only listens that have at least one autostart row are planned; otherwise the
      listen of row -Name is planned. Starts nothing and touches no network.
    #>
    [CmdletBinding()]
    param(
        [string]$Name = 'default',
        [switch]$All
    )
    $reg = Get-FakescanRegistry
    $rows = @($reg.devices)
    [object[]]$seedRows = @()
    if ($All) {
        $seedRows = @($rows | Where-Object { Get-FakescanRowValue $_ 'autostart' })
    } else {
        $seedRows = @($rows | Where-Object { $_.name -eq $Name } | Select-Object -First 1)
    }
    $plans = [System.Collections.Generic.List[object]]::new()
    $seen = @{}
    foreach ($seed in $seedRows) {
        if (-not $seed) { continue }
        $sl = Get-FakescanRowValue $seed 'listen'
        $listen = if ($sl) { ([string]$sl).Trim() } else { '0.0.0.0:3920' }
        $key = $listen.ToLowerInvariant()
        if ($seen.ContainsKey($key)) { continue }
        $seen[$key] = $true
        $group = @($rows | Where-Object {
            $gl = Get-FakescanRowValue $_ 'listen'
            $l = if ($gl) { ([string]$gl).Trim().ToLowerInvariant() } else { '0.0.0.0:3920' }
            $l -eq $key
        })
        $port = if ($listen -match ':(\d+)$') { [int]$Matches[1] } else { 3920 }
        $exe = $null; $adv = $null; $profilesDir = $null
        foreach ($g in $group) {
            if (-not $exe) { $exe = Get-FakescanRowValue $g 'exe' }
            if (-not $adv) { $adv = Get-FakescanRowValue $g 'advertise' }
            if (-not $profilesDir) { $profilesDir = Get-FakescanRowValue $g 'profiles' }
        }
        if (-not $profilesDir) {
            $cand = Join-Path (Split-Path -Parent $PSScriptRoot) 'profiles'
            if (Test-Path -LiteralPath $cand) { $profilesDir = $cand }
        }
        $procArgs = @('--manage', '--listen', $listen)
        if ($adv) { $procArgs += '--advertise', $adv }
        if ($profilesDir) { $procArgs += '--profiles', $profilesDir }
        # Windows default is Hello-only (dasHost owns :3702); --probe-listen unless a row opts out.
        $wantProbe = $true
        foreach ($g in $group) {
            $pl = Get-FakescanRowValue $g 'probe_listen'
            if ($null -ne $pl -and -not [bool]$pl) { $wantProbe = $false }
        }
        if ($wantProbe) { $procArgs += '--probe-listen' }
        $skip = $null
        if ([string]::IsNullOrWhiteSpace($exe)) {
            $skip = "no exe in registry for listen $listen; add with: fakescan.ps1 add -Name <row> -Exe PATH"
        } elseif (-not (Test-Path -LiteralPath $exe)) {
            $skip = "exe not found: $exe"
        }
        [void]$plans.Add([pscustomobject]@{
            Listen = $listen
            Port   = $port
            Names  = @($group | ForEach-Object { $_.name })
            Exe    = $exe
            Args   = $procArgs
            Skip   = $skip
        })
    }
    return $plans.ToArray()
}

function Start-FakescanEnsure {
    <#
    .SYNOPSIS
      Make sure the fakescan daemon is running for the registry (one process per distinct listen).

    .DESCRIPTION
      Attach-first: if /health on a listen already answers, do nothing for that listen. Otherwise
      start `exe --manage --listen L [--advertise A] --profiles P` once for ALL rows sharing L (the
      process brings every row up with its stored UUID). Never kills or restarts a running
      instance (that would break Windows PnP / WIA). Never touches fakeprint.
      -WhatIf prints the planned command lines and starts nothing.
    #>
    [CmdletBinding()]
    param(
        [string]$Name = 'default',
        [switch]$All,
        [switch]$WhatIf,
        [switch]$Loud,
        [switch]$Quiet
    )

    [object[]]$plans = @(Get-FakescanEnsurePlan -Name $Name -All:$All)
    if ($plans.Count -eq 0) {
        if ($All) { Write-Host 'ensure: no autostart rows in registry' }
        else { Write-Warning "ensure: no registry row for $Name" }
        return
    }

    foreach ($p in $plans) {
        $baseUrl = "http://127.0.0.1:$($p.Port)"
        $label = "listen $($p.Listen) [$($p.Names -join ', ')]"

        $health = $null
        try {
            $health = Invoke-WebRequest -Uri "$baseUrl/health" -UseBasicParsing -TimeoutSec 3 -ErrorAction Stop
        } catch { }
        $running = ($health -and $health.StatusCode -eq 200 -and $health.Content.Trim() -eq 'ok')

        if ($running) {
            Write-Host "fakescan $label already running - attaching (nothing started)"
            if ($WhatIf) { continue }
            try {
                Get-FakescanCtlInfo -BaseUrl $baseUrl | ConvertTo-Json -Depth 4 | Write-Host
            } catch {
                try {
                    Invoke-RestMethod -Uri "$baseUrl/v1/status" -TimeoutSec 5 | ConvertTo-Json -Depth 4 | Write-Host
                } catch {
                    Write-Warning "fakescan ${label}: attached via /health but status/ctl failed: $_"
                }
            }
            foreach ($n in $p.Names) { Complete-FakescanPnpIfNeeded -Name $n -Loud:$Loud -Quiet:$Quiet }
            continue
        }

        if ($p.Skip) {
            Write-Warning "ensure/${label}: $($p.Skip)"
            continue
        }
        if ($WhatIf) {
            Write-Host ("WhatIf: would start fakescan {0}: {1} {2}" -f $label, $p.Exe, ($p.Args -join ' '))
            continue
        }

        Write-Host "starting fakescan $label -> $($p.Exe) $($p.Args -join ' ')"
        Start-Process -FilePath $p.Exe -ArgumentList $p.Args -WindowStyle Hidden
        $deadline = (Get-Date).AddSeconds(8)
        $up = $false
        while ((Get-Date) -lt $deadline) {
            Start-Sleep -Milliseconds 400
            try {
                $h = Invoke-WebRequest -Uri "$baseUrl/health" -UseBasicParsing -TimeoutSec 2 -ErrorAction Stop
                if ($h.StatusCode -eq 200 -and $h.Content.Trim() -eq 'ok') { $up = $true; break }
            } catch { }
        }
        if ($up) {
            Write-Host "fakescan $label started"
            foreach ($n in $p.Names) { Complete-FakescanPnpIfNeeded -Name $n -Loud:$Loud -Quiet:$Quiet }
        } else {
            Write-Warning "fakescan ${label}: started but /health did not answer in 8s"
        }
    }
}

function Complete-FakescanPnpIfNeeded {
    <#
    .SYNOPSIS
      Resolve added.pnp for a running instance: detect WIA, try Associate, else attention.
    .DESCRIPTION
      Order:
        1. Registry added.pnp already true → nothing
        2. Fakescan already in WIA → mark-added (no UI)
        3. Elevated session → Invoke-FakescanPnpxAssociate; mark-added on success
        4. Else Open-FakescanAddDevice (attention + Settings)
      Never kills fakeprint. Does not remove an existing association to "prove" Associate.
    #>
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)][string]$Name,
        [switch]$Loud,
        [switch]$Quiet
    )

    $reg = Get-FakescanRegistry
    $row = @($reg.devices | Where-Object { $_.name -eq $Name } | Select-Object -First 1)
    if (-not $row) { return }
    $row = $row[0]

    $pnpDone = $false
    try {
        if ($row.added -and $row.added.pnp -eq $true) { $pnpDone = $true }
    } catch { }
    if ($pnpDone) { return }

    $wia = $null
    try { $wia = Get-FakescanWiaDevices } catch { }
    if ($wia -and $wia.FakescanInWia) {
        Write-Host "fakescan/${Name}: already in WIA - marking added.pnp=true"
        Set-FakescanDeviceAdded -Name $Name -Pnp $true | Out-Null
        return
    }

    # Skip Associate when FD has no Installable fakescan hit (common on same-host
    # advertise even with Hellos + --probe-listen). Fall straight through to person Add.
    $fdInstallable = $false
    try {
        $fd = Get-FakescanFunctionDiscovery -TimeoutMs 8000
        $fdInstallable = [bool]$fd.InstallableFakescan
        if (-not $fdInstallable) {
            Write-Host ("fakescan/${Name}: FD InstallableFakescan=False (Hits={0}, FakescanHits={1}) - skipping Associate." -f `
                @($fd.Hits).Count, @($fd.FakescanHits).Count)
            Write-Host '  Same-host advertise often stays FD-invisible; person Add device is the path.'
        }
    } catch {
        Write-Warning ("fakescan/${Name}: FD query failed ({0}); will try Associate if elevated." -f $_)
        $fdInstallable = $true  # don't skip Associate solely because FD query failed
    }

    $elevated = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
        [Security.Principal.WindowsBuiltInRole]::Administrator)
    if ($elevated -and $fdInstallable) {
        Write-Host "fakescan/${Name}: added.pnp=false - trying IPNPXDeviceAssociation::Associate (elevated) ..."
        $assoc = Invoke-FakescanPnpxAssociate
        if ($assoc -and $assoc.Ok -and $assoc.FakescanInWiaAfter) {
            Write-Host "fakescan/${Name}: Associate succeeded - marking added.pnp=true"
            Set-FakescanDeviceAdded -Name $Name -Pnp $true | Out-Null
            return
        }
        if ($assoc -and @($assoc.Results).Count -gt 0) {
            Write-Warning ("fakescan/${Name}: Associate ran but WIA not ready. Results: {0}" -f (
                ($assoc.Results | ConvertTo-Json -Compress -Depth 4)))
        } else {
            Write-Warning "fakescan/${Name}: Associate found no FD instance (need uninstalled device sending Hellos)."
        }
    }

    Write-Host "fakescan/${Name}: added.pnp=false - opening Add device (attention follows)."
    Open-FakescanAddDevice -Loud:$Loud -Quiet:$Quiet
}

function Trace-FakescanWiaConnect {
    <#
    .SYNOPSIS
      Connect the Fake WIA device and say whether fakescan saw any WSD call.

    .DESCRIPTION
      Prints the COM HRESULT and elapsed time. When this fakescan exposes
      GET /v1/trace, also prints WSD calls that arrived during Connect.
      Zero calls and a ~16 ms 0x80210001 means Windows failed before SOAP
      (sticky offline). Listing devices is not a Connect.
    #>
    [CmdletBinding()]
    param(
        [string]$BaseUrl = 'http://127.0.0.1:3920',
        [string]$NameLike = 'Fake*'
    )

    $BaseUrl = $BaseUrl.TrimEnd('/')
    $before = $null
    try {
        $before = Invoke-RestMethod -Uri "$BaseUrl/v1/trace" -TimeoutSec 3
    } catch {
        Write-Host "trace=unavailable ($($_.Exception.Message))"
        Write-Host 'hint=restart fakescan to enable GET /v1/activity, /v1/jobs, and /v1/log'
    }

    $dm = New-Object -ComObject WIA.DeviceManager
    $info = $null
    foreach ($d in $dm.DeviceInfos) {
        $n = ''
        try { $n = [string]$d.Properties.Item('Name').Value } catch { continue }
        if ($n -like $NameLike) { $info = $d; break }
    }
    if (-not $info) {
        Write-Host "device=missing filter=$NameLike"
        return
    }
    $deviceName = [string]$info.Properties.Item('Name').Value
    Write-Host "device=$deviceName"

    $sw = [Diagnostics.Stopwatch]::StartNew()
    $ok = $false
    $hresults = @()
    try {
        $null = $info.Connect()
        $ok = $true
    } catch {
        $ex = $_.Exception
        while ($null -ne $ex) {
            $code = '{0:X8}' -f ($ex.HResult -band 0xFFFFFFFF)
            $hresults += "$code $($ex.GetType().Name): $($ex.Message)"
            $ex = $ex.InnerException
        }
    }
    $ms = $sw.ElapsedMilliseconds
    if ($ok) {
        Write-Host "connect=ok elapsed_ms=$ms"
    } else {
        Write-Host "connect=fail elapsed_ms=$ms"
        $hresults | ForEach-Object { Write-Host "hresult=$_" }
    }

    if ($null -eq $before) { return }
    $after = Invoke-RestMethod -Uri "$BaseUrl/v1/trace?after=$($before.seq)" -TimeoutSec 3
    $events = @($after.events)
    Write-Host "wsd_during_connect=$($events.Count)"
    foreach ($ev in $events) {
        Write-Host ("  seq={0} {1} {2} {3}ms {4} {5}" -f $ev.seq, $ev.peer, $ev.action, $ev.ms, $ev.outcome, $ev.detail)
    }
    if (-not $ok -and $events.Count -eq 0) {
        Write-Host 'verdict=Windows failed before ScannerService (sticky offline Connect). Keep this fakescan up and re-Add the device.'
    } elseif (-not $ok) {
        Write-Host 'verdict=ScannerService ran and Connect still failed. Read the actions above; the fault is in the SOAP exchange, not a silent device.'
    }
}

Export-ModuleMember -Function @(
    'Get-FakescanHealth',
    'Get-FakescanStatus',
    'Set-FakescanProfile',
    'Set-FakescanFeeder',
    'Set-FakescanPages',
    'Set-FakescanFault',
    'Get-FakescanScenarios',
    'Set-FakescanScenario',
    'Reset-Fakescan',
    'Invoke-FakescanSoap',
    'Get-FakescanElements',
    'Test-FakescanSoap',
    'Find-FakescanDiscovery',
    'Find-FakescanHello',
    'Test-FakescanLooksLike',
    'Test-FakescanFunctionDiscoveryRegistered',
    'Get-FakescanFunctionDiscovery',
    'Find-FakescanFunctionDiscovery',
    'Get-FakescanAssociationEndpoints',
    'Connect-FakescanAssociationEndpoint',
    'Invoke-FakescanPnpxAssociate',
    'Disconnect-FakescanAssociationEndpoint',
    'Restart-FakescanDiscoveryServices',
    'Invoke-FakescanDiscoveryRestart',
    'Invoke-FakescanPairToolDisassociate',
    'Get-FakescanSwdDevices',
    'Test-FakescanDiscoverable',
    'Get-FakescanWiaDevices',
    'Trace-FakescanWiaConnect',
    'Open-FakescanAddDevice',
    'Complete-FakescanPnpIfNeeded',
    'Show-FakescanBanner',
    'Start-FakescanElevatedShell',
    'Start-FakescanElevatedHelper',
    'Watch-FakescanElevatedHelper',
    'Invoke-FakescanElevatedHelper',
    'Get-FakescanElevatedHelperSession',
    'Get-FakescanElevatedHelperDir',
    'Test-FakescanProcessElevated',
    'Install-FakescanScanner',
    'Remove-FakescanScanner',
    'Remove-FakescanGhost',
    'Get-FakescanGhostNodes',
    'Get-FakescanPnpRemoveTargets',
    'Repair-FakescanScanner',
    # Lab instance control (new)
    'Get-FakescanRegistry',
    'Save-FakescanRegistry',
    'Get-FakescanDeviceList',
    'Add-FakescanDevice',
    'Remove-FakescanDevice',
    'Set-FakescanDeviceAdded',
    'Get-FakescanCtlInfo',
    'Invoke-FakescanCtl',
    'Get-FakescanEnsurePlan',
    'Invoke-FakescanRegistryLocked',
    'Start-FakescanEnsure'
)
