[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$fixture = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) ('achimeld-development-source-' + [guid]::NewGuid().ToString('N'))))
$source = Join-Path $fixture 'source'
$output = Join-Path $fixture 'output'
$snapshot = $null
try {
    New-Item -ItemType Directory -Path $source,$output -Force | Out-Null
    & git init -b main $source | Out-Null
    & git -C $source config user.name Fixture
    & git -C $source config user.email fixture@example.invalid
    Set-Content -LiteralPath (Join-Path $source 'CHANGELOG.md') -Value 'original'
    Set-Content -LiteralPath (Join-Path $source 'deleted.txt') -Value 'remove me'
    Set-Content -LiteralPath (Join-Path $source '.gitignore') -Value 'ignored-output/'
    & git -C $source add --all
    & git -C $source commit -m 'Fixture source' | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Fixture commit failed.' }
    Set-Content -LiteralPath (Join-Path $source 'CHANGELOG.md') -Value 'captured edit'
    Remove-Item -LiteralPath (Join-Path $source 'deleted.txt')
    New-Item -ItemType Directory -Path (Join-Path $source 'assets'),(Join-Path $source 'ignored-output') | Out-Null
    Set-Content -LiteralPath (Join-Path $source 'assets/loading.png') -Value 'new source art'
    Set-Content -LiteralPath (Join-Path $source 'ignored-output/generated.bin') -Value 'not source'
    $snapshot = & (Join-Path $PSScriptRoot 'New-DevelopmentSourceSnapshot.ps1') -SourceRoot $source -OutputRoot $output
    if ((Get-Content -LiteralPath (Join-Path $snapshot 'CHANGELOG.md') -Raw).Trim() -cne 'captured edit' -or
        !(Test-Path -LiteralPath (Join-Path $snapshot 'assets/loading.png')) -or
        (Test-Path -LiteralPath (Join-Path $snapshot 'deleted.txt')) -or
        (Test-Path -LiteralPath (Join-Path $snapshot 'ignored-output'))) {
        throw 'Captured source did not preserve edits, additions, deletions and ignored-output exclusion.'
    }
    Set-Content -LiteralPath (Join-Path $source 'CHANGELOG.md') -Value 'edit during compile'
    if ((Get-Content -LiteralPath (Join-Path $snapshot 'CHANGELOG.md') -Raw).Trim() -cne 'captured edit') {
        throw 'Later source edits changed the build snapshot.'
    }
    Write-Output 'Development source snapshot retains complete working edits and stays stable during later changes.'
} finally {
    $temporary = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
    if (!$fixture.StartsWith($temporary, [StringComparison]::OrdinalIgnoreCase) -or
        (Split-Path -Leaf $fixture) -notmatch '^achimeld-development-source-[a-f0-9]{32}$') { throw 'Snapshot fixture cleanup boundary rejected.' }
    if ($snapshot -and (Test-Path -LiteralPath $snapshot)) {
        $resolved = [IO.Path]::GetFullPath($snapshot)
        if (!$resolved.StartsWith([IO.Path]::GetFullPath($output).TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase) -or
            (Split-Path -Leaf $resolved) -notmatch '^development-source-[a-f0-9]{32}$') { throw 'Snapshot worktree cleanup boundary rejected.' }
        & git -C $source worktree remove --force $resolved
        if ($LASTEXITCODE -ne 0) { throw 'Snapshot fixture worktree cleanup failed.' }
    }
    if (Test-Path -LiteralPath $fixture) { Remove-Item -LiteralPath $fixture -Recurse -Force }
}
