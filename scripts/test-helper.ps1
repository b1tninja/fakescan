<#
.SYNOPSIS
  Offline test of the elevated-helper request/ack protocol (no elevation, no devices touched).
.DESCRIPTION
  Starts Watch-FakescanElevatedHelper -AllowUnelevated in a child PowerShell, then checks that
  `stop` is acknowledged cleanly (no "exited before answering"), that a second stop fails with the
  "not running" message instead of hanging, and that an unsupported command is refused by the
  validator. Uses only whatif/stop: nothing is removed, unpaired or restarted.
  Refuses to run if a real elevated helper is already running.
#>
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'Fakescan.psm1') -Force -DisableNameChecking

$pass = 0
$fail = 0
function Check($name, [bool]$ok, $detail = '') {
    if ($ok) { $script:pass++; Write-Host "ok   $name" }
    else { $script:fail++; Write-Host "FAIL $name $detail" }
}

if (Get-FakescanElevatedHelperSession) {
    Write-Host 'a helper is already running; refusing to interfere'
    exit 2
}

$mod = (Join-Path $PSScriptRoot 'Fakescan.psm1').Replace("'", "''")
$child = Start-Process -FilePath 'powershell.exe' -WindowStyle Hidden -PassThru -ArgumentList @(
    '-NoProfile', '-ExecutionPolicy', 'Bypass', '-Command',
    "Import-Module '$mod' -Force -DisableNameChecking; Watch-FakescanElevatedHelper -AllowUnelevated -IdleSeconds 60")
try {
    $deadline = [datetime]::UtcNow.AddSeconds(20)
    while (-not (Get-FakescanElevatedHelperSession) -and [datetime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 200 }
    Check 'helper session appears' ([bool](Get-FakescanElevatedHelperSession))

    $wi = Invoke-FakescanElevatedHelper -Cmd whatif -TimeoutSeconds 30
    Check 'whatif is answered' ($wi.ok -eq $true)

    $sw = [Diagnostics.Stopwatch]::StartNew()
    $stop = $null
    $err = $null
    try { $stop = Invoke-FakescanElevatedHelper -Cmd stop -TimeoutSeconds 30 } catch { $err = "$_" }
    $sw.Stop()
    Check 'stop is acknowledged without an error' ($null -eq $err -and $stop.ok -eq $true) $err
    Check 'stop returns promptly (<8s)' ($sw.Elapsed.TotalSeconds -lt 8) ("{0:N1}s" -f $sw.Elapsed.TotalSeconds)

    $gone = $child.WaitForExit(10000)
    Check 'helper process exits after the ack' $gone
    Check 'session file is cleaned up' (-not (Get-FakescanElevatedHelperSession))

    $err2 = $null
    try { Invoke-FakescanElevatedHelper -Cmd stop -TimeoutSeconds 5 } catch { $err2 = "$_" }
    Check 'second stop says not running (does not hang)' ($err2 -match 'not running') $err2

    $err3 = $null
    try { Invoke-FakescanElevatedHelper -Cmd 'rm-rf' -TimeoutSeconds 5 } catch { $err3 = "$_" }
    Check 'unsupported command is rejected by the validator' ([bool]$err3)
} finally {
    if ($child -and -not $child.HasExited) { $child.Kill() }
    Remove-Item -LiteralPath (Join-Path (Get-FakescanElevatedHelperDir) 'session.json'),
        (Join-Path (Get-FakescanElevatedHelperDir) 'request.json'),
        (Join-Path (Get-FakescanElevatedHelperDir) 'reply.json') -ErrorAction SilentlyContinue
}
Write-Host ("passed={0} failed={1}" -f $pass, $fail)
if ($fail) { exit 1 }
