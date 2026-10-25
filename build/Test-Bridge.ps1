[CmdletBinding()]
param()
$ErrorActionPreference='Stop'
$repositoryRoot=Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'BuildPaths.ps1')
Initialize-AchimeldBuildPaths
$env:DOTNET_CLI_HOME=Join-Path $env:ACHIMELD_CACHE_ROOT 'dotnet-home'
$env:NUGET_PACKAGES=Join-Path $env:ACHIMELD_CACHE_ROOT 'nuget'
$env:DOTNET_CLI_TELEMETRY_OPTOUT='1'
$env:DOTNET_GENERATE_ASPNET_CERTIFICATE='false'
$fixture=Join-Path $env:ACHIMELD_ARTIFACT_ROOT 'worker-fixture'
& dotnet build (Join-Path $repositoryRoot 'tools/worker-fixture/WorkerFixture.csproj') -c Release -o $fixture
if($LASTEXITCODE -ne 0){throw 'C# pipe fixture build failed.'}
$env:ACHIMELD_TEST_WORKER=Join-Path $fixture 'WorkerFixture.exe'
try {
    & (Join-Path $PSScriptRoot 'Rust.ps1') -CargoArguments @('test','-p','achimeld-mcp','--locked','--offline','lifetime::tests::mcp_process_exits_with_open_stdin_after_last_editor_closes','--','--ignored','--exact')
    & (Join-Path $PSScriptRoot 'Rust.ps1') -CargoArguments @('test','-p','achimeld-security','--locked','--offline','csharp_','--','--ignored')
    & (Join-Path $PSScriptRoot 'Rust.ps1') -CargoArguments @('test','-p','achimeld-bootstrapper','--locked','--offline','console_app_launch','--','--ignored')
    & (Join-Path $PSScriptRoot 'Rust.ps1') -CargoArguments @('test','-p','achimeld-app','--bin','achimeld-node-host','--locked','--offline','tests::stdio_driver_round_trip_and_health_loss_terminate_child','--','--ignored','--exact')
} finally {Remove-Item Env:ACHIMELD_TEST_WORKER -ErrorAction SilentlyContinue}
