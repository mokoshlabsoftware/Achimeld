[CmdletBinding()]
param([Parameter(Mandatory)][string]$PackageDirectory, [switch]$Offline)
$ErrorActionPreference = 'Stop'
$repositoryRoot = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'BuildPaths.ps1')
Initialize-AchimeldBuildPaths
$package = [IO.Path]::GetFullPath($PackageDirectory)
$boundary = [IO.Path]::GetFullPath(($env:ACHIMELD_ARTIFACT_ROOT)) + [IO.Path]::DirectorySeparatorChar
if (!$package.StartsWith($boundary,[StringComparison]::OrdinalIgnoreCase) -or !(Test-Path -LiteralPath $package -PathType Container)) { throw 'Browser destination must be an existing current build package.' }
$lockPath = Join-Path $PSScriptRoot 'browser-runtime.lock.json'
$lockHash = (Get-FileHash -LiteralPath $lockPath -Algorithm SHA256).Hash.ToLowerInvariant()
$lock = Get-Content -LiteralPath $lockPath -Raw | ConvertFrom-Json
if ($lock.schemaVersion -ne 1 -or $lock.archiveSha256 -cnotmatch '^[a-f0-9]{64}$' -or $lock.chromiumRevision -notmatch '^[0-9]+$' -or $lock.archiveUrl -notmatch '^https://cdn\.playwright\.dev/builds/cft/[0-9.]+/win64/chrome-win64\.zip$' -or $lock.executable -cne "browser/chromium-$($lock.chromiumRevision)/chrome-win64/chrome.exe") { throw 'Browser runtime lock rejected.' }
$driverManifest = Get-Content -LiteralPath (Join-Path $package '.playwright/package/browsers.json') -Raw | ConvertFrom-Json
$chromium = @($driverManifest.browsers | Where-Object name -ceq 'chromium')
$driverPackage = Get-Content -LiteralPath (Join-Path $package '.playwright/package/package.json') -Raw | ConvertFrom-Json
if ($chromium.Count -ne 1 -or $chromium[0].revision -cne $lock.chromiumRevision -or $chromium[0].browserVersion -cne $lock.chromiumVersion -or $driverPackage.version -cne $lock.playwrightVersion) { throw 'Browser lock does not match the locked Playwright driver.' }
$downloads = Join-Path $env:ACHIMELD_CACHE_ROOT 'downloads'
New-Item -ItemType Directory -Path $downloads -Force | Out-Null
$archive = Join-Path $downloads ($lock.archiveSha256 + '.zip')
if (!(Test-Path -LiteralPath $archive)) {
    if ($Offline) { throw 'Pinned browser archive is not cached. Run package provisioning online once.' }
    $pending = Join-Path $downloads ([Guid]::NewGuid().ToString('N') + '.zip')
    Invoke-WebRequest -Uri $lock.archiveUrl -OutFile $pending
    if ((Get-FileHash -LiteralPath $pending -Algorithm SHA256).Hash.ToLowerInvariant() -cne $lock.archiveSha256) { throw 'Browser download digest mismatch.' }
    Move-Item -LiteralPath $pending -Destination $archive
}
if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant() -cne $lock.archiveSha256) { throw 'Cached browser archive digest mismatch.' }
$destination = Join-Path $package "browser/chromium-$($lock.chromiumRevision)"
if (Test-Path -LiteralPath (Join-Path $package 'browser')) { throw 'Use a fresh browser destination.' }
New-Item -ItemType Directory -Path $destination -Force | Out-Null
$extractBoundary = [IO.Path]::GetFullPath($destination) + [IO.Path]::DirectorySeparatorChar
$names = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
$zip = [IO.Compression.ZipFile]::OpenRead($archive)
try {
    if ($zip.Entries.Count -gt 10000) { throw 'Browser archive entry limit exceeded.' }
    [long]$total = 0
    foreach ($entry in $zip.Entries) {
        $relative = $entry.FullName.Replace('\','/')
        if ([IO.Path]::IsPathRooted($relative) -or $relative.Contains(':') -or ($relative -split '/') -contains '..' -or !$names.Add($relative)) { throw 'Unsafe browser archive path.' }
        $target = [IO.Path]::GetFullPath((Join-Path $destination $relative))
        if (!$target.StartsWith($extractBoundary,[StringComparison]::OrdinalIgnoreCase)) { throw 'Browser archive escapes its destination.' }
        $total += $entry.Length
        if ($total -gt 4GB) { throw 'Browser archive expanded size limit exceeded.' }
        # This reader does not install/update browsers, open PWAs, trace with elevation,
        # register a system browser or emit desktop notifications.
        if ([IO.Path]::GetExtension($relative) -ieq '.exe' -and $relative -cne 'chrome-win64/chrome.exe') { continue }
        if (!$entry.Name) { New-Item -ItemType Directory -Path $target -Force | Out-Null; continue }
        New-Item -ItemType Directory -Path (Split-Path -Parent $target) -Force | Out-Null
        [IO.Compression.ZipFileExtensions]::ExtractToFile($entry,$target,$false)
    }
} finally { $zip.Dispose() }
if (!(Test-Path -LiteralPath (Join-Path $package $lock.executable) -PathType Leaf)) { throw 'Pinned browser executable missing.' }
$files = @(Get-ChildItem -LiteralPath $destination -File -Recurse | Sort-Object FullName | ForEach-Object {
    [ordered]@{path=[IO.Path]::GetRelativePath($package,$_.FullName).Replace('\','/'); sha256=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()}
})
$record = [ordered]@{schemaVersion=1; runtimeLockSha256=$lockHash; archiveUrl=$lock.archiveUrl; archiveSha256=$lock.archiveSha256; executable=$lock.executable; executablePolicy='Only chrome.exe is extracted; installation, elevation, proxy, PWA and notification helpers are omitted.'; files=$files}
[IO.File]::WriteAllText((Join-Path $package 'browser-provenance.json'),($record | ConvertTo-Json -Depth 6)+"`n",[Text.UTF8Encoding]::new($false))
Write-Output 'Pinned Chromium archive verified and extracted; no browser was launched.'
