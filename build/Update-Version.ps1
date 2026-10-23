[CmdletBinding()]
param(
    [ValidateSet('Check', 'Change', 'Commit')][string]$Kind = 'Check',
    [string]$Summary
)
$ErrorActionPreference = 'Stop'
$historyPath = Join-Path (Split-Path -Parent $PSScriptRoot) 'CHANGELOG.md'
$access = if ($Kind -eq 'Check') { [IO.FileAccess]::Read } else { [IO.FileAccess]::ReadWrite }
# Hold the file across the read/update so concurrent agents cannot overwrite an entry.
$historyFile = [IO.File]::Open($historyPath, [IO.FileMode]::Open, $access, [IO.FileShare]::Read)
try {
$reader = [IO.StreamReader]::new($historyFile, [Text.Encoding]::UTF8, $true, 1024, $true)
try { $history = $reader.ReadToEnd() } finally { $reader.Dispose() }
$pattern = '(?m)^## ((?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)) \((\d{4}-\d{2}-\d{2})\)\r?$'
$entries = [regex]::Matches($history, $pattern)
if (!$entries.Count -or [regex]::Matches($history, '(?m)^## ').Count -ne $entries.Count) {
    throw 'Change log must contain four-part version headings with ISO dates.'
}
$previous = $null
for ($entryIndex = 0; $entryIndex -lt $entries.Count; $entryIndex++) {
    $entry = $entries[$entryIndex]
    $version = [version]::Parse($entry.Groups[1].Value)
    [void][datetime]::ParseExact($entry.Groups[2].Value, 'yyyy-MM-dd', [Globalization.CultureInfo]::InvariantCulture)
    if ($null -ne $previous -and $version -ge $previous) { throw 'Change log versions must be unique and newest first.' }
    $previous = $version
    $end = if ($entryIndex -eq $entries.Count - 1) { $history.Length } else { $entries[$entryIndex + 1].Index }
    if ($history.Substring($entry.Index + $entry.Length, $end - $entry.Index - $entry.Length) -notmatch '(?m)^- \S') {
        throw 'Each version entry must describe at least one change.'
    }
}
$current = [version]::Parse($entries[0].Groups[1].Value)
if ($Kind -eq 'Check') { Write-Output "Working version: $current"; return }
if ([string]::IsNullOrWhiteSpace($Summary) -or $Summary -match '[\r\n]') {
    throw 'Supply a nonempty, single-line change summary.'
}
$parts = @($current.Major, $current.Minor, $current.Build, $current.Revision)
$index = if ($Kind -eq 'Commit') { 2 } else { 3 }
if ($parts[$index] -eq [int]::MaxValue) { throw 'Version counter exhausted; choose the next release series explicitly.' }
$parts[$index]++
if ($Kind -eq 'Commit') { $parts[3] = 0 }
$next = $parts -join '.'
$date = [TimeZoneInfo]::ConvertTimeBySystemTimeZoneId([DateTimeOffset]::UtcNow, 'Central European Standard Time').ToString('yyyy-MM-dd', [Globalization.CultureInfo]::InvariantCulture)
$position = $entries[0].Index
$updated = $history.Insert($position, "## $next ($date)`n`n- $($Summary.Trim())`n`n")
# One source file owns both the current version and its immutable history.
$bytes = [Text.UTF8Encoding]::new($false).GetBytes($updated)
$historyFile.Position = 0
$historyFile.Write($bytes, 0, $bytes.Length)
$historyFile.SetLength($bytes.Length)
$historyFile.Flush()
Write-Output "Working version: $current -> $next"
} finally { $historyFile.Dispose() }
