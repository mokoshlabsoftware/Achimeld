$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'DevelopmentProcesses.ps1')
$package = 'C:\Fixture\current\package'
function Assert-Connector([string]$Path, [string]$CommandLine, [bool]$Expected) {
    $actual = Test-AchimeldMcpProcess ([pscustomobject]@{ ExecutablePath = $Path; CommandLine = $CommandLine }) $package
    if ($actual -ne $Expected) { throw "Unexpected development process classification: $CommandLine" }
}
Assert-Connector "$package\Achimeld.exe" "`"$package\Achimeld.exe`" --mcp" $true
Assert-Connector "$package\Achimeld.exe" "$package\Achimeld.exe --mcp" $true
foreach ($arguments in @('', '--verify', '--mcp --verify', '--protected --mcp', '--mcp-other', '--MCP')) {
    Assert-Connector "$package\Achimeld.exe" "`"$package\Achimeld.exe`" $arguments" $false
}
Assert-Connector "$package\Achimeld.App.exe" "`"$package\Achimeld.App.exe`" --mcp" $false
Assert-Connector "$package-other\Achimeld.exe" "`"$package-other\Achimeld.exe`" --mcp" $false
Assert-Connector "$package\Achimeld.exe" '"C:\Other\Achimeld.exe" --mcp' $false

# Exercise cleanup selection without terminating real user processes.
$script:rows = @()
$script:killed = @()
$script:timeOffset = 0
function Get-CimInstance { param($ClassName, $Filter) $script:rows }
function Get-Process {
    param($Id, $ErrorAction)
    $row = $script:rows | Where-Object ProcessId -eq $Id
    $process = [pscustomobject]@{ Handle = 1; HasExited = $false; Path = $row.ExecutablePath; StartTime = $row.CreationDate.AddSeconds($script:timeOffset); Id = $Id }
    $process | Add-Member ScriptMethod Kill { $script:killed += $this.Id }
    $process | Add-Member ScriptMethod WaitForExit { param($Timeout) return $true }
    $process | Add-Member ScriptMethod Dispose { }
    return $process
}
$connector = [pscustomobject]@{ ProcessId = 1; ExecutablePath = "$package\Achimeld.exe"; CommandLine = "`"$package\Achimeld.exe`" --mcp"; CreationDate = Get-Date }
$editor = [pscustomobject]@{ ProcessId = 2; ExecutablePath = "$package\Achimeld.App.exe"; CommandLine = "`"$package\Achimeld.App.exe`""; CreationDate = Get-Date }
$script:rows = @($connector, $editor)
Stop-AchimeldIdleDevelopmentMcp $package
if ($script:killed.Count) { throw 'Cleanup touched a live editor session.' }
$supervisor = [pscustomobject]@{ ProcessId = 3; ExecutablePath = "$package\Achimeld.exe"; CommandLine = "`"$package\Achimeld.exe`""; CreationDate = Get-Date }
$script:rows = @($connector, $supervisor)
Stop-AchimeldIdleDevelopmentMcp $package
if ($script:killed.Count) { throw 'Cleanup touched a starting or stopping supervised session.' }
$script:rows = @($connector)
$script:timeOffset = 5
$rejected = $false
try { Stop-AchimeldIdleDevelopmentMcp $package } catch { $rejected = $true }
if (!$rejected -or $script:killed.Count) { throw 'Cleanup did not reject a recycled process ID.' }
$script:timeOffset = 0
Stop-AchimeldIdleDevelopmentMcp $package
if ($script:killed.Count -ne 1 -or $script:killed[0] -ne 1) { throw 'Idle connector was not cleaned up.' }
Write-Output 'Development connector cleanup preserves live sessions and rejects other commands and packages.'
