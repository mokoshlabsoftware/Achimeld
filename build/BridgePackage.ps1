[CmdletBinding()]
param([switch]$Development, [switch]$Offline, [switch]$PassThru)
$ErrorActionPreference = 'Stop'
if (!$Development) { throw 'Only explicit development packages are supported.' }
$repositoryRoot = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'BuildPaths.ps1')
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
if (([Security.Principal.WindowsPrincipal]::new($identity)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'Build as a normal user; only installation is elevated.' }
Initialize-AchimeldBuildPaths
$package = Join-Path $env:ACHIMELD_ARTIFACT_ROOT ('bridge-package-' + [Guid]::NewGuid().ToString('N'))
& (Join-Path $PSScriptRoot 'Build-ReadWorker.ps1') -OutputDirectory $package -Offline:$Offline
& (Join-Path $PSScriptRoot 'Setup-Browser.ps1') -PackageDirectory $package -Offline:$Offline
$provenance = [ordered]@{
    schemaVersion=2; workerPolicy='reviewed-sync-v1';
    applicationSource='All application-owned code is built from this repository; no binary application inputs.';
    runtimeLock='build/browser-runtime.lock.json';
    workerBuild='read-worker-provenance.json'; browserBuild='browser-provenance.json';
    note='Third-party .NET, Playwright, Node and Chromium identities are locked separately from application source.'
}
[IO.File]::WriteAllText((Join-Path $package 'bridge-provenance.json'),($provenance | ConvertTo-Json)+[Environment]::NewLine,[Text.UTF8Encoding]::new($false))
$env:DOTNET_CLI_HOME=Join-Path $env:ACHIMELD_CACHE_ROOT 'dotnet-home'
$env:NUGET_PACKAGES=Join-Path $env:ACHIMELD_CACHE_ROOT 'nuget'
$env:DOTNET_CLI_TELEMETRY_OPTOUT='1'
$env:DOTNET_GENERATE_ASPNET_CERTIFICATE='false'
& dotnet run --project (Join-Path $repositoryRoot 'tools/bridge-package/BridgePackage.csproj') -- --development $repositoryRoot $package
if ($LASTEXITCODE -ne 0) { throw 'Development signing/build failed.' }
$package = & (Join-Path $PSScriptRoot 'Publish-CurrentBuild.ps1') -PackagePath $package
Write-Output ('Verified development package: '+$package)
Write-Output ('Expected verifier SHA-256 (retain from this trusted build): '+(Get-FileHash -LiteralPath (Join-Path $package 'Achimeld.exe') -Algorithm SHA256).Hash.ToLowerInvariant())
[IO.File]::WriteAllText((Join-Path $env:ACHIMELD_ARTIFACT_ROOT 'last-bridge-package.txt'),$package,[Text.UTF8Encoding]::new($false))
if ($PassThru) {
    [pscustomobject]@{ PackagePath=$package; VerifierSha256=(Get-FileHash -LiteralPath (Join-Path $package 'Achimeld.exe') -Algorithm SHA256).Hash.ToLowerInvariant() }
}
