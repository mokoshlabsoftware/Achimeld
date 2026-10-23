[CmdletBinding()]
param(
    [string[]]$CargoArguments = @('test', '--workspace', '--locked'),
    [string]$ToolchainRoot,
    [string]$WindowsSdkRoot,
    [string]$AuxiliaryToolsRoot,
    [switch]$BuildRelease
)
$ErrorActionPreference = 'Stop'
$savedEnvironment = [Environment]::GetEnvironmentVariables('Process')
try {
    $repositoryRoot = Split-Path -Parent $PSScriptRoot
    . (Join-Path $PSScriptRoot 'BuildPaths.ps1')
    Initialize-AchimeldBuildPaths
    if ($BuildRelease) { $CargoArguments = @('build','--workspace','--release','--locked','--offline') }
    if (!$ToolchainRoot) {
        $localCompiler = Join-Path $env:ACHIMELD_CACHE_ROOT 'rustup/toolchains/1.99.0-x86_64-pc-windows-msvc'
        if (Test-Path -LiteralPath (Join-Path $localCompiler 'bin/cargo.exe')) { $ToolchainRoot = $localCompiler }
    }
    if (!$WindowsSdkRoot) {
        $localSdk = Join-Path $env:ACHIMELD_CACHE_ROOT 'windows-sdk'
        if (Test-Path -LiteralPath (Join-Path $localSdk 'um/x64/kernel32.lib')) { $WindowsSdkRoot = $localSdk }
    }
    $env:CARGO_HOME = Join-Path $env:ACHIMELD_CACHE_ROOT 'cargo-home'
    New-Item -ItemType Directory -Path $env:CARGO_HOME -Force | Out-Null
    if ($ToolchainRoot) {
        $compilerRoot = (Resolve-Path -LiteralPath $ToolchainRoot).Path
        $cargoPath = Join-Path $compilerRoot 'bin/cargo.exe'
        $env:RUSTC = Join-Path $compilerRoot 'bin/rustc.exe'
        $env:RUSTDOC = Join-Path $compilerRoot 'bin/rustdoc.exe'
        $env:RUSTFMT = Join-Path $compilerRoot 'bin/rustfmt.exe'
        if (Test-Path -LiteralPath (Join-Path $env:ACHIMELD_CACHE_ROOT 'rustup/settings.toml')) {
            $env:RUSTUP_HOME = Join-Path $env:ACHIMELD_CACHE_ROOT 'rustup'
        }
        if (!(Test-Path -LiteralPath $cargoPath) -or !(Test-Path -LiteralPath $env:RUSTC)) {
            throw 'ToolchainRoot must contain the installed Rust MSVC toolchain.'
        }
        $env:PATH = (Join-Path $compilerRoot 'bin') + ';' + $env:PATH
    } else {
        $cargoCommand = Get-Command cargo.exe -ErrorAction SilentlyContinue
        if (!$cargoCommand) { throw 'Install Rust 1.99.0 MSVC or supply -ToolchainRoot.' }
        $cargoPath = $cargoCommand.Source
    }
    $vswherePath = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
    if (!(Test-Path -LiteralPath $vswherePath)) { throw 'Visual C++ x64 build tools are required.' }
    $installation = & $vswherePath -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (!$installation) { throw 'Visual C++ x64 build tools are required.' }
    $vcvarsPath = Join-Path $installation 'VC/Auxiliary/Build/vcvars64.bat'
    $compilerEnvironment = & $env:ComSpec /d /c ('"' + $vcvarsPath + '" >nul && set')
    if ($LASTEXITCODE -ne 0) { throw 'Visual C++ environment initialization failed.' }
    foreach ($line in $compilerEnvironment) {
        if ($line -match '^([^=]+)=(.*)$') {
            [Environment]::SetEnvironmentVariable($matches[1], $matches[2], 'Process')
        }
    }
    if ($WindowsSdkRoot) {
        $sdkPath = (Resolve-Path -LiteralPath $WindowsSdkRoot).Path
        $umLibraries = Join-Path $sdkPath 'um/x64'
        $ucrtLibraries = Join-Path $sdkPath 'ucrt/x64'
        if (!(Test-Path -LiteralPath (Join-Path $umLibraries 'kernel32.lib')) -or
            !(Test-Path -LiteralPath (Join-Path $ucrtLibraries 'ucrt.lib'))) {
            throw 'WindowsSdkRoot must contain um/x64 and ucrt/x64 SDK libraries.'
        }
        $env:LIB = $umLibraries + ';' + $ucrtLibraries + ';' + $env:LIB
    }
    if ($AuxiliaryToolsRoot) {
        $auxiliaryPath = (Resolve-Path -LiteralPath $AuxiliaryToolsRoot).Path
        $env:PATH = $auxiliaryPath + ';' + $env:PATH
    }
    if ($CargoArguments.Count -lt 1) { throw 'Supply a Cargo command.' }
    $cargoCommandName = $CargoArguments[0]
    $cargoRemainingArguments = @($CargoArguments | Select-Object -Skip 1)
    & $cargoPath $cargoCommandName --manifest-path (Join-Path $repositoryRoot 'Cargo.toml') @cargoRemainingArguments
    if ($LASTEXITCODE -ne 0) { throw 'Rust build/check failed.' }

} finally {
    # Each invocation must leave its caller environment intact. Repeated vcvars
    # initialization otherwise duplicates PATH/LIB until cmd.exe exceeds its limit.
    foreach ($name in [Environment]::GetEnvironmentVariables('Process').Keys) {
        if (!$savedEnvironment.Contains($name)) { [Environment]::SetEnvironmentVariable($name, $null, 'Process') }
    }
    foreach ($name in $savedEnvironment.Keys) {
        [Environment]::SetEnvironmentVariable($name, $savedEnvironment[$name], 'Process')
    }
}
