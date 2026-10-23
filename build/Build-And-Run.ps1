[CmdletBinding()]
param([switch]$Offline, [switch]$BuildOnly, [switch]$RunCurrent, [switch]$Protected)
$ErrorActionPreference = 'Stop'
$repositoryRoot = Split-Path -Parent $PSScriptRoot
# Existing shortcuts may start PowerShell inside current. Release that directory
# before waiting for the app so build storage can be cleaned while it is open.
Set-Location -LiteralPath $repositoryRoot
[Environment]::CurrentDirectory = $repositoryRoot
. (Join-Path $PSScriptRoot 'BuildPaths.ps1')
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
if (([Security.Principal.WindowsPrincipal]::new($identity)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Start Achimeld normally, without administrator privileges.'
}
Initialize-AchimeldBuildPaths
# The signer invokes pwsh by name. Use the same PowerShell selected by the batch
# file for child build tools, including when it was discovered outside PATH.
$env:PATH = $PSHOME + ';' + $env:PATH
$built = $null
if ($RunCurrent) {
    if ($BuildOnly) { throw 'RunCurrent cannot be combined with BuildOnly.' }
    $package = Join-Path $env:ACHIMELD_ARTIFACT_ROOT 'package'
    $built = [pscustomobject]@{ PackagePath = $package; VerifierSha256 = (Get-FileHash -LiteralPath (Join-Path $package 'Achimeld.exe') -Algorithm SHA256).Hash.ToLowerInvariant() }
} else {
Write-Host 'Building the Rust desktop application and its supervised worker...'
& (Join-Path $PSScriptRoot 'BridgePackage.ps1') -Development -Offline:$Offline -PassThru | ForEach-Object {
    if ($_ -is [pscustomobject] -and $_.PSObject.Properties['PackagePath'] -and $_.PSObject.Properties['VerifierSha256']) { $built = $_ }
    else { Write-Host $_ }
}
if (!$built) { throw 'The current build did not produce a verified development package.' }
}
& (Join-Path $built.PackagePath 'Achimeld.exe') --verify
if ($LASTEXITCODE -ne 0) { throw 'Current development package verification failed.' }
if (!$RunCurrent) {
    Write-Host 'Cleaning temporary artifacts from the verified build...'
    & (Join-Path $PSScriptRoot 'Clear-CodeArtifacts.ps1')
}
& (Join-Path $PSScriptRoot 'Update-DesktopShortcut.ps1') -CurrentBuildPath $env:ACHIMELD_ARTIFACT_ROOT
if ($BuildOnly) { Write-Host ('Build complete: ' + $built.PackagePath); return }
$manifest = Get-Content -LiteralPath (Join-Path $built.PackagePath 'manifest.json') -Raw | ConvertFrom-Json
if ($manifest.mode -cne 'development' -or $manifest.product -cne 'Achimeld' -or $manifest.version -notmatch '^\d+\.\d+\.\d+$' -or $manifest.keyId -cnotmatch '^[a-f0-9]{64}$') { throw 'Built package identity rejected.' }
$launchRoot = $built.PackagePath
$launchArguments = @{}
if ($Protected) {
$installationRoot = Join-Path ([Environment]::GetFolderPath([Environment+SpecialFolder]::ProgramFiles)) 'Achimeld-Development'
$installed = Join-Path $installationRoot ($manifest.version + '-' + $manifest.keyId.Substring(0,16))
$bootstrapper = Join-Path $installed 'Achimeld.exe'
if (!(Test-Path -LiteralPath $bootstrapper -PathType Leaf) -or (Get-FileHash -LiteralPath $bootstrapper -Algorithm SHA256).Hash.ToLowerInvariant() -cne $built.VerifierSha256) {
$installer = Join-Path $repositoryRoot 'installer/Install-Development.ps1'
$log = Join-Path $env:ACHIMELD_ARTIFACT_ROOT ('desktop-install-' + [Guid]::NewGuid().ToString('N') + '.log')
# Keep build artifacts owned by the launching user, including installer output.
# The elevated child only appends to this already-created file.
New-Item -ItemType File -Path $log | Out-Null
function Quote-PowerShellLiteral([string]$Value) { return "'" + $Value.Replace("'", "''") + "'" }
$command = '$ErrorActionPreference = ''Stop''; try { & ' + (Quote-PowerShellLiteral $installer) +
    ' -PackagePath ' + (Quote-PowerShellLiteral $built.PackagePath) + ' -ExpectedVerifierSha256 ' +
    (Quote-PowerShellLiteral $built.VerifierSha256) + ' *>> ' + (Quote-PowerShellLiteral $log) +
    '; exit 0 } catch { $_ | Out-String | Add-Content -LiteralPath ' + (Quote-PowerShellLiteral $log) + '; exit 1 }'
$encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($command))
Write-Host 'Windows will request administrator approval to install this build and its network restrictions.'
try {
    $process = Start-Process -FilePath (Join-Path $PSHOME 'pwsh.exe') -ArgumentList @('-NoLogo','-NoProfile','-EncodedCommand',$encoded) -Verb RunAs -WindowStyle Hidden -Wait -PassThru
} catch {
    # Start-Process discards the native error code and inner exception. Match
    # only its launch error and the localized Windows ERROR_CANCELLED suffix.
    $cancelled = [ComponentModel.Win32Exception]::new(1223).Message
    if ($_.FullyQualifiedErrorId -cne 'InvalidOperationException,Microsoft.PowerShell.Commands.StartProcessCommand' -or
        !$_.Exception.Message.EndsWith($cancelled, [StringComparison]::Ordinal)) { throw }
    throw ('Build succeeded, but Windows administrator approval was canceled. The installer did not start and Achimeld was not launched. The verified package and dated archive were kept. Open the achimeld desktop shortcut to retry without rebuilding. Verified package: ' + $built.PackagePath)
}
if ($process.ExitCode -ne 0) { if (Test-Path -LiteralPath $log) { Get-Content -LiteralPath $log | Write-Host }; throw 'Desktop installation failed. The application was not launched.' }
}
$bootstrapper = Join-Path $installed 'Achimeld.exe'
if (!(Test-Path -LiteralPath $bootstrapper -PathType Leaf) -or (Get-FileHash -LiteralPath $bootstrapper -Algorithm SHA256).Hash.ToLowerInvariant() -cne $built.VerifierSha256) { throw 'Installed bootstrapper differs from this trusted build.' }
$launchRoot = $installed
$launchArguments.ArgumentList = '--protected'
}
$bootstrapper = Join-Path $launchRoot 'Achimeld.exe'
if ((Get-FileHash -LiteralPath $bootstrapper -Algorithm SHA256).Hash.ToLowerInvariant() -cne $built.VerifierSha256) { throw 'Bootstrapper differs from this verified build.' }
Write-Host 'Starting Achimeld without administrator privileges...'
$application = Start-Process -FilePath $bootstrapper -WorkingDirectory $launchRoot -NoNewWindow -Wait -PassThru @launchArguments
if ($application.ExitCode -ne 0) { throw 'Achimeld stopped. Check the startup or security diagnostics.' }
