[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'BuildPaths.ps1')
Initialize-AchimeldBuildPaths
& (Join-Path $PSScriptRoot 'Update-Version.ps1') -Kind Check
$fixtureRoot = Join-Path $env:ACHIMELD_ARTIFACT_ROOT ('versioning-tests-' + [Guid]::NewGuid().ToString('N'))
$fixtureBuild = Join-Path $fixtureRoot 'build'
New-Item -ItemType Directory -Path $fixtureBuild -Force | Out-Null
$script = Join-Path $fixtureBuild 'Update-Version.ps1'
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'Update-Version.ps1') -Destination $script
$historyPath = Join-Path $fixtureRoot 'CHANGELOG.md'
$seed = "# Change log`n`n## 0.1.0.9 (2026-10-18)`n`n- Original change.`n"
[IO.File]::WriteAllText($historyPath, $seed)
& $script -Kind Change -Summary 'Completed a feature.'
$history = [IO.File]::ReadAllText($historyPath)
if (!$history.Contains('## 0.1.0.10 (') -or !$history.EndsWith($seed.Substring($seed.IndexOf('## ')))) { throw 'Change increment or history preservation failed.' }
& $script -Kind Commit -Summary 'Commit completed changes.'
if (![IO.File]::ReadAllText($historyPath).Contains('## 0.1.1.0 (')) { throw 'Commit increment/reset failed.' }
& $script -Kind Change -Summary 'Next feature.'
$history = [IO.File]::ReadAllText($historyPath)
if (!$history.Contains('## 0.1.1.1 (')) { throw 'Post-commit change increment failed.' }
& $script -Kind Check
if ([IO.File]::ReadAllText($historyPath) -cne $history) { throw 'Read-only check changed history.' }
$held = [IO.File]::Open($historyPath, [IO.FileMode]::Open, [IO.FileAccess]::ReadWrite, [IO.FileShare]::Read)
try {
    $rejected = $false
    try { & $script -Kind Change -Summary 'Concurrent update.' } catch { $rejected = $true }
    if (!$rejected) { throw 'Concurrent writer was accepted.' }
} finally { $held.Dispose() }
if ([IO.File]::ReadAllText($historyPath) -cne $history) { throw 'Concurrent update changed history.' }
foreach ($summary in @('', "Two`nlines")) {
    $rejected = $false
    try { & $script -Kind Change -Summary $summary } catch { $rejected = $true }
    if (!$rejected -or [IO.File]::ReadAllText($historyPath) -cne $history) { throw 'Invalid summary changed history.' }
}
foreach ($invalid in @(
    $seed.Replace('0.1.0.9', '0.1.0'),
    $seed.Replace('0.1.0.9', '0.1.0.09'),
    $seed.Replace('2026-10-18', '2026-02-30'),
    $seed.Replace('- Original change.', ''),
    ($seed + $seed.Substring($seed.IndexOf('## '))),
    ($seed + "`n## 0.1.1.0 (2026-10-18)`n`n- Out of order.`n")
)) {
    [IO.File]::WriteAllText($historyPath, $invalid)
    $rejected = $false
    try { & $script -Kind Change -Summary 'Must be rejected.' } catch { $rejected = $true }
    if (!$rejected -or [IO.File]::ReadAllText($historyPath) -cne $invalid) { throw 'Invalid history was accepted or changed.' }
}
Write-Output "Version transitions, history preservation and invalid-input checks passed. Fixtures: $fixtureRoot"
