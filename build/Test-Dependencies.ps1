[CmdletBinding()]
param([switch]$Offline)
$ErrorActionPreference = 'Stop'
$repositoryRoot = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'BuildPaths.ps1')
Initialize-AchimeldBuildPaths
$scanner = Join-Path $env:ACHIMELD_CACHE_ROOT 'audit-tools/cargo-audit-0.22.2/cargo-audit-x86_64-pc-windows-msvc-v0.22.2/cargo-audit.exe'
if (!(Test-Path -LiteralPath $scanner) -or (Get-FileHash -LiteralPath $scanner -Algorithm SHA256).Hash.ToLowerInvariant() -ne '0157f5ce1ce9fd4fb0a1f7c79af1229771d1f80b6c2613ddb0d9200a8ba73946') { throw 'Provision the official pinned scanner with Setup-DependencyAudit.ps1.' }
$database = Join-Path $env:ACHIMELD_CACHE_ROOT 'advisory-db'
$arguments = @('audit','--file',(Join-Path $repositoryRoot 'Cargo.lock'),'--db',$database,'--url','https://github.com/RustSec/advisory-db.git','--json','--no-yanked')
if ($Offline) { $arguments += '--no-fetch' }
# No ignore flags: preserve every raw finding, then apply explicitly reviewed,
# expiring use-specific exceptions to this gate. Yanked registry checks are a separate scope.
$raw = & $scanner @arguments
$scanExit = $LASTEXITCODE
Write-Output ($raw -join "`n")
if (!$raw) { throw 'Advisory scanner produced no report.' }
$report = ($raw -join "`n") | ConvertFrom-Json
if ($scanExit -gt 1 -or !$report.database) { throw 'Dependency scanner failed before producing a complete advisory report.' }
$revision = & git -C $database rev-parse HEAD
if ($LASTEXITCODE -ne 0) { throw 'Advisory database revision unavailable.' }
$committed = & git -C $database show -s --format=%cI HEAD
if ($LASTEXITCODE -ne 0) { throw 'Advisory database commit date unavailable.' }
$commitDate = [DateTimeOffset]::Parse($committed)
if ($commitDate -lt [DateTimeOffset]::UtcNow.AddDays(-14) -or $commitDate -gt [DateTimeOffset]::UtcNow.AddDays(1)) { throw 'Advisory database is stale or has an invalid future date; refresh online.' }
$dirty = & git -C $database status --porcelain --untracked-files=no
if ($LASTEXITCODE -ne 0 -or $dirty) { throw 'Advisory database tracked contents differ from the recorded commit.' }
Write-Output ("Advisory database revision: $revision; committed: $committed; checked UTC: " + [DateTimeOffset]::UtcNow.ToString('O') + '; yanked registry state not checked.')
$exceptionPath = Join-Path $repositoryRoot 'docs/security/dependency-exceptions.json'
$exceptions = if (Test-Path -LiteralPath $exceptionPath) { (Get-Content -LiteralPath $exceptionPath -Raw | ConvertFrom-Json).exceptions } else { @() }
foreach ($source in Get-ChildItem -LiteralPath (Join-Path $repositoryRoot 'crates') -Recurse -Filter '*.rs') {
    $text = Get-Content -LiteralPath $source.FullName -Raw
    $testModule = [regex]::Match($text, '#\[cfg\(test\)\]\s*mod tests\s*\{')
    $production = if ($testModule.Success) { $text.Substring(0, $testModule.Index) } else { $text }
    if ($production -match 'RsaPrivateKey|SigningKey|Pkcs1v15Encrypt|Oaep|\.decrypt\s*\(|\.sign\s*\(') { throw 'Private-key use requires renewed review of the RSA advisory exception.' }
}
$unresolved = [Collections.Generic.List[string]]::new()
foreach ($finding in @($report.vulnerabilities.list)) {
    if (!$finding) { continue }
    $matches = @($exceptions | Where-Object { $_.advisory -ceq $finding.advisory.id -and $_.package -ceq $finding.package.name -and $_.version -ceq $finding.package.version })
    if ($matches.Count -ne 1 -or !$matches[0].owner -or !$matches[0].rationale -or !$matches[0].mitigation -or [DateTimeOffset]($matches[0].expiresUtc) -le [DateTimeOffset]::UtcNow) { $unresolved.Add($finding.advisory.id); continue }
    Write-Output ("Scoped exception $($finding.advisory.id): $($matches[0].rationale); expires $($matches[0].expiresUtc)")
}
# Maintenance/unsound warnings need review too; do not silently discard them.
if ($report.warnings -and $report.warnings.PSObject.Properties.Count) { $unresolved.Add('advisory-warning') }
if ($unresolved.Count) { throw ('Unresolved dependency advisories: ' + ($unresolved -join ', ')) }
if ($scanExit -ne 0 -and !$report.vulnerabilities.found) { throw 'Scanner failed without a classified vulnerability.' }
Write-Output 'Rust lockfile advisory gate passed with the explicitly printed scope and exceptions. Third-party managed/browser runtimes are outside this Rust scan.'
