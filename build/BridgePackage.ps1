[CmdletBinding()]
param([switch]$Development, [switch]$Offline, [switch]$PassThru, [string]$PublicationReceipt)
$ErrorActionPreference = 'Stop'
if (!$Development) { throw 'Only explicit development packages are supported.' }
$repositoryRoot = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'BuildPaths.ps1')
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
if (([Security.Principal.WindowsPrincipal]::new($identity)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'Build as a normal user; only installation is elevated.' }
Initialize-AchimeldBuildPaths
if ($PublicationReceipt) {
    $receiptPath = [IO.Path]::GetFullPath($PublicationReceipt)
    $receiptDirectory = Split-Path -Parent $receiptPath
    if ((Split-Path -Parent $receiptDirectory) -ine [IO.Path]::GetFullPath($env:ACHIMELD_ARTIFACT_ROOT) -or
        (Split-Path -Leaf $receiptDirectory) -cnotmatch '^github-release-[a-f0-9]{32}$' -or
        (Split-Path -Leaf $receiptPath) -cne 'package-result.json' -or
        !(Test-Path -LiteralPath $receiptDirectory -PathType Container) -or
        ((Get-Item -LiteralPath $receiptDirectory -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw 'Publication receipt must be in the active release staging directory.'
    }
}
& (Join-Path $PSScriptRoot 'Clear-PreviousPackages.ps1') | Out-Host
$package = Join-Path $env:ACHIMELD_ARTIFACT_ROOT ('bridge-package-' + [Guid]::NewGuid().ToString('N'))
& (Join-Path $PSScriptRoot 'Build-ReadWorker.ps1') -OutputDirectory $package -Offline:$Offline
& (Join-Path $PSScriptRoot 'Setup-Browser.ps1') -PackageDirectory $package -Offline:$Offline
if ($PublicationReceipt) {
    # Worker and browser setup reinitialize the shared build paths in this shell.
    # Set the isolated target afterward so the signer and Rust child agree.
    $env:ACHIMELD_RELEASE_TARGET = Join-Path $env:ACHIMELD_ARTIFACT_ROOT ('release-target-' + [Guid]::NewGuid().ToString('N'))
    $env:CARGO_TARGET_DIR = $env:ACHIMELD_RELEASE_TARGET
}
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
# Each invocation owns its signer binaries: another build may still be running
# its signer while waiting for Rust. Never overwrite that process's executable.
$signerArtifacts = Join-Path $env:ACHIMELD_ARTIFACT_ROOT ('bridge-signer-' + [Guid]::NewGuid().ToString('N'))
try {
    & dotnet build (Join-Path $repositoryRoot 'tools/bridge-package/BridgePackage.csproj') "-p:ArtifactsPath=$signerArtifacts"
    if ($LASTEXITCODE -ne 0) { throw 'Development signing tool build failed.' }
    & dotnet (Join-Path $signerArtifacts 'bin/BridgePackage/debug/BridgePackage.dll') --development $repositoryRoot $package
    if ($LASTEXITCODE -ne 0) { throw 'Development signing/build failed.' }
} finally {
    & (Join-Path $PSScriptRoot 'Clear-PreviousPackages.ps1') -TransientPath $signerArtifacts | Out-Host
    if ($PublicationReceipt) {
        & (Join-Path $PSScriptRoot 'Clear-PreviousPackages.ps1') -TransientPath $env:ACHIMELD_RELEASE_TARGET | Out-Host
    }
}
if ($PublicationReceipt) {
    $releasePackage = Join-Path $receiptDirectory 'verified-package'
    if (Test-Path -LiteralPath $releasePackage) { throw 'Publication package staging is already occupied.' }
    Move-Item -LiteralPath $package -Destination $releasePackage
    $package = $releasePackage
    $releaseManifest = Get-Content -LiteralPath (Join-Path $package 'manifest.json') -Raw | ConvertFrom-Json
    @{ schemaVersion = 2; packagePath = $package; sourceCommit = $releaseManifest.sourceCommit } |
        ConvertTo-Json | Set-Content -LiteralPath $receiptPath -Encoding utf8
} else {
    $published = & (Join-Path $PSScriptRoot 'Publish-CurrentBuild.ps1') -PackagePath $package -PassThru
    $package = $published.PackagePath
    [IO.File]::WriteAllText((Join-Path $env:ACHIMELD_ARTIFACT_ROOT 'last-bridge-package.txt'),$package,[Text.UTF8Encoding]::new($false))
}
Write-Output ('Verified development package: '+$package)
Write-Output ('Expected verifier SHA-256 (retain from this trusted build): '+(Get-FileHash -LiteralPath (Join-Path $package 'Achimeld.exe') -Algorithm SHA256).Hash.ToLowerInvariant())
if ($PassThru) {
    [pscustomobject]@{ PackagePath=$package; VerifierSha256=(Get-FileHash -LiteralPath (Join-Path $package 'Achimeld.exe') -Algorithm SHA256).Hash.ToLowerInvariant() }
}
