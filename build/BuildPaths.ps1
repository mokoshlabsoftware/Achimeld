# Shared by build, packaging and audit entry points. Nothing generated belongs in source.
function Get-AchimeldBuildPaths {
    param([datetime]$Date = (Get-Date), [string]$BuildRoot = $env:ACHIMELD_BUILD_ROOT)
    if (!$BuildRoot) { $BuildRoot = 'C:\Projects\AchimeldBuild' }
    $root = [IO.Path]::GetFullPath($BuildRoot)
    $sourceRoot = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot)).TrimEnd('\', '/')
    if ($root.Equals($sourceRoot, [StringComparison]::OrdinalIgnoreCase) -or
        $root.StartsWith($sourceRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Build storage must be outside the source repository.'
    }
    $day = Join-Path $root $Date.ToString('dd_MM_yyyy', [Globalization.CultureInfo]::InvariantCulture)
    $current = Join-Path $root 'current'
    [pscustomobject]@{ Root = $root; Archive = $day; Artifacts = $current; Cache = (Join-Path $root 'cache'); Target = (Join-Path $current 'target') }
}

function Initialize-AchimeldBuildPaths {
    $paths = Get-AchimeldBuildPaths
    $env:ACHIMELD_SOURCE_ROOT = Split-Path -Parent $PSScriptRoot
    $env:ACHIMELD_BUILD_ROOT = $paths.Root
    $env:ACHIMELD_ARTIFACT_ROOT = $paths.Artifacts
    $env:ACHIMELD_ARCHIVE_ROOT = $paths.Archive
    $env:ACHIMELD_CACHE_ROOT = $paths.Cache
    $env:CARGO_TARGET_DIR = $paths.Target
    $env:CARGO_HOME = Join-Path $paths.Cache 'cargo-home'
    $env:DOTNET_CLI_HOME = Join-Path $paths.Cache 'dotnet-home'
    $env:NUGET_PACKAGES = Join-Path $paths.Cache 'nuget'
    $env:DOTNET_CLI_TELEMETRY_OPTOUT = '1'
    $env:DOTNET_GENERATE_ASPNET_CERTIFICATE = 'false'
    New-Item -ItemType Directory -Path $paths.Artifacts, $paths.Cache -Force | Out-Null
}
