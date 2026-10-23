[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$repositoryRoot = Split-Path -Parent $PSScriptRoot
$registry = Get-Content -LiteralPath (Join-Path $repositoryRoot 'docs/security/invariants.json') -Raw | ConvertFrom-Json
if ($registry.schemaVersion -ne 1 -or !$registry.invariants.Count) { throw 'Invariant registry schema rejected.' }
$ids = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
foreach ($invariant in $registry.invariants) {
    if ($invariant.id -notmatch '^SEC-[A-Z]+-\d{3}$' -or !$ids.Add($invariant.id) -or !$invariant.claim -or !$invariant.qualification) { throw 'Invalid or duplicate invariant identity.' }
    if (!$invariant.enforcement.Count -or !$invariant.evidence.Count) { throw "Missing invariant links: $($invariant.id)" }
    foreach ($reference in @($invariant.enforcement) + @($invariant.evidence)) {
        $path = [IO.Path]::GetFullPath((Join-Path $repositoryRoot $reference.file))
        if (!$path.StartsWith($repositoryRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase) -or !(Test-Path -LiteralPath $path -PathType Leaf)) { throw "Missing invariant source: $($reference.file)" }
        if (!$reference.symbol -or !(Get-Content -LiteralPath $path -Raw).Contains($reference.symbol, [StringComparison]::Ordinal)) { throw "Missing invariant symbol: $($reference.file) $($reference.symbol)" }
    }
}
foreach ($file in Get-ChildItem -LiteralPath (Join-Path $repositoryRoot 'docs/security') -Filter '*.md') {
    foreach ($match in [regex]::Matches((Get-Content -LiteralPath $file.FullName -Raw), '\[[^\]]+\]\(([^)]+)\)')) {
        $target = $match.Groups[1].Value.Split('#')[0]
        if (!$target -or $target -match '^[a-z]+:' ) { continue }
        if (!(Test-Path -LiteralPath (Join-Path $file.DirectoryName $target))) { throw "Broken audit navigation: $($file.Name) -> $target" }
    }
}
foreach ($file in Get-ChildItem -LiteralPath @((Join-Path $repositoryRoot 'build'),(Join-Path $repositoryRoot 'installer'),(Join-Path $repositoryRoot 'tests')) -Filter '*.ps1' -Recurse) {
    $tokens = $null; $parseErrors = $null
    [void][Management.Automation.Language.Parser]::ParseFile($file.FullName, [ref]$tokens, [ref]$parseErrors)
    if ($parseErrors.Count) { throw "PowerShell parse failed: $($file.FullName)" }
}
$ci = Get-Content -LiteralPath (Join-Path $repositoryRoot '.gitlab-ci.yml') -Raw
if (!$ci.Contains('Audit.ps1 -Fixtures') -or $ci.Contains('--exclude')) { throw 'CI audit gate or full-workspace lint coverage changed.' }
foreach ($crate in Get-ChildItem -LiteralPath (Join-Path $repositoryRoot 'crates') -Directory) {
    if (!(Get-Content -LiteralPath (Join-Path $crate.FullName 'Cargo.toml') -Raw).Contains('workspace = true')) { throw "Workspace lint inheritance missing: $($crate.Name)" }
}
Write-Output "Audit navigation, $($ids.Count) invariant references, script syntax and CI scope checks passed. These checks detect drift; they do not prove the claims."
