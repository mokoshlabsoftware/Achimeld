[CmdletBinding()]
param([string]$BuildRoot = $env:ACHIMELD_BUILD_ROOT)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'BuildPaths.ps1')
$paths = Get-AchimeldBuildPaths -BuildRoot $BuildRoot
$current = [IO.Path]::GetFullPath($paths.Artifacts)
$repository = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot)).TrimEnd('\', '/')
if ($current.Equals($repository, [StringComparison]::OrdinalIgnoreCase) -or
    $current.StartsWith($repository + '\', [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Code artifact cleanup must stay outside the source repository.'
}
# Validate every existing ancestor before enumerating or deleting build storage.
for ($ancestor = $current; $ancestor; $ancestor = Split-Path -Parent $ancestor) {
    if (Test-Path -LiteralPath $ancestor) {
        $item = Get-Item -LiteralPath $ancestor -Force
        if (!$item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            throw "Code artifact cleanup requires ordinary directories: $ancestor"
        }
    }
}
if (!(Test-Path -LiteralPath $current)) { return }
$targets = @(Get-ChildItem -LiteralPath $current -Force | Where-Object {
    $_.Name -in @('worker-fixture', 'offline-nuget.config') -or
    $_.Name -cmatch '^bridge-package-[a-f0-9]{32}$' -or
    $_.Name -cmatch '^bridge-trust-[a-f0-9]{16}\.json$'
})
# Preflight the entire deletion set. Never traverse a junction or symbolic link.
foreach ($target in $targets) {
    $resolved = (Resolve-Path -LiteralPath $target.FullName).ProviderPath
    if (![IO.Path]::GetFullPath($resolved).Equals($target.FullName, [StringComparison]::OrdinalIgnoreCase) -or
        !(Split-Path -Parent $resolved).Equals($current, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Code artifact cleanup boundary rejected: $resolved"
    }
    if ($target.Attributes -band [IO.FileAttributes]::ReparsePoint) {
        throw "Reparse code artifact rejected: $resolved"
    }
    if ($target.PSIsContainer) {
        foreach ($entry in Get-ChildItem -LiteralPath $resolved -Recurse -Force) {
            if ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) {
                throw "Reparse code artifact rejected: $($entry.FullName)"
            }
        }
    }
}
foreach ($target in $targets) {
    Write-Host ('Deleting generated code artifacts: ' + $target.FullName)
    Remove-Item -LiteralPath $target.FullName -Recurse -Force
}
