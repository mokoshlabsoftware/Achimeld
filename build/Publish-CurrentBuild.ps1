[CmdletBinding()]
param([Parameter(Mandatory)][string]$PackagePath, [switch]$PassThru)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'BuildPaths.ps1')
Initialize-AchimeldBuildPaths
$package = (Resolve-Path -LiteralPath $PackagePath).Path
$current = $env:ACHIMELD_ARTIFACT_ROOT
if (!$package.StartsWith($current + '\', [StringComparison]::OrdinalIgnoreCase) -or
    (Split-Path -Parent $package) -ine $current -or (Split-Path -Leaf $package) -notmatch '^bridge-package-[a-f0-9]{32}$') {
    throw 'Only a fresh current build package can be published.'
}
# Verify the exact candidate before replacing the current build.
& (Join-Path $package 'Achimeld.exe') --verify | Out-Host
if ($LASTEXITCODE -ne 0) { throw 'Package verification failed; current build was not replaced.' }
$destination = Join-Path $current 'package'
. (Join-Path $PSScriptRoot 'DevelopmentProcesses.ps1')
# Installer builds bypass Build-And-Run. Release idle connectors here too,
# immediately before replacing the package they may still have loaded.
Stop-AchimeldIdleDevelopmentMcp -PackagePath $destination
$previous = Join-Path $current ('previous-' + [Guid]::NewGuid().ToString('N'))
if (Test-Path -LiteralPath $destination) { Move-Item -LiteralPath $destination -Destination $previous }
try { Move-Item -LiteralPath $package -Destination $destination }
catch {
    if (Test-Path -LiteralPath $previous) { Move-Item -LiteralPath $previous -Destination $destination }
    throw
}
$repository = Split-Path -Parent $PSScriptRoot
$runner = Join-Path $PSScriptRoot 'Build-And-Run.ps1'
$launcher = "[CmdletBinding()]`nparam()`n`$ErrorActionPreference = 'Stop'`n`$env:ACHIMELD_BUILD_ROOT = '" + $env:ACHIMELD_BUILD_ROOT.Replace("'", "''") + "'`n& '" + $runner.Replace("'", "''") + "' -RunCurrent`n"
[IO.File]::WriteAllText((Join-Path $current 'Run-Achimeld.ps1'), $launcher, [Text.UTF8Encoding]::new($false))
$buildLauncher = "[CmdletBinding()]`nparam([switch]`$Offline, [switch]`$BuildOnly)`n`$ErrorActionPreference = 'Stop'`n`$env:ACHIMELD_BUILD_ROOT = '" + $env:ACHIMELD_BUILD_ROOT.Replace("'", "''") + "'`n& '" + $runner.Replace("'", "''") + "' -Offline:`$Offline -BuildOnly:`$BuildOnly`n"
[IO.File]::WriteAllText((Join-Path $current 'Build-And-Run.ps1'), $buildLauncher, [Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText((Join-Path $current 'last-bridge-package.txt'), $destination, [Text.UTF8Encoding]::new($false))
& (Join-Path $PSScriptRoot 'Clear-PreviousPackages.ps1') -TransientPath $previous | Out-Host
Write-Host "Current build: $destination"
if ($PassThru) {
    [pscustomobject]@{ PackagePath = $destination }
} else {
    $destination
}
