[CmdletBinding()]
param([Parameter(Mandatory)][string]$PackagePath, [switch]$PolicyRemovalProbe)
$ErrorActionPreference = 'Stop'
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
if ((New-Object Security.Principal.WindowsPrincipal($identity)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Run installed acceptance tests as a normal user, after administrator installation.'
}
$root = (Resolve-Path -LiteralPath $PackagePath).Path
$boundary = Join-Path ([Environment]::GetFolderPath([Environment+SpecialFolder]::ProgramFiles)) 'Achimeld-Development'
if (!$root.StartsWith([IO.Path]::GetFullPath($boundary) + '\',[StringComparison]::OrdinalIgnoreCase)) { throw 'Protected development installation required.' }
$repository = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
. (Join-Path $repository 'build/BuildPaths.ps1')
Initialize-AchimeldBuildPaths
$artifacts = $env:ACHIMELD_ARTIFACT_ROOT
$bootstrapper = Join-Path $root 'Achimeld.exe'
function Package-Processes {
    @(Get-Process | Where-Object { $_.Path -and $_.Path.StartsWith($root + '\',[StringComparison]::OrdinalIgnoreCase) })
}
function Start-HeldProbe {
    $probe = Start-Process -FilePath $bootstrapper -ArgumentList '--self-test-hold' -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $artifacts 'held-test-output.txt') -RedirectStandardError (Join-Path $artifacts 'held-test-error.txt')
    $deadline = [DateTime]::UtcNow.AddSeconds(90)
    do {
        if ($probe.HasExited) { throw 'Supervised probe exited before its child browser started.' }
        $children = Package-Processes | Where-Object Id -ne $probe.Id
        if ($children | Where-Object ProcessName -eq 'chrome') { return @{ Root=$probe; Children=$children } }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    $probe.Kill(); $probe.WaitForExit(); throw 'Private browser did not start within the test deadline.'
}
function Require-ChildrenStopped($Processes) {
    foreach ($process in $Processes) {
        if (!$process.WaitForExit(5000)) { throw ('Supervised child remained alive: ' + $process.ProcessName) }
    }
}
& $bootstrapper --verify
if ($LASTEXITCODE -ne 0) { throw 'Installed signature/inventory rejected.' }
& $bootstrapper --check-policy
if ($LASTEXITCODE -ne 0) { throw 'Normal-user Windows policy verification failed.' }
$writeDenied = $false
try { $writer=[IO.File]::Open((Join-Path $root 'Achimeld.App.exe'),'Open','Write','ReadWrite'); $writer.Dispose() }
catch [UnauthorizedAccessException] { $writeDenied=$true }
if (!$writeDenied) { throw 'Normal user can write the installed editor.' }
Write-Output 'PASS: Protected package is read-only to the application user'
& $bootstrapper --self-test
if ($LASTEXITCODE -ne 0) { throw 'Installed IPv4/IPv6 TCP/UDP and Steam broker acceptance failed.' }
Write-Output 'PASS: Editor/worker direct IPv4/IPv6 TCP/UDP denied; unrelated host rejected; approved worker and private browser Steam access works'
$held = Start-HeldProbe
try {
    # Even an approved CONNECT request from a process outside the Job is refused.
    $client = New-Object Net.Sockets.TcpClient
    try {
        $client.Connect('127.0.0.1',42761)
        $stream=$client.GetStream(); $stream.ReadTimeout=5000
        $request=[Text.Encoding]::ASCII.GetBytes("CONNECT partner.steamgames.com:443 HTTP/1.1`r`nHost: partner.steamgames.com:443`r`n`r`n")
        $response=New-Object byte[] 256; $authorized=$false
        try { $stream.Write($request,0,$request.Length); $length=$stream.Read($response,0,$response.Length); $authorized=[Text.Encoding]::ASCII.GetString($response,0,$length).StartsWith('HTTP/1.1 200') }
        catch [IO.IOException] { }
        if ($authorized) { throw 'An unsupervised process used the broker.' }
        Write-Output 'PASS: Unsupervised broker client refused'
    } finally { $client.Dispose() }
    $held.Root.Kill(); $held.Root.WaitForExit()
    Require-ChildrenStopped $held.Children
    Write-Output 'PASS: Bootstrapper death closes editor, worker, Node and private browser children'
} finally {
    if (!$held.Root.HasExited) { $held.Root.Kill(); $held.Root.WaitForExit() }
    $held.Root.Dispose()
}
& $bootstrapper --check-policy
if ($LASTEXITCODE -ne 0) { throw 'Persistent restrictions were lost on supervisor death.' }
Write-Output 'PASS: Restrictions persist after supervisor death'
if ($PolicyRemovalProbe) {
    # An administrator runs the separately scoped policy-loss helper first.
    # These files carry only fixed test signals, never commands or package paths.
    $held = Start-HeldProbe
    try {
        [IO.File]::WriteAllText((Join-Path $artifacts 'security-policy-remove.request'),'REMOVE FIXED TEST PACKAGE POLICY')
        $deadline=[DateTime]::UtcNow.AddSeconds(20)
        while (!(Test-Path (Join-Path $artifacts 'security-policy-removed.result'))) {
            if ([DateTime]::UtcNow -ge $deadline) { throw 'Administrator policy-loss probe did not respond.' }
            Start-Sleep -Milliseconds 100
        }
        if (!$held.Root.WaitForExit(10000)) { throw 'Supervisor did not stop after policy loss.' }
        if ($held.Root.ExitCode -eq 0) { throw 'Policy loss was reported as successful supervision.' }
        Require-ChildrenStopped $held.Children
        Write-Output 'PASS: Removed mandatory policy terminates the whole supervised session'
    } finally {
        [IO.File]::WriteAllText((Join-Path $artifacts 'security-policy-repair.request'),'RESTORE FIXED TEST PACKAGE POLICY')
        if (!$held.Root.HasExited) { $held.Root.Kill(); $held.Root.WaitForExit() }
        $held.Root.Dispose()
    }
    $deadline=[DateTime]::UtcNow.AddSeconds(30)
    while (!(Test-Path (Join-Path $artifacts 'security-policy-repaired.result'))) {
        if ([DateTime]::UtcNow -ge $deadline) { throw 'Package policy repair did not finish.' }
        Start-Sleep -Milliseconds 100
    }
    if ((Get-Content (Join-Path $artifacts 'security-policy-repaired.result')).Trim() -ne '0') { throw 'Package policy repair failed.' }
    & $bootstrapper --check-policy
    if ($LASTEXITCODE -ne 0) { throw 'Repaired policy verification failed.' }
    Write-Output 'PASS: Package-specific policy restored and verified'
}
