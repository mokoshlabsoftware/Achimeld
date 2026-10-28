[CmdletBinding()]
param([switch]$Offline, [switch]$BuildOnly, [switch]$RunCurrent, [switch]$Protected)
$ErrorActionPreference = 'Stop'
if ($Protected) { throw 'Build and Run launches from current build output only. Use the separate installer for ready builds.' }
$repositoryRoot = Split-Path -Parent $PSScriptRoot
# Existing shortcuts may start PowerShell inside current. Release that directory
# before waiting for the app so build storage can be cleaned while it is open.
Set-Location -LiteralPath $repositoryRoot
[Environment]::CurrentDirectory = $repositoryRoot
. (Join-Path $PSScriptRoot 'BuildPaths.ps1')
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
if (([Security.Principal.WindowsPrincipal]::new($identity)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Start Achimeld normally, without administrator privileges.'
}
Initialize-AchimeldBuildPaths
# The signer invokes pwsh by name. Use the same PowerShell selected by the batch
# file for child build tools, including when it was discovered outside PATH.
$env:PATH = $PSHOME + ';' + $env:PATH
$built = $null
if ($RunCurrent) {
    if ($BuildOnly) { throw 'RunCurrent cannot be combined with BuildOnly.' }
    $package = Join-Path $env:ACHIMELD_ARTIFACT_ROOT 'package'
    $built = [pscustomobject]@{ PackagePath = $package; VerifierSha256 = (Get-FileHash -LiteralPath (Join-Path $package 'Achimeld.exe') -Algorithm SHA256).Hash.ToLowerInvariant() }
} else {
$currentPackage = [IO.Path]::GetFullPath((Join-Path $env:ACHIMELD_ARTIFACT_ROOT 'package')).TrimEnd('\') + '\'
    $running = @(Get-Process -Name 'Achimeld','Achimeld.App','achimeld-app','achimeld-node-host','Achimeld.SteamWorker' -ErrorAction SilentlyContinue |
    Where-Object {
        $path = try { $_.Path } catch { $null }
        $path -and [IO.Path]::GetFullPath($path).StartsWith($currentPackage, [StringComparison]::OrdinalIgnoreCase)
    } | Select-Object -ExpandProperty Id)
if ($running.Count) {
    . (Join-Path $PSScriptRoot 'DevelopmentProcesses.ps1')
    Stop-AchimeldIdleDevelopmentMcp -PackagePath $currentPackage.TrimEnd('\')
    $running = @($running | Where-Object { Get-Process -Id $_ -ErrorAction SilentlyContinue })
}
if ($running.Count) {
    throw "The current development build is still running (process IDs: $($running -join ', ')). Save and close it before building so the verified package can be promoted."
}
Write-Host 'Capturing the current source before the long build...'
$snapshot = & (Join-Path $PSScriptRoot 'New-DevelopmentSourceSnapshot.ps1') -SourceRoot $repositoryRoot
Write-Host 'Building the Rust desktop application and its supervised worker from the captured source...'
& (Join-Path $snapshot 'build/BridgePackage.ps1') -Development -Offline:$Offline -PassThru | ForEach-Object {
    if ($_ -is [pscustomobject] -and $_.PSObject.Properties['PackagePath'] -and $_.PSObject.Properties['VerifierSha256']) { $built = $_ }
    else { Write-Host $_ }
}
if (!$built) { throw 'The current build did not produce a verified development package.' }
}
$expectedPackage = [IO.Path]::GetFullPath((Join-Path $env:ACHIMELD_ARTIFACT_ROOT 'package'))
if (![IO.Path]::GetFullPath($built.PackagePath).Equals($expectedPackage, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Build and Run requires the package in current build output.'
}
& (Join-Path $built.PackagePath 'Achimeld.exe') --verify
if ($LASTEXITCODE -ne 0) { throw 'Current development package verification failed.' }
if (!$RunCurrent) {
    Write-Host 'Cleaning temporary artifacts from the verified build...'
    & (Join-Path $PSScriptRoot 'Clear-CodeArtifacts.ps1')
}
& (Join-Path $PSScriptRoot 'Update-DesktopShortcut.ps1') -CurrentBuildPath $env:ACHIMELD_ARTIFACT_ROOT
if ($BuildOnly) { Write-Host ('Build complete: ' + $built.PackagePath); return }
$manifest = Get-Content -LiteralPath (Join-Path $built.PackagePath 'manifest.json') -Raw | ConvertFrom-Json
if ($manifest.mode -cne 'development' -or $manifest.product -cne 'Achimeld' -or $manifest.version -notmatch '^\d+\.\d+\.\d+$' -or $manifest.keyId -cnotmatch '^[a-f0-9]{64}$') { throw 'Built package identity rejected.' }
$launchRoot = $built.PackagePath
$launchArguments = @{}
$bootstrapper = Join-Path $launchRoot 'Achimeld.exe'
if ((Get-FileHash -LiteralPath $bootstrapper -Algorithm SHA256).Hash.ToLowerInvariant() -cne $built.VerifierSha256) { throw 'Bootstrapper differs from this verified build.' }
Write-Host ('Starting the local test build without installation: ' + $bootstrapper)
try {
    $application = Start-Process -FilePath $bootstrapper -WorkingDirectory $launchRoot -NoNewWindow -Wait -PassThru @launchArguments
} finally {
    . (Join-Path $PSScriptRoot 'DevelopmentProcesses.ps1')
    Stop-AchimeldIdleDevelopmentMcp -PackagePath $launchRoot
}
if ($application.ExitCode -ne 0) { throw 'Achimeld stopped. Check the startup or security diagnostics.' }
