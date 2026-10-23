[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$repositoryRoot = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'BuildPaths.ps1')
Initialize-AchimeldBuildPaths
$tools = Join-Path $env:ACHIMELD_CACHE_ROOT 'audit-tools'
$archive = Join-Path $tools 'cargo-audit-0.22.2.zip'
$destination = Join-Path $tools 'cargo-audit-0.22.2'
New-Item -ItemType Directory -Path $tools -Force | Out-Null
Invoke-WebRequest -Uri 'https://github.com/rustsec/rustsec/releases/download/cargo-audit/v0.22.2/cargo-audit-x86_64-pc-windows-msvc-v0.22.2.zip' -OutFile $archive
if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne '0a7316540862c13d954f648917ceacca593747baed6eec180fafa590be2710ab') { throw 'Official pinned scanner archive digest mismatch.' }
Expand-Archive -LiteralPath $archive -DestinationPath $destination -Force
Write-Output 'RustSec cargo-audit 0.22.2 provisioned in external build cache. Run Test-Dependencies.ps1 online once to fetch the advisory database.'
