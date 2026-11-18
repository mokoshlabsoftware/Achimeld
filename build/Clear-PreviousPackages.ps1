[CmdletBinding()]
param([string]$BuildRoot = $env:ACHIMELD_BUILD_ROOT, [switch]$Preview, [string]$TransientPath)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'BuildPaths.ps1')
$paths = Get-AchimeldBuildPaths -BuildRoot $BuildRoot
$root = [IO.Path]::GetFullPath($paths.Root).TrimEnd('\', '/')
$current = Join-Path $root 'current'

function Assert-OrdinaryPath([string]$Path) {
    $item = Get-Item -LiteralPath $Path -Force
    if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -or
        (Resolve-Path -LiteralPath $Path).ProviderPath -ine $Path) {
        throw "Package cleanup boundary rejected: $Path"
    }
}

if (!(Test-Path -LiteralPath $root)) { return }
for ($ancestor = $root; $ancestor; $ancestor = Split-Path -Parent $ancestor) {
    Assert-OrdinaryPath $ancestor
}
$targets = @(
    if ($TransientPath) {
        $path = [IO.Path]::GetFullPath($TransientPath)
        $parent = Split-Path -Parent $path
        $leaf = Split-Path -Leaf $path
        $directScratch = $parent -ieq $current -and $leaf -cmatch '^(previous|bridge-signer|release-target)-[a-f0-9]{32}$'
        $installerScratch = (Split-Path -Parent $parent) -ieq $current -and
            (Split-Path -Leaf $parent) -cmatch '^(github-release|quick-install)-[a-f0-9]{32}$' -and
            $leaf -cin @('payload', 'verified-package')
        if (!$directScratch -and !$installerScratch) { throw 'Transient package cleanup path is outside build staging.' }
        if (Test-Path -LiteralPath $path) {
            Assert-OrdinaryPath $current
            Assert-OrdinaryPath $parent
            Get-Item -LiteralPath $path -Force
        }
    } else {
        if (Test-Path -LiteralPath $current) {
            Assert-OrdinaryPath $current
            Get-ChildItem -LiteralPath $current -Force | Where-Object {
                !$_.PSIsContainer -and $_.Name -ceq 'last-archive.txt'
            }
        }
        foreach ($day in Get-ChildItem -LiteralPath $root -Directory -Force | Where-Object Name -CMatch '^\d{2}_\d{2}_\d{4}$') {
            Assert-OrdinaryPath $day.FullName
            Get-ChildItem -LiteralPath $day.FullName -Directory -Force | Where-Object Name -CMatch '^(build-\d{2}_\d{2}_\d{2}-[a-f0-9]{8}|previous-[a-f0-9]{32})$'
        }
    }
)
foreach ($target in $targets) {
    if (!$target.FullName.StartsWith($root + '\', [StringComparison]::OrdinalIgnoreCase)) {
        throw "Package cleanup escaped build storage: $($target.FullName)"
    }
    Assert-OrdinaryPath $target.FullName
    if ($target.PSIsContainer) {
        foreach ($entry in Get-ChildItem -LiteralPath $target.FullName -Recurse -Force) {
            if ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) {
                throw "Package cleanup rejects a reparse point: $($entry.FullName)"
            }
        }
    }
}
foreach ($target in $targets) {
    if ($Preview) { Write-Output $target.FullName; continue }
    Remove-Item -LiteralPath $target.FullName -Recurse:$target.PSIsContainer -Force
    Write-Output "Removed obsolete package artifact: $($target.FullName)"
}
