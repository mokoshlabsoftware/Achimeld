[CmdletBinding()]
param([Parameter(Mandatory)][string]$OutputDirectory, [switch]$Offline)
$ErrorActionPreference = 'Stop'
$repositoryRoot = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'BuildPaths.ps1')
. (Join-Path $PSScriptRoot 'PlaywrightRuntime.ps1')
Initialize-AchimeldBuildPaths
$output = [IO.Path]::GetFullPath($OutputDirectory)
$boundary = [IO.Path]::GetFullPath(($env:ACHIMELD_ARTIFACT_ROOT)) + [IO.Path]::DirectorySeparatorChar
if (!$output.StartsWith($boundary, [StringComparison]::OrdinalIgnoreCase)) { throw 'Worker output must be inside the external current build directory.' }
if (Test-Path -LiteralPath $output) { throw 'Use a fresh worker output directory.' }
$env:DOTNET_CLI_HOME = Join-Path $env:ACHIMELD_CACHE_ROOT 'dotnet-home'
$env:NUGET_PACKAGES = Join-Path $env:ACHIMELD_CACHE_ROOT 'nuget'
$env:DOTNET_CLI_TELEMETRY_OPTOUT = '1'
$env:DOTNET_GENERATE_ASPNET_CERTIFICATE = 'false'
$source = @(Get-ChildItem -LiteralPath (Join-Path $repositoryRoot 'bridge') -File -Recurse |
    Where-Object { $_.FullName -notmatch '[\\/](bin|obj)[\\/]' -and $_.Extension -in @('.cs','.csproj','.props','.targets','.config','.json') } |
    Sort-Object FullName | ForEach-Object {
        if ($_.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Reparse source input rejected.' }
        [ordered]@{path=[IO.Path]::GetRelativePath($repositoryRoot,$_.FullName).Replace('\','/'); sha256=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()}
    })
$source += [ordered]@{path='build/PlaywrightRuntime.ps1'; sha256=(Get-FileHash -LiteralPath (Join-Path $PSScriptRoot 'PlaywrightRuntime.ps1') -Algorithm SHA256).Hash.ToLowerInvariant()}
& (Join-Path $PSScriptRoot 'Test-Migration.ps1') -InputsOnly
$project = Join-Path $repositoryRoot 'bridge/read-worker/ReadWorker.csproj'
$restore = @('restore',$project,'--locked-mode')
if ($Offline) {
    $config = Join-Path $env:ACHIMELD_ARTIFACT_ROOT 'offline-nuget.config'
    [IO.File]::WriteAllText($config,'<configuration><packageSources><clear /></packageSources></configuration>',[Text.UTF8Encoding]::new($false))
    $restore += @('--configfile',$config,'-p:NuGetAudit=false')
}
& dotnet @restore
if ($LASTEXITCODE -ne 0) { throw 'Locked managed dependency restore failed.' }
& dotnet publish $project -c Release --no-restore -o $output
if ($LASTEXITCODE -ne 0) { throw 'Local read-worker publication failed.' }
$runtimePatch = Set-AchimeldPlaywrightRuntime -PackageDirectory $output
# No standalone dump tool is required by the supervised read capability.
Remove-Item -LiteralPath (Join-Path $output 'createdump.exe') -ErrorAction SilentlyContinue
$allowedExecutables = @('Achimeld.SteamWorker.exe','.playwright/node/win32_x64/node.exe')
foreach ($file in Get-ChildItem -LiteralPath $output -Filter '*.exe' -File -Recurse) {
    if ([IO.Path]::GetRelativePath($output,$file.FullName).Replace('\','/') -cnotin $allowedExecutables) { throw 'Unexpected executable in managed publication.' }
}
foreach ($file in $source) {
    if ((Get-FileHash -LiteralPath (Join-Path $repositoryRoot $file.path) -Algorithm SHA256).Hash.ToLowerInvariant() -ne $file.sha256) {
        throw "Managed source changed during compilation: '$($file.path)'. Wait for source edits to finish, then rerun the build."
    }
}
$inputs = @(Get-ChildItem -LiteralPath $output -File -Recurse | Sort-Object FullName | ForEach-Object {
    [ordered]@{path=[IO.Path]::GetRelativePath($output,$_.FullName).Replace('\','/'); sha256=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()}
})
$evidence = [ordered]@{schemaVersion=2; policy='reviewed-sync-v1'; source=$source; inputs=$inputs;
    playwrightRuntimePatch=$runtimePatch;
    workerAssemblySha256=(Get-FileHash -LiteralPath (Join-Path $output 'Achimeld.SteamWorker.dll') -Algorithm SHA256).Hash.ToLowerInvariant();
    dotnetSdkVersion=((& dotnet --version) | Out-String).Trim();
    build='dotnet publish --no-restore, Release, win-x64, self-contained; locked NuGet dependencies';
    dependencyAdvisories=if($Offline){'not-run (offline publication)'}else{'NuGet restore audit; browser/Node advisories require separate assessment'};
    limitations='Exact source and output hashes establish this local build identity, not independent reproducibility or installed enforcement.'}
[IO.File]::WriteAllText((Join-Path $output 'read-worker-provenance.json'), ($evidence | ConvertTo-Json -Depth 8) + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
