[CmdletBinding()]
param(
    [string]$SourceRoot = (Split-Path -Parent $PSScriptRoot),
    [string]$OutputRoot
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'BuildPaths.ps1')
if (!$OutputRoot) { $OutputRoot = Join-Path (Get-AchimeldBuildPaths).Artifacts 'development-sources' }

$source = [IO.Path]::GetFullPath($SourceRoot).TrimEnd('\', '/')
$output = [IO.Path]::GetFullPath($OutputRoot).TrimEnd('\', '/')
if ($output.Equals($source, [StringComparison]::OrdinalIgnoreCase) -or
    $output.StartsWith($source + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Development source snapshot must stay outside the repository.'
}
if (!(Test-Path -LiteralPath (Join-Path $source '.git'))) { throw 'Development source must be a Git checkout.' }

function Get-SourceInventory([string]$Root, [string]$Phase) {
    Write-Host "$Phase`: listing source files..."
    $names = (& git -C $Root ls-files --cached --others --exclude-standard -z) -split "`0"
    if ($LASTEXITCODE -ne 0) { throw 'Development source inventory unavailable.' }
    $names = @($names | Where-Object { $_ } | Sort-Object -Unique)
    $completed = 0
    Write-Host "$Phase`: checking $($names.Count) files..."
    @($names | ForEach-Object {
        $relative = $_
        if ([IO.Path]::IsPathRooted($relative) -or
            ($relative -split '[/\\]') -contains '..' -or
            ($relative -split '[/\\]') -contains '.git') { throw 'Development source path rejected.' }
        $path = Join-Path $Root $relative
        $hash = $null
        if (Test-Path -LiteralPath $path) {
            $item = Get-Item -LiteralPath $path -Force
            if ($item.PSIsContainer) { throw "Development source entry is not a file: $relative" }
            while ($item -and $item.FullName.StartsWith($Root, [StringComparison]::OrdinalIgnoreCase)) {
                if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Linked development source rejected: $relative" }
                $item = if ($item -is [IO.FileInfo]) { $item.Directory } else { $item.Parent }
            }
            $hash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
        }
        $completed++
        if ($completed % 100 -eq 0 -or $completed -eq $names.Count) {
            Write-Host "$Phase`: $completed/$($names.Count) files checked."
        }
        [pscustomobject]@{ path = $relative; sha256 = $hash }
    })
}

$head = (& git -C $source rev-parse HEAD | Out-String).Trim()
if ($LASTEXITCODE -ne 0 -or $head -cnotmatch '^[a-f0-9]{40}$') { throw 'Development source commit unavailable.' }
$before = @(Get-SourceInventory $source 'Reading current source')
New-Item -ItemType Directory -Path $output -Force | Out-Null
$snapshot = Join-Path $output ('development-source-' + [guid]::NewGuid().ToString('N'))
Write-Host "Capturing $($before.Count) source files for this build..."
& git -C $source worktree add --detach $snapshot $head | Out-Host
if ($LASTEXITCODE -ne 0) { throw 'Could not create the isolated development source.' }
Write-Host "Copying $($before.Count) source files into the build snapshot..."
$copied = 0
foreach ($file in $before) {
    $target = Join-Path $snapshot $file.path
    if ($null -eq $file.sha256) {
        if (Test-Path -LiteralPath $target) { Remove-Item -LiteralPath $target -Force }
        $copied++
        if ($copied % 100 -eq 0 -or $copied -eq $before.Count) {
            Write-Host "Copying source: $copied/$($before.Count) files processed."
        }
        continue
    }
    New-Item -ItemType Directory -Path (Split-Path -Parent $target) -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $source $file.path) -Destination $target -Force
    # Cargo and MSBuild share external caches across snapshots. Preserved source
    # timestamps can predate a previous build of different bytes, causing stale
    # binaries (including the embedded working version) to be reused.
    [IO.File]::SetLastWriteTimeUtc($target, [DateTime]::UtcNow)
    if ((Get-FileHash -LiteralPath $target -Algorithm SHA256).Hash.ToLowerInvariant() -cne $file.sha256) {
        throw 'Source changed during snapshot capture. Retry after the current edit finishes.'
    }
    $copied++
    if ($copied % 100 -eq 0 -or $copied -eq $before.Count) {
        Write-Host "Copying source: $copied/$($before.Count) files processed."
    }
}
$after = @(Get-SourceInventory $source 'Confirming current source')
$captured = @(Get-SourceInventory $snapshot 'Confirming build snapshot')
if (($before | ConvertTo-Json -Compress) -cne ($after | ConvertTo-Json -Compress) -or
    ($before | ConvertTo-Json -Compress) -cne ($captured | ConvertTo-Json -Compress) -or
    ((& git -C $snapshot rev-parse HEAD | Out-String).Trim()) -cne $head) {
    throw 'Development source changed during snapshot capture. Retry after the current edit finishes.'
}
Write-Host "Build source captured: $snapshot"
Write-Output $snapshot
