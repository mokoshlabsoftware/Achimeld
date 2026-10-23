[CmdletBinding()]
param([switch]$Offline)
$ErrorActionPreference = 'Stop'
$repositoryRoot = Split-Path -Parent $PSScriptRoot
$arguments = @('metadata','--format-version','1','--locked')
if ($Offline) { $arguments += '--offline' }
$metadata = (& (Join-Path $PSScriptRoot 'Rust.ps1') -CargoArguments $arguments | Out-String | ConvertFrom-Json)
$allowed = @{
    'achimeld-ui' = @('achimeld-application','achimeld-core','egui','serde_json')
    'achimeld-application' = @('achimeld-core','serde','serde_json','rand','sha2','hex','rust_decimal')
    'achimeld-imaging' = @('achimeld-application','image','serde_json','skrifa','vello_cpu','flate2')
}
foreach ($name in $allowed.Keys) {
    $package = @($metadata.packages | Where-Object name -CEQ $name)
    if ($package.Count -ne 1) { throw "Missing desktop package: $name" }
    foreach ($dependency in $package[0].dependencies | Where-Object { !$_.kind -or $_.kind -eq 'normal' }) {
        if ($dependency.name -cnotin $allowed[$name]) { throw "Unexpected production authority dependency: $name -> $($dependency.name)" }
    }
    $source = Join-Path $repositoryRoot ('crates/' + $name + '/src')
    $root = Get-Content -LiteralPath (Join-Path $source 'lib.rs') -Raw
    if (!$root.Contains('#![forbid(unsafe_code)]')) { throw "Unsafe code restriction missing in $name" }
    foreach ($file in Get-ChildItem -LiteralPath $source -Filter '*.rs' -Recurse | Where-Object Name -NE 'tests.rs') {
        $text = Get-Content -LiteralPath $file.FullName -Raw
        if ($text -match '(?m)^\s*(?:use|extern\s+crate)\s+(?:std::(?:fs|net|process|io)|achimeld_(?:browser|security|persistence))\b') {
            throw "Host I/O belongs outside $name : $($file.FullName)"
        }
    }
}
Write-Output 'Desktop production dependency and source restrictions passed. This is an architecture drift check, not an OS sandbox or proof against compromised process execution.'
