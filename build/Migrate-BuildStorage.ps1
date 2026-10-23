[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'BuildPaths.ps1')
Initialize-AchimeldBuildPaths
$repository = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$archive = Join-Path $env:ACHIMELD_ARCHIVE_ROOT ('migrated-' + [Guid]::NewGuid().ToString('N'))
function Move-Generated([string]$Source, [string]$Destination) {
    if (!(Test-Path -LiteralPath $Source)) { return }
    $resolved = (Resolve-Path -LiteralPath $Source).Path
    $destinationFull = [IO.Path]::GetFullPath($Destination)
    if (!$resolved.StartsWith($repository + '\', [StringComparison]::OrdinalIgnoreCase) -or
        !$destinationFull.StartsWith($env:ACHIMELD_BUILD_ROOT + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Migration boundary rejected.' }
    if ((Get-Item -LiteralPath $resolved -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Reparse migration source rejected.' }
    if (Test-Path -LiteralPath $destinationFull) { throw "Migration destination already exists: $destinationFull" }
    New-Item -ItemType Directory -Path (Split-Path -Parent $destinationFull) -Force | Out-Null
    Move-Item -LiteralPath $resolved -Destination $destinationFull
    Write-Host "Moved $resolved to $destinationFull"
}
$legacy = Join-Path $repository '.artifacts'
$cacheNames = @('cargo-home','dotnet-home','nuget','downloads','rustup','windows-sdk','audit-tools','advisory-db','rust-tools','tool-setup')
if (Test-Path -LiteralPath $legacy) {
    foreach ($entry in Get-ChildItem -LiteralPath $legacy -Force) {
        $destination = if ($entry.Name -in $cacheNames -and !(Test-Path -LiteralPath (Join-Path $env:ACHIMELD_CACHE_ROOT $entry.Name))) {
            Join-Path $env:ACHIMELD_CACHE_ROOT $entry.Name
        } else { Join-Path (Join-Path $archive 'artifacts') $entry.Name }
        Move-Generated $entry.FullName $destination
    }
    # Only remove the now-empty generated directory.
    Remove-Item -LiteralPath $legacy
}
$target = Join-Path $env:ACHIMELD_ARTIFACT_ROOT 'target'
if (Test-Path -LiteralPath $target) { $target = Join-Path $archive 'target' }
Move-Generated (Join-Path $repository 'target') $target
foreach ($area in @('bridge','tools')) {
    foreach ($project in Get-ChildItem -LiteralPath (Join-Path $repository $area) -Directory) {
        foreach ($name in @('bin','obj')) {
            Move-Generated (Join-Path $project.FullName $name) (Join-Path $archive "$area/$($project.Name)/$name")
        }
    }
}
Write-Host "Previous outputs preserved under $archive"
