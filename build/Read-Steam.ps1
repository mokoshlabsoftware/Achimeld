[CmdletBinding()]
param([Parameter(Mandatory)][string]$PackagePath,[Parameter(Mandatory)][string]$AppId,[switch]$Protected)
$ErrorActionPreference='Stop'
$parsed=[uint32]0
if($AppId -notmatch '^[1-9][0-9]*$' -or ![uint32]::TryParse($AppId,[ref]$parsed)){throw 'A canonical nonzero Steam App ID is required.'}
$identity=[Security.Principal.WindowsIdentity]::GetCurrent()
if(([Security.Principal.WindowsPrincipal]::new($identity)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Run Steam reading as a normal user.'}
$root=(Resolve-Path -LiteralPath $PackagePath).Path
$boundary=[IO.Path]::GetFullPath((Join-Path ([Environment]::GetFolderPath([Environment+SpecialFolder]::ProgramFiles)) 'Achimeld-Development'))+'\'
if($Protected -and !$root.StartsWith($boundary,[StringComparison]::OrdinalIgnoreCase)){throw 'Protected reading requires an installed protected development package.'}
$started=[DateTime]::UtcNow
Write-Output ('Reading Steam app '+$AppId+'. Sign in manually in the dedicated browser if requested.')
if ($Protected) { & (Join-Path $root 'Achimeld.exe') --protected --read-steam $AppId }
else { & (Join-Path $root 'Achimeld.exe') --read-steam $AppId }
if($LASTEXITCODE -ne 0){
    $diagnostics=Join-Path $env:LOCALAPPDATA 'Achimeld/RustDiagnostics'
    if(Test-Path -LiteralPath $diagnostics){
        $recent=Get-ChildItem -LiteralPath $diagnostics -Filter 'failure-*.json'|Where-Object LastWriteTimeUtc -ge $started|Sort-Object LastWriteTimeUtc -Descending|Select-Object -First 1
        if($recent){$failure=Get-Content -LiteralPath $recent.FullName -Raw|ConvertFrom-Json;Write-Output ('Stopped at '+$failure.stage+': '+$failure.message)}
    }
    throw 'Supervised Steam read failed; no operation was replayed.'
}
$reads=Join-Path $env:LOCALAPPDATA 'Achimeld/RustReads'
$snapshotFile=Get-ChildItem -LiteralPath $reads -Filter ($AppId+'-*.json')|Where-Object LastWriteTimeUtc -ge $started|Sort-Object LastWriteTimeUtc -Descending|Select-Object -First 1
if(!$snapshotFile){throw 'Successful child exit had no new snapshot.'}
$snapshot=Get-Content -LiteralPath $snapshotFile.FullName -Raw|ConvertFrom-Json
if($snapshot.appId.value -ne $parsed -or $snapshot.revision -ne 'steamworks-configuration-json-v1' -or $snapshot.contentFingerprint -notmatch '^[a-fA-F0-9]{64}$'){throw 'Snapshot identity/revision/hash rejected.'}
$report=[ordered]@{appId=$parsed;package=$root;manifestSha256=(Get-FileHash -LiteralPath (Join-Path $root 'manifest.json')).Hash.ToLowerInvariant();stats=@($snapshot.stats).Count;achievements=@($snapshot.achievements).Count;localizations=@($snapshot.localizations).Count;icons=@($snapshot.icons).Count;contentFingerprint=$snapshot.contentFingerprint;snapshot=$snapshotFile.FullName;completedAtUtc=[DateTime]::UtcNow.ToString('o');operations='Read-only; no portal saves, uploads, deletes or publication exposed by the Rust bridge.'}
$repository=Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'BuildPaths.ps1')
Initialize-AchimeldBuildPaths
$artifacts=$env:ACHIMELD_ARTIFACT_ROOT;New-Item -ItemType Directory -Path $artifacts -Force|Out-Null
[IO.File]::WriteAllText((Join-Path $artifacts 'steam-read-report.json'),($report|ConvertTo-Json)+"`n",[Text.UTF8Encoding]::new($false))
Write-Output ('Read '+$report.stats+' stats and '+$report.achievements+' achievements.')
Write-Output ('Snapshot: '+$snapshotFile.FullName)
