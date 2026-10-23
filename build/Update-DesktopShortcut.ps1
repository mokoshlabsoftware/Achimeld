[CmdletBinding()]
param([Parameter(Mandatory)][string]$CurrentBuildPath)
$ErrorActionPreference = 'Stop'
$launcher = (Resolve-Path -LiteralPath (Join-Path $CurrentBuildPath 'Run-Achimeld.ps1')).Path
$bootstrapper = (Resolve-Path -LiteralPath (Join-Path $CurrentBuildPath 'package/Achimeld.exe')).Path
$desktop = [Environment]::GetFolderPath([Environment+SpecialFolder]::DesktopDirectory)
if (!$desktop -or !(Test-Path -LiteralPath $desktop -PathType Container)) {
    throw 'The current user desktop directory is unavailable.'
}
$shortcutPath = Join-Path $desktop 'achimeld.lnk'
$workingDirectory = [Environment]::GetFolderPath([Environment+SpecialFolder]::UserProfile)
$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut($shortcutPath)
$shortcut.TargetPath = Join-Path $PSHOME 'pwsh.exe'
$shortcut.Arguments = '-NoLogo -NoProfile -WindowStyle Hidden -File "' + $launcher + '"'
$shortcut.WorkingDirectory = $workingDirectory
$shortcut.IconLocation = $bootstrapper + ',0'
$shortcut.Description = 'Run the current verified Achimeld build without administrator approval.'
$shortcut.WindowStyle = 1
$shortcut.Save()
Write-Host ('Desktop shortcut updated: ' + $shortcutPath)
$buildLauncher = (Resolve-Path -LiteralPath (Join-Path (Split-Path -Parent $PSScriptRoot) 'build-and-run.bat')).Path
$buildShortcutPath = Join-Path $desktop 'achimeld build and run.lnk'
$buildShortcut = $shell.CreateShortcut($buildShortcutPath)
$buildShortcut.TargetPath = $buildLauncher
$buildShortcut.Arguments = ''
$buildShortcut.WorkingDirectory = $workingDirectory
$buildShortcut.IconLocation = Join-Path $PSHOME 'pwsh.exe'
$buildShortcut.Description = 'Build into current, archive by date and launch Achimeld without administrator approval.'
$buildShortcut.WindowStyle = 1
$buildShortcut.Save()
Write-Host ('Build-and-run shortcut updated: ' + $buildShortcutPath)
$readme = (Resolve-Path -LiteralPath (Join-Path (Split-Path -Parent $PSScriptRoot) 'README.md')).Path
$documentationShortcutPath = Join-Path $desktop 'achimeld build guide.lnk'
$documentationShortcut = $shell.CreateShortcut($documentationShortcutPath)
$documentationShortcut.TargetPath = $readme
$documentationShortcut.Arguments = ''
$documentationShortcut.WorkingDirectory = Split-Path -Parent $readme
$documentationShortcut.Description = 'Repository build guide, including which generated folders can be deleted.'
$documentationShortcut.Save()
Write-Host ('Build guide shortcut updated: ' + $documentationShortcutPath)
