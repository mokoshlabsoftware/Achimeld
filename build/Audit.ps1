[CmdletBinding()]
param(
    [switch]$Fixtures,
    [switch]$Offline,
    [switch]$DependencyAudit,
    [switch]$FailFast,
    [string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
$repositoryRoot = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'BuildPaths.ps1')
Initialize-AchimeldBuildPaths
$artifactRoot = [IO.Path]::GetFullPath(($env:ACHIMELD_ARTIFACT_ROOT))
if (!$OutputDirectory) { $OutputDirectory = Join-Path $artifactRoot ('audit-' + [Guid]::NewGuid().ToString('N')) }
$output = [IO.Path]::GetFullPath($OutputDirectory)
if (!$output.StartsWith($artifactRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Audit evidence must be written inside the external current build directory.'
}
if (Test-Path -LiteralPath $output) { throw 'Use a fresh evidence directory; previous results must not be overwritten.' }
New-Item -ItemType Directory -Path $output -Force | Out-Null
$started = [DateTimeOffset]::UtcNow.ToString('O')
$checks = [Collections.Generic.List[object]]::new()
$env:DOTNET_CLI_HOME = Join-Path $env:ACHIMELD_CACHE_ROOT 'dotnet-home'
$env:NUGET_PACKAGES = Join-Path $env:ACHIMELD_CACHE_ROOT 'nuget'
$env:DOTNET_CLI_TELEMETRY_OPTOUT = '1'
$env:DOTNET_GENERATE_ASPNET_CERTIFICATE = 'false'
$rust = Join-Path $PSScriptRoot 'Rust.ps1'

function Source-Identity {
    $head = & git -C $repositoryRoot rev-parse HEAD
    if ($LASTEXITCODE -ne 0) { throw 'Source commit unavailable.' }
    $status = & git -C $repositoryRoot status --porcelain
    if ($LASTEXITCODE -ne 0) { throw 'Source status unavailable.' }
    $names = (& git -C $repositoryRoot ls-files --cached --others --exclude-standard -z) -split "`0"
    if ($LASTEXITCODE -ne 0) { throw 'Source inventory unavailable.' }
    $files = @($names | Where-Object { $_ } | Sort-Object -Unique | ForEach-Object {
        $path = Join-Path $repositoryRoot $_
        if (Test-Path -LiteralPath $path -PathType Leaf) {
            [ordered]@{ path = $_; sha256 = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() }
        } else { [ordered]@{ path = $_; sha256 = $null } }
    })
    $bytes = [Text.Encoding]::UTF8.GetBytes(($files | ForEach-Object { $_.path + "`0" + $_.sha256 }) -join "`n")
    $hash = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($bytes)).ToLowerInvariant()
    [ordered]@{ commit = "$head".Trim(); dirty = [bool]$status; filesDigestSha256 = $hash; files = $files }
}
function Run-Check([string]$Id, [string]$Scope, [string[]]$Arguments, [scriptblock]$Action) {
    Write-Output "Checking $Id"
    $log = Join-Path $output ($Id + '.log')
    $checkStarted = [DateTimeOffset]::UtcNow.ToString('O')
    $state = 'test-passed'; $exitCode = 0
    try { & $Action *> $log }
    catch { $state = 'test-failed'; $exitCode = 1; Add-Content -LiteralPath $log -Value $_.Exception.Message }
    $checks.Add([ordered]@{ id = $Id; scope = $Scope; status = $state; arguments = $Arguments;
        startedUtc = $checkStarted; completedUtc = [DateTimeOffset]::UtcNow.ToString('O'); exitCode = $exitCode;
        artifact = [IO.Path]::GetFileName($log); artifactSha256 = (Get-FileHash -LiteralPath $log -Algorithm SHA256).Hash.ToLowerInvariant() })
    Write-Output "$Id : $state"
    if ($FailFast -and $state -eq 'test-failed') {
        $planned = @('format','lint','rust-tests','read-policy','browser-policy','audit-contracts','build-storage','working-version','desktop-boundaries','installer-contracts','dependency-inventory','bridge-fixtures','read-worker-build','playwright-runtime','migration-contracts','browser-request-fixtures','dependency-advisories','installed-enforcement','live-steam')
        foreach ($pending in $planned) {
            if ($pending -notin $checks.id) {
                $pendingScope = switch ($pending) {
                    { $_ -in @('bridge-fixtures','browser-request-fixtures') } { 'fixture' }
                    'dependency-advisories' { 'dependency' }
                    'installed-enforcement' { 'installed' }
                    'live-steam' { 'live' }
                    default { 'source' }
                }
                $checks.Add([ordered]@{id=$pending;scope=$pendingScope;status='not-run';reason="Stopped after failed check: $Id";exitCode=$null})
            }
        }
        $report = [ordered]@{schemaVersion=1;startedUtc=$started;completedUtc=[DateTimeOffset]::UtcNow.ToString('O');source=$source;selectedChecksPassed=$false;checks=$checks.ToArray();limitations='Stopped at the first failure; remaining checks were not run. Tool versions are recorded in executed check logs when available.'}
        [IO.File]::WriteAllText((Join-Path $output 'evidence.json'), ($report | ConvertTo-Json -Depth 12) + "`n", [Text.UTF8Encoding]::new($false))
        Write-Output "Failure details: $log"
        Get-Content -LiteralPath $log -Tail 25 | Write-Output
        throw "Check '$Id' failed. See $log and $(Join-Path $output 'evidence.json'). Remaining checks were not run."
    }
}
function Skip-Check([string]$Id, [string]$Scope, [string]$Reason) {
    $checks.Add([ordered]@{ id = $Id; scope = $Scope; status = 'not-run'; reason = $Reason; exitCode = $null })
}

Push-Location $repositoryRoot
try {
    $source = Source-Identity
    $locked = @('--locked'); if ($Offline) { $locked += '--offline' }
    Run-Check 'format' 'source' @('fmt','--all','--check') { & $rust -CargoArguments @('fmt','--all','--check') }
    # Reject invalid source/build layouts before compiling the workspace.
    Run-Check 'audit-contracts' 'source' @('Test-AuditContracts.ps1') { & (Join-Path $PSScriptRoot 'Test-AuditContracts.ps1') }
    Run-Check 'build-storage' 'source' @('Test-BuildStorage.ps1') { & (Join-Path $PSScriptRoot 'Test-BuildStorage.ps1') }
    Run-Check 'working-version' 'source' @('Test-Versioning.ps1') { & (Join-Path $PSScriptRoot 'Test-Versioning.ps1') }
    Run-Check 'installer-contracts' 'source' @('Test-InstallerContracts.ps1') { & (Join-Path $PSScriptRoot 'Test-InstallerContracts.ps1') }
    $lint = @('clippy','--workspace','--all-targets') + $locked + @('--','-D','warnings')
    Run-Check 'lint' 'source' $lint { & $rust -CargoArguments $lint }
    $tests = @('test','--workspace') + $locked
    Run-Check 'rust-tests' 'source' $tests { & $rust -CargoArguments $tests }
    Run-Check 'read-policy' 'source' @('dotnet','run','--project','tools/read-policy-tests','-c','Release') {
        & dotnet run --project tools/read-policy-tests/ReadPolicyTests.csproj -c Release
        if ($LASTEXITCODE -ne 0) { throw 'Receiving read-policy checks failed.' }
    }
    Run-Check 'browser-policy' 'source' @('dotnet','run','--project','tools/browser-policy-tests','-c','Release') {
        & dotnet run --project tools/browser-policy-tests/BrowserPolicyTests.csproj -c Release
        if ($LASTEXITCODE -ne 0) { throw 'Migrated browser/security policy checks failed.' }
    }
    Run-Check 'desktop-boundaries' 'source' @('Test-DesktopBoundaries.ps1') { & (Join-Path $PSScriptRoot 'Test-DesktopBoundaries.ps1') -Offline:$Offline }
    Run-Check 'dependency-inventory' 'source' @('metadata','--format-version','1') {
        & $rust -CargoArguments (@('metadata','--format-version','1') + $locked)
    }
    if ($Fixtures) {
        Run-Check 'bridge-fixtures' 'fixture' @('Test-Bridge.ps1') { & (Join-Path $PSScriptRoot 'Test-Bridge.ps1') }
    } else { Skip-Check 'bridge-fixtures' 'fixture' 'Not selected; use -Fixtures on Windows with local pipe/process/loopback access.' }
    Run-Check 'read-worker-build' 'source' @('Build-ReadWorker.ps1') {
        & (Join-Path $PSScriptRoot 'Build-ReadWorker.ps1') -Offline:$Offline -OutputDirectory (Join-Path $output 'read-worker')
    }
    Run-Check 'playwright-runtime' 'source' @('Test-PlaywrightRuntime.ps1') {
        & (Join-Path $PSScriptRoot 'Test-PlaywrightRuntime.ps1') -WorkerDirectory (Join-Path $output 'read-worker')
    }
    Run-Check 'migration-contracts' 'source' @('Test-Migration.ps1') { & (Join-Path $PSScriptRoot 'Test-Migration.ps1') }
    if ($Fixtures) {
        Run-Check 'browser-request-fixtures' 'fixture' @('Test-BrowserRequests.ps1') {
            & (Join-Path $PSScriptRoot 'Test-BrowserRequests.ps1') -WorkerDirectory (Join-Path $output 'read-worker') -Offline:$Offline
        }
    } else { Skip-Check 'browser-request-fixtures' 'fixture' 'Not selected; fixed pinned Chromium, synthetic pages, isolated Job and rejecting loopback proxy; no Steam session.' }
    if ($DependencyAudit) {
        Run-Check 'dependency-advisories' 'dependency' @('Test-Dependencies.ps1') { & (Join-Path $PSScriptRoot 'Test-Dependencies.ps1') -Offline:$Offline }
    } else { Skip-Check 'dependency-advisories' 'dependency' 'Not selected; advisory freshness is separate from a locked dependency inventory.' }
    Skip-Check 'installed-enforcement' 'installed' 'Separate protected-package, normal-user acceptance after administrator installation.'
    Skip-Check 'live-steam' 'live' 'Separate explicitly scoped authorized Steam workflow; no live actions in this command.'
    $after = Source-Identity
    if ($after.filesDigestSha256 -ne $source.filesDigestSha256 -or $after.commit -ne $source.commit) {
        $beforeFiles = @{}; $afterFiles = @{}
        foreach ($file in $source.files) { $beforeFiles[$file.path] = $file.sha256 }
        foreach ($file in $after.files) { $afterFiles[$file.path] = $file.sha256 }
        $changed = @(@($beforeFiles.Keys) + @($afterFiles.Keys) | Sort-Object -Unique | Where-Object { !$beforeFiles.ContainsKey($_) -or !$afterFiles.ContainsKey($_) -or $beforeFiles[$_] -cne $afterFiles[$_] })
        Write-Output 'Source files changed during the audit. This run cannot be packaged. Use an isolated installer build or stop editing the source during a standalone audit.'
        foreach ($path in $changed) { Write-Output "Changed source: $path" }
        $checks.Add([ordered]@{ id='source-stability'; scope='source'; status='test-failed'; reason='Source changed during checks.'; changedFiles=$changed; beforeCommit=$source.commit; afterCommit=$after.commit; exitCode=1 })
    }
    $compiler = Join-Path $env:ACHIMELD_CACHE_ROOT 'rustup/toolchains/1.99.0-x86_64-pc-windows-msvc/bin/rustc.exe'
    $rustVersion = if (Test-Path -LiteralPath $compiler) { & $compiler --version } else { 'See rust-toolchain.toml and check logs.' }
    $dotnetVersion = & dotnet --version
    $passed = !@($checks | Where-Object status -eq 'test-failed').Count
    $report = [ordered]@{ schemaVersion=1; startedUtc=$started; completedUtc=[DateTimeOffset]::UtcNow.ToString('O');
        source=$source; platform=[Environment]::OSVersion.VersionString; rustVersion="$rustVersion"; dotnetVersion="$dotnetVersion";
        selectedChecksPassed=$passed; checks=$checks.ToArray(); limitations='Local evidence is not independently attested; not-run scopes are not passing qualification.' }
    [IO.File]::WriteAllText((Join-Path $output 'evidence.json'), ($report | ConvertTo-Json -Depth 12) + "`n", [Text.UTF8Encoding]::new($false))
    Write-Output "Audit evidence: $output"
    if (!$passed) {
        foreach ($failure in @($checks | Where-Object status -eq 'test-failed')) { Write-Output "Failed check: $($failure.id)" }
        throw "Audit failed. See $(Join-Path $output 'evidence.json') for the named failures and their check logs."
    }
} finally { Pop-Location }
