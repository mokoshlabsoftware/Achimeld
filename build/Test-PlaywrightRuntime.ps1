[CmdletBinding()]
param([Parameter(Mandatory)][string]$WorkerDirectory)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'BuildPaths.ps1')
. (Join-Path $PSScriptRoot 'PlaywrightRuntime.ps1')
Initialize-AchimeldBuildPaths
$worker = [IO.Path]::GetFullPath($WorkerDirectory)
$boundary = [IO.Path]::GetFullPath($env:ACHIMELD_ARTIFACT_ROOT) + [IO.Path]::DirectorySeparatorChar
if (!$worker.StartsWith($boundary, [StringComparison]::OrdinalIgnoreCase)) { throw 'Runtime fixture must use external current build storage.' }
$evidence = Get-Content -LiteralPath (Join-Path $worker 'read-worker-provenance.json') -Raw | ConvertFrom-Json
$patch = $evidence.playwrightRuntimePatch
if ($patch.id -cne 'supervised-windows-cleanup-v1' -or $patch.path -cne '.playwright/package/lib/coreBundle.js') { throw 'Reviewed runtime patch provenance missing.' }
$path = Join-Path $worker $patch.path
$hash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
$inventory = @($evidence.inputs | Where-Object path -CEQ $patch.path)
if ($hash -cne $patch.patchedSha256 -or $inventory.Count -ne 1 -or $hash -cne $inventory[0].sha256) { throw 'Patched runtime inventory mismatch.' }
$script = @($evidence.source | Where-Object path -CEQ 'build/PlaywrightRuntime.ps1')
if ($script.Count -ne 1 -or $script[0].sha256 -cne (Get-FileHash -LiteralPath (Join-Path $PSScriptRoot 'PlaywrightRuntime.ps1') -Algorithm SHA256).Hash.ToLowerInvariant()) { throw 'Runtime patch source differs from worker provenance.' }
# Neither dependency drift nor a second application may silently change the bundle.
$fixture = Join-Path $env:ACHIMELD_ARTIFACT_ROOT ('playwright-patch-fixture-' + [Guid]::NewGuid().ToString('N'))
$copy = Join-Path $fixture $patch.path
New-Item -ItemType Directory -Path (Split-Path -Parent $copy) -Force | Out-Null
Copy-Item -LiteralPath $path -Destination $copy
foreach ($altered in @($false, $true)) {
    if ($altered) { [IO.File]::AppendAllText($copy, "`n// Changed dependency fixture`n") }
    $before = (Get-FileHash -LiteralPath $copy -Algorithm SHA256).Hash
    $rejected = $false
    try { $null = Set-AchimeldPlaywrightRuntime -PackageDirectory $fixture }
    catch {
        if (!$_.Exception.Message.StartsWith('Unrecognized Playwright 1.62.0 runtime;', [StringComparison]::Ordinal)) { throw }
        $rejected = $true
    }
    if (!$rejected -or (Get-FileHash -LiteralPath $copy -Algorithm SHA256).Hash -cne $before) { throw 'Unknown or already patched runtime was modified.' }
}
Write-Output 'Patched runtime/source provenance and fail-closed dependency drift checks passed.'
