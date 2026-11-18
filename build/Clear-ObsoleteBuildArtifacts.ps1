[CmdletBinding()]
param([string]$BuildRoot = $env:ACHIMELD_BUILD_ROOT, [switch]$Preview)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'BuildPaths.ps1')
$paths = Get-AchimeldBuildPaths -BuildRoot $BuildRoot
$repository = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot)).TrimEnd('\', '/')
$root = [IO.Path]::GetFullPath($paths.Root).TrimEnd('\', '/')
$current = [IO.Path]::GetFullPath($paths.Artifacts).TrimEnd('\', '/')
$install = [IO.Path]::GetFullPath($paths.Installer).TrimEnd('\', '/')
if ($root.Equals($repository, [StringComparison]::OrdinalIgnoreCase) -or
    $root.StartsWith($repository + '\', [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Build artifact cleanup must stay outside the source repository.'
}
function Assert-OrdinaryDirectory([string]$Path) {
    $item = Get-Item -LiteralPath $Path -Force
    if (!$item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -or
        !((Resolve-Path -LiteralPath $Path).Path).Equals($Path, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Build artifact cleanup requires an ordinary directory: $Path"
    }
}
function Assert-OrdinaryTree([string]$Path) {
    Assert-OrdinaryDirectory $Path
    foreach ($entry in Get-ChildItem -LiteralPath $Path -Recurse -Force) {
        if ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) {
            throw "Build artifact cleanup rejects a reparse point: $($entry.FullName)"
        }
    }
}
function Assert-PlainFile([string]$Path) {
    $item = Get-Item -LiteralPath $Path -Force
    if ($item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw "Build artifact cleanup requires a plain file: $Path"
    }
}
for ($ancestor = $root; $ancestor; $ancestor = Split-Path -Parent $ancestor) {
    if (Test-Path -LiteralPath $ancestor) { Assert-OrdinaryDirectory $ancestor }
}
foreach ($directory in @($current, $install)) { Assert-OrdinaryDirectory $directory }
$quick = Test-Path -LiteralPath (Join-Path $install 'QUICK-BUILD.json') -PathType Leaf
$audited = Test-Path -LiteralPath (Join-Path $install 'RELEASE-SOURCE.json') -PathType Leaf
if ($quick -eq $audited) { throw 'Exactly one completed installer source record is required for cleanup.' }
$sourceName = if ($quick) { 'QUICK-BUILD.json' } else { 'RELEASE-SOURCE.json' }
Assert-PlainFile (Join-Path $install $sourceName)
$source = Get-Content -LiteralPath (Join-Path $install $sourceName) -Raw | ConvertFrom-Json
$receiptPath = Join-Path $install 'INSTALLER-COMPLETE.json'
Assert-PlainFile $receiptPath
$receipt = Get-Content -LiteralPath $receiptPath -Raw | ConvertFrom-Json
if ($source.workingVersion -cnotmatch '^\d+\.\d+\.\d+\.\d+$' -or
    $source.installerSha256 -cnotmatch '^[a-f0-9]{64}$' -or
    $receipt.schemaVersion -ne 1 -or $receipt.version -cne $source.workingVersion) {
    throw 'Completed installer identity is invalid; nothing was removed.'
}
$setupName = 'Achimeld-' + $source.workingVersion.Replace('.', '_') + $(if ($quick) { '-Quick-Test' } else { '' }) + '-Setup.exe'
$setup = Join-Path $install $setupName
$expectedChecksum = "$($source.installerSha256)  $setupName"
$expectedFiles = if ($quick) { @($setupName, $sourceName, 'SHA256SUMS.txt', 'INSTALL-NOTES.txt') }
                 else { @($setupName, $sourceName, 'SHA256SUMS.txt', 'INSTALL-NOTES.txt', 'audit/evidence.json') }
if (@($receipt.files).Count -ne $expectedFiles.Count) {
    throw 'Completed installer checksum is invalid; nothing was removed.'
}
foreach ($name in $expectedFiles) {
    $entry = @($receipt.files | Where-Object { $_.path -ceq $name })
    $file = Join-Path $install $name
    if ($entry.Count -ne 1 -or $entry[0].sha256 -cnotmatch '^[a-f0-9]{64}$') {
        throw 'Completed installer receipt is invalid; nothing was removed.'
    }
    if ($name -eq 'audit/evidence.json') { Assert-OrdinaryDirectory (Join-Path $install 'audit') }
    Assert-PlainFile $file
    if ((Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant() -cne $entry[0].sha256) {
        throw 'Completed installer file changed; nothing was removed.'
    }
}
$receiptSetup = @($receipt.files | Where-Object { $_.path -ceq $setupName })[0]
if ($receiptSetup.sha256 -cne $source.installerSha256 -or
    (Get-Content -LiteralPath (Join-Path $install 'SHA256SUMS.txt') -Raw).Trim() -cne $expectedChecksum) {
    throw 'Completed installer checksum is invalid; nothing was removed.'
}
if ($audited) {
    $evidence = Get-Content -LiteralPath (Join-Path $install 'audit/evidence.json') -Raw | ConvertFrom-Json
    if (!$evidence.selectedChecksPassed -or $receipt.commit -cne $source.gitlabSourceCommit) {
        throw 'Audited installer evidence is invalid; nothing was removed.'
    }
}
$cutoff = (Get-Item -LiteralPath $receiptPath).LastWriteTimeUtc
$targets = [Collections.Generic.List[string]]::new()
$newestAudit = if ($audited) { Get-ChildItem -LiteralPath $current -Directory -Filter 'audit-*' |
    Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 1 } else { $null }
foreach ($directory in Get-ChildItem -LiteralPath $current -Directory -Force) {
    if ($directory.LastWriteTimeUtc -ge $cutoff -or
        ($newestAudit -and $directory.FullName -ieq $newestAudit.FullName)) { continue }
    $eligible = if ($quick) {
        $directory.Name -cmatch '^(quick-install|quick-installer-source)-[a-f0-9]{32}$'
    } else {
        $directory.Name -cmatch '^(audit|github-release|quick-install|quick-installer-source|bridge-signer|bridge-package|release-target)-[a-f0-9]{32}$' -or
        $directory.Name -cmatch '^publication-source-[a-f0-9]{40}-[a-f0-9]{32}$'
    }
    if ($eligible) { $targets.Add($directory.FullName) }
}
if ($audited) {
    $pointer = Join-Path $current 'last-archive.txt'
    if (Test-Path -LiteralPath $pointer -PathType Leaf) {
        Assert-PlainFile $pointer
        $keepArchive = [IO.Path]::GetFullPath((Get-Content -LiteralPath $pointer -Raw).Trim())
        if (!(Split-Path -Parent (Split-Path -Parent $keepArchive)).Equals($root, [StringComparison]::OrdinalIgnoreCase) -or
            (Split-Path -Leaf (Split-Path -Parent $keepArchive)) -cnotmatch '^\d{2}_\d{2}_\d{4}$' -or
            (Split-Path -Leaf $keepArchive) -cnotmatch '^build-\d{2}_\d{2}_\d{2}-[a-f0-9]{8}$') {
            throw 'Current archive pointer is invalid; nothing was removed.'
        }
        foreach ($day in Get-ChildItem -LiteralPath $root -Directory -Force | Where-Object { $_.Name -cmatch '^\d{2}_\d{2}_\d{4}$' }) {
            Assert-OrdinaryDirectory $day.FullName
            foreach ($archive in Get-ChildItem -LiteralPath $day.FullName -Directory -Force) {
                if ($archive.FullName -ieq $keepArchive -or $archive.LastWriteTimeUtc -ge $cutoff) { continue }
                if ($archive.Name -cmatch '^(build-\d{2}_\d{2}_\d{2}-[a-f0-9]{8}|previous-[a-f0-9]{32})$') {
                    $targets.Add($archive.FullName)
                }
            }
        }
    }
}
# Preflight the complete set before deleting any part of it.
foreach ($target in $targets) {
    if (!($target.StartsWith($current + '\', [StringComparison]::OrdinalIgnoreCase) -or
        $target.StartsWith($root + '\', [StringComparison]::OrdinalIgnoreCase))) {
        throw "Build artifact cleanup boundary rejected: $target"
    }
    Assert-OrdinaryTree $target
}
foreach ($target in $targets) {
    if ($Preview) { Write-Output "Would delete obsolete generated artifact: $target"; continue }
    try {
        Remove-Item -LiteralPath $target -Recurse -Force -ErrorAction Stop
        Write-Output "Deleted obsolete generated artifact: $target"
    } catch { Write-Warning "Obsolete artifact remains in use: $target - $($_.Exception.Message)" }
}
