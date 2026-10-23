[CmdletBinding()]
param([switch]$InputsOnly)
$ErrorActionPreference = 'Stop'
$repositoryRoot = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'BuildPaths.ps1')
Initialize-AchimeldBuildPaths
$boundary = $repositoryRoot + [IO.Path]::DirectorySeparatorChar
$projects = @(Get-ChildItem -LiteralPath @((Join-Path $repositoryRoot 'bridge'),(Join-Path $repositoryRoot 'tools')) -Filter '*.csproj' -File -Recurse | Where-Object FullName -NotMatch '[\\/](bin|obj)[\\/]')
foreach ($project in $projects) {
    [xml]$xml = Get-Content -LiteralPath $project.FullName -Raw
    if ($xml.SelectNodes('//Reference').Count) { throw 'Prebuilt application assembly references are forbidden.' }
    $result = & dotnet msbuild $project.FullName '-p:Configuration=Release' '-getItem:Compile,ProjectReference' -nologo
    if ($LASTEXITCODE -ne 0) { throw 'Cannot inspect evaluated managed source inputs.' }
    $items = ($result -join "`n" | ConvertFrom-Json).Items
    foreach ($item in @($items.Compile) + @($items.ProjectReference)) {
        if (!$item) { continue }
        $path = [IO.Path]::GetFullPath($item.FullPath)
        if (!$path.StartsWith($boundary,[StringComparison]::OrdinalIgnoreCase) -or !(Test-Path -LiteralPath $path -PathType Leaf)) { throw "Source input outside this repository or absent: $path" }
        $relative = [IO.Path]::GetRelativePath($repositoryRoot,$path)
        if ($relative -match '(^|[\\/])(\.artifacts|target|bin|obj)([\\/]|$)') { throw 'Generated application source/build references are forbidden.' }
        $entry = Get-Item -LiteralPath $path -Force
        while ($entry -and $entry.FullName.StartsWith($boundary,[StringComparison]::OrdinalIgnoreCase)) {
            if ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Reparse source dependency rejected.' }
            $entry = if ($entry -is [IO.DirectoryInfo]) { $entry.Parent } else { $entry.Directory }
        }
    }
}
if ($InputsOnly) { Write-Output 'Evaluated managed compilation inputs are local.'; return }
# Actual parser/hash behavior must remain byte-compatible after relocation.
$oracle = Join-Path $env:ACHIMELD_ARTIFACT_ROOT 'migration-golden.json'
& dotnet run --project (Join-Path $repositoryRoot 'tools/legacy-oracle/LegacyOracle.csproj') -c Release -- (Join-Path $repositoryRoot 'tests/fixtures/steamworks/normalized-2026-09.html') $oracle
if ($LASTEXITCODE -ne 0) { throw 'Local compatibility oracle failed.' }
$actual = Get-Content -LiteralPath $oracle -Raw | ConvertFrom-Json
$expected = Get-Content -LiteralPath (Join-Path $repositoryRoot 'tests/fixtures/steamworks/legacy-golden.json') -Raw | ConvertFrom-Json
foreach ($field in @('snapshots','decimals','unicodeFingerprint','workerReadRequestFrameHex')) {
    if (($actual.$field | ConvertTo-Json -Depth 20 -Compress) -cne ($expected.$field | ConvertTo-Json -Depth 20 -Compress)) { throw "Compatibility changed after migration: $field" }
}
Write-Output "All $($projects.Count) managed projects use local source; compatibility vectors are unchanged."
