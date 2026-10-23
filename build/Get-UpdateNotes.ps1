[CmdletBinding()]
param(
    [string]$SinceVersion,
    [string]$TargetVersion
)
$ErrorActionPreference = 'Stop'
# Keep the embedded Help history and machine-readable notes on one source.
& (Join-Path $PSScriptRoot 'Update-Version.ps1') -Kind Check | Out-Null
$history = Get-Content -LiteralPath (Join-Path (Split-Path -Parent $PSScriptRoot) 'CHANGELOG.md') -Raw
$entries = [regex]::Matches($history, '(?m)^## (\d+\.\d+\.\d+\.\d+) \((\d{4}-\d{2}-\d{2})\)\r?$')
$versions = @($entries | ForEach-Object { $_.Groups[1].Value })
if (!$TargetVersion) { $TargetVersion = $versions[0] }
foreach ($value in @($TargetVersion, $SinceVersion)) {
    if ($value -and ($value -notmatch '^\d+\.\d+\.\d+\.\d+$' -or $value -cnotin $versions)) {
        throw "Version is not present in the recorded history: $value"
    }
}
if ($SinceVersion -and [version]$SinceVersion -gt [version]$TargetVersion) { throw 'Installed version is newer than the target version.' }
$releases = @(
    for ($i = 0; $i -lt $entries.Count; $i++) {
        $entry = $entries[$i]
        $version = $entry.Groups[1].Value
        if ([version]$version -gt [version]$TargetVersion) { continue }
        if ($SinceVersion -and [version]$version -le [version]$SinceVersion) { continue }
        $end = if ($i + 1 -lt $entries.Count) { $entries[$i + 1].Index } else { $history.Length }
        $body = $history.Substring($entry.Index + $entry.Length, $end - $entry.Index - $entry.Length)
        [ordered]@{
            version = $version
            date = $entry.Groups[2].Value
            improvements = @([regex]::Matches($body, '(?m)^- (.+?)\r?$') | ForEach-Object { $_.Groups[1].Value })
        }
    }
)
[ordered]@{
    schemaVersion = 1
    targetVersion = $TargetVersion
    sinceVersion = $(if ($SinceVersion) { $SinceVersion } else { $null })
    releases = $releases
} | ConvertTo-Json -Depth 6
