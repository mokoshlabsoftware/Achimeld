[CmdletBinding()]
param([Parameter(Mandatory)][string]$WorkerDirectory, [switch]$Offline)
$ErrorActionPreference = 'Stop'
$repositoryRoot = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'BuildPaths.ps1')
Initialize-AchimeldBuildPaths
$env:DOTNET_CLI_HOME = Join-Path $env:ACHIMELD_CACHE_ROOT 'dotnet-home'
$env:NUGET_PACKAGES = Join-Path $env:ACHIMELD_CACHE_ROOT 'nuget'
$env:DOTNET_CLI_TELEMETRY_OPTOUT = '1'
& (Join-Path $PSScriptRoot 'Setup-Browser.ps1') -PackageDirectory $WorkerDirectory -Offline:$Offline
$env:ACHIMELD_TEST_PLAYWRIGHT_WORKER = [IO.Path]::GetFullPath($WorkerDirectory)
try {
    & (Join-Path $PSScriptRoot 'Rust.ps1') -CargoArguments @('test','-p','achimeld-app','--bin','achimeld-node-host','--locked','--offline','stdio_driver_playwright_','--','--ignored')
} finally { Remove-Item Env:ACHIMELD_TEST_PLAYWRIGHT_WORKER -ErrorAction SilentlyContinue }
& dotnet run --project (Join-Path $repositoryRoot 'tools/browser-policy-tests/BrowserPolicyTests.csproj') -c Release --no-restore -- --browser-fixtures $WorkerDirectory
if ($LASTEXITCODE -ne 0) { throw 'Synthetic browser request fixtures failed.' }
