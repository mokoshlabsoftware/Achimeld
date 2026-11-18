[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$cleanup = Join-Path $PSScriptRoot 'Clear-ObsoleteBuildArtifacts.ps1'
$repository = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$fixture = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) ('achimeld-obsolete-' + [guid]::NewGuid().ToString('N'))))
if ($fixture.StartsWith($repository.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Cleanup fixture must stay outside source.'
}
$current = Join-Path $fixture 'current'
$install = Join-Path $fixture 'install'
$day = Join-Path $fixture '01_01_2026'
$oldAudit = Join-Path $current ('audit-' + ('a' * 32))
$newAudit = Join-Path $current ('audit-' + ('b' * 32))
$oldRelease = Join-Path $current ('github-release-' + ('c' * 32))
$oldArchive = Join-Path $day 'build-01_02_03-aaaaaaaa'
$keepArchive = Join-Path $day 'build-04_05_06-bbbbbbbb'
$cache = Join-Path $current 'target/debug'
$junction = Join-Path $oldRelease 'linked'
try {
    foreach ($directory in @($current, $install, (Join-Path $install 'audit'), $oldAudit, $newAudit, $oldRelease, $oldArchive, $keepArchive, $cache)) {
        New-Item -ItemType Directory -Path $directory -Force | Out-Null
    }
    foreach ($directory in @($oldAudit, $newAudit, $oldRelease, $oldArchive, $keepArchive, $cache)) {
        [IO.File]::WriteAllText((Join-Path $directory 'keep.txt'), $directory)
    }
    $setupName = 'Achimeld-0_3_9_10-Setup.exe'
    $setup = Join-Path $install $setupName
    [IO.File]::WriteAllText($setup, 'verified installer fixture')
    $hash = (Get-FileHash -LiteralPath $setup -Algorithm SHA256).Hash.ToLowerInvariant()
    $commit = 'd' * 40
    @{ schemaVersion=1; workingVersion='0.3.9.10'; gitlabSourceCommit=$commit; installerSha256=$hash } |
        ConvertTo-Json | Set-Content -LiteralPath (Join-Path $install 'RELEASE-SOURCE.json')
    @{ selectedChecksPassed=$true } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $install 'audit/evidence.json') -Force
    [IO.File]::WriteAllText((Join-Path $install 'SHA256SUMS.txt'), "$hash  $setupName`n")
    [IO.File]::WriteAllText((Join-Path $install 'INSTALL-NOTES.txt'), 'fixture notes')
    $receiptFiles = @($setupName, 'RELEASE-SOURCE.json', 'SHA256SUMS.txt', 'INSTALL-NOTES.txt', 'audit/evidence.json') |
        ForEach-Object { @{ path=$_; sha256=(Get-FileHash -LiteralPath (Join-Path $install $_) -Algorithm SHA256).Hash.ToLowerInvariant() } }
    @{ schemaVersion=1; version='0.3.9.10'; commit=$commit; files=$receiptFiles } |
        ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $install 'INSTALLER-COMPLETE.json')
    [IO.File]::WriteAllText((Join-Path $current 'last-archive.txt'), $keepArchive)
    $old = [datetime]'2026-01-01T00:00:00Z'
    foreach ($directory in @($oldAudit, $oldRelease, $oldArchive)) { [IO.Directory]::SetLastWriteTimeUtc($directory, $old) }
    [IO.File]::SetLastWriteTimeUtc((Join-Path $install 'INSTALLER-COMPLETE.json'), [datetime]'2026-02-01T00:00:00Z')
    [IO.Directory]::SetLastWriteTimeUtc($newAudit, [datetime]'2026-03-01T00:00:00Z')
    $preview = @(& $cleanup -BuildRoot $fixture -Preview)
    if ($preview.Count -ne 3 -or !(Test-Path -LiteralPath $oldAudit) -or !(Test-Path -LiteralPath $oldArchive)) {
        throw 'Preview changed files or missed obsolete evidence.'
    }
    & $cleanup -BuildRoot $fixture | Out-Null
    if ((Test-Path -LiteralPath $oldAudit) -or (Test-Path -LiteralPath $oldRelease) -or
        (Test-Path -LiteralPath $oldArchive) -or !(Test-Path -LiteralPath $newAudit) -or
        !(Test-Path -LiteralPath $keepArchive) -or !(Test-Path -LiteralPath $cache) -or
        !(Test-Path -LiteralPath $setup)) {
        throw 'Obsolete cleanup removed current or unrelated artifacts.'
    }
    New-Item -ItemType Directory -Path $oldRelease | Out-Null
    [IO.File]::WriteAllText((Join-Path $oldRelease 'keep.txt'), 'keep')
    [IO.Directory]::SetLastWriteTimeUtc($oldRelease, $old)
    [IO.File]::WriteAllText((Join-Path $install 'SHA256SUMS.txt'), 'invalid')
    $rejected = $false
    try { & $cleanup -BuildRoot $fixture | Out-Null } catch { $rejected = $true }
    if (!$rejected -or !(Test-Path -LiteralPath $oldRelease)) { throw 'Invalid installer authorized cleanup.' }
    [IO.File]::WriteAllText((Join-Path $install 'SHA256SUMS.txt'), "$hash  $setupName`n")
    New-Item -ItemType Junction -Path $junction -Target $cache | Out-Null
    [IO.Directory]::SetLastWriteTimeUtc($oldRelease, $old)
    $rejected = $false
    try { & $cleanup -BuildRoot $fixture | Out-Null } catch { $rejected = $true }
    if (!$rejected -or !(Test-Path -LiteralPath $oldRelease) -or !(Test-Path -LiteralPath (Join-Path $cache 'keep.txt'))) {
        throw 'Reparse preflight did not preserve the deletion set.'
    }
    $packages = Join-Path $PSScriptRoot 'Clear-PreviousPackages.ps1'
    $currentPackage = Join-Path $current 'package'
    $previous = Join-Path $current ('previous-' + ('e' * 32))
    New-Item -ItemType Directory -Path $currentPackage, $previous | Out-Null
    $legacyPlan = @(& $packages -BuildRoot $fixture -Preview)
    if ($keepArchive -cnotin $legacyPlan -or $previous -cin $legacyPlan -or $currentPackage -cin $legacyPlan) {
        throw 'Pre-build package cleanup selected the wrong generation.'
    }
    $archiveLink = Join-Path $keepArchive 'linked'
    New-Item -ItemType Junction -Path $archiveLink -Target $cache | Out-Null
    try {
        $rejected = $false
        try { & $packages -BuildRoot $fixture | Out-Null } catch { $rejected = $true }
        if (!$rejected -or !(Test-Path -LiteralPath (Join-Path $current 'last-archive.txt'))) {
            throw 'Pre-build package cleanup did not preflight the full set.'
        }
    } finally { Remove-Item -LiteralPath $archiveLink -Force }
    & $packages -BuildRoot $fixture | Out-Null
    & $packages -BuildRoot $fixture -TransientPath $previous | Out-Null
    if ((Test-Path -LiteralPath $keepArchive) -or (Test-Path -LiteralPath $previous) -or
        !(Test-Path -LiteralPath $currentPackage) -or !(Test-Path -LiteralPath $setup)) {
        throw 'Package cleanup did not preserve the current build and installer.'
    }
    Write-Output 'Obsolete staging, archive and evidence cleanup; current retention; checksum and reparse rejection passed.'
} finally {
    if (Test-Path -LiteralPath $junction) { Remove-Item -LiteralPath $junction -Force }
    if (Test-Path -LiteralPath $fixture) { Remove-Item -LiteralPath $fixture -Recurse -Force }
}
