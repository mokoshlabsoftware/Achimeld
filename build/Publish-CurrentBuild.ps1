[CmdletBinding()]
param([Parameter(Mandatory)][string]$PackagePath)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'BuildPaths.ps1')
Initialize-AchimeldBuildPaths
$package = (Resolve-Path -LiteralPath $PackagePath).Path
$current = $env:ACHIMELD_ARTIFACT_ROOT
if (!$package.StartsWith($current + '\', [StringComparison]::OrdinalIgnoreCase) -or
    (Split-Path -Parent $package) -ine $current -or (Split-Path -Leaf $package) -notmatch '^bridge-package-[a-f0-9]{32}$') {
    throw 'Only a fresh current build package can be published.'
}
New-Item -ItemType Directory -Path $env:ACHIMELD_ARCHIVE_ROOT -Force | Out-Null
$archive = Join-Path $env:ACHIMELD_ARCHIVE_ROOT ('build-' + (Get-Date -Format 'HH_mm_ss') + '-' + [Guid]::NewGuid().ToString('N').Substring(0,8))
Copy-Item -LiteralPath $package -Destination $archive -Recurse
# Re-verify every archived inventory entry and signature before promoting current.
& (Join-Path $archive 'Achimeld.exe') --verify | Out-Host
if ($LASTEXITCODE -ne 0) { throw 'Archived package verification failed; current build was not replaced.' }
$destination = Join-Path $current 'package'
$previous = Join-Path $current ('previous-' + [Guid]::NewGuid().ToString('N'))
if (Test-Path -LiteralPath $destination) { Move-Item -LiteralPath $destination -Destination $previous }
try { Move-Item -LiteralPath $package -Destination $destination }
catch {
    if (Test-Path -LiteralPath $previous) { Move-Item -LiteralPath $previous -Destination $destination }
    throw
}
if (Test-Path -LiteralPath $previous) {
    Move-Item -LiteralPath $previous -Destination (Join-Path $env:ACHIMELD_ARCHIVE_ROOT (Split-Path -Leaf $previous))
}
$repository = Split-Path -Parent $PSScriptRoot
$runner = Join-Path $PSScriptRoot 'Build-And-Run.ps1'
$launcher = "[CmdletBinding()]`nparam([switch]`$Protected)`n`$ErrorActionPreference = 'Stop'`n`$env:ACHIMELD_BUILD_ROOT = '" + $env:ACHIMELD_BUILD_ROOT.Replace("'", "''") + "'`n& '" + $runner.Replace("'", "''") + "' -RunCurrent -Protected:`$Protected`n"
[IO.File]::WriteAllText((Join-Path $current 'Run-Achimeld.ps1'), $launcher, [Text.UTF8Encoding]::new($false))
$buildLauncher = "[CmdletBinding()]`nparam([switch]`$Offline, [switch]`$BuildOnly, [switch]`$Protected)`n`$ErrorActionPreference = 'Stop'`n`$env:ACHIMELD_BUILD_ROOT = '" + $env:ACHIMELD_BUILD_ROOT.Replace("'", "''") + "'`n& '" + $runner.Replace("'", "''") + "' -Offline:`$Offline -BuildOnly:`$BuildOnly -Protected:`$Protected`n"
[IO.File]::WriteAllText((Join-Path $current 'Build-And-Run.ps1'), $buildLauncher, [Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText((Join-Path $current 'last-bridge-package.txt'), $destination, [Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText((Join-Path $current 'last-archive.txt'), $archive, [Text.UTF8Encoding]::new($false))
Write-Host "Current build: $destination"
Write-Host "Dated archive: $archive"
$destination
