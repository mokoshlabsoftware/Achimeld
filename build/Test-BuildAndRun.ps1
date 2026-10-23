[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$repository = Split-Path -Parent $PSScriptRoot
$fixture = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) ('achimeld-launch-' + [Guid]::NewGuid().ToString('N'))))
$savedRoot = $env:ACHIMELD_BUILD_ROOT
$savedRecord = $env:ACHIMELD_LAUNCH_FIXTURE
try {
    New-Item -ItemType Directory -Path $fixture | Out-Null
    $runner = Join-Path $fixture 'Record-Launch.ps1'
    $record = Join-Path $fixture 'launch.json'
    $env:ACHIMELD_LAUNCH_FIXTURE = $record
    $env:ACHIMELD_BUILD_ROOT = Join-Path $fixture "build root's space"
    [IO.File]::WriteAllText($runner, @'
[CmdletBinding()]
param([switch]$Offline, [switch]$BuildOnly, [switch]$RunCurrent)
@{Offline=[bool]$Offline;BuildOnly=[bool]$BuildOnly;RunCurrent=[bool]$RunCurrent;Root=$env:ACHIMELD_BUILD_ROOT} |
    ConvertTo-Json | Set-Content -LiteralPath $env:ACHIMELD_LAUNCH_FIXTURE
'@)
    $tokens = $null; $errors = $null
    $ast = [Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot 'Publish-CurrentBuild.ps1'), [ref]$tokens, [ref]$errors)
    if ($errors.Count) { throw 'Launcher generator syntax rejected.' }
    # Evaluate the actual generator expressions, then run their output against a
    # recorder. No compiler, package executable, installer or elevation is used.
    foreach ($name in @('launcher', 'buildLauncher')) {
        $assignment = $ast.Find({ param($node)
            $node -is [Management.Automation.Language.AssignmentStatementAst] -and
            $node.Left -is [Management.Automation.Language.VariableExpressionAst] -and
            $node.Left.VariablePath.UserPath -eq $name
        }, $true)
        if (!$assignment) { throw "Missing generated launcher: $name" }
        . ([scriptblock]::Create($assignment.Extent.Text))
        $path = Join-Path $fixture ($name + '.ps1')
        [IO.File]::WriteAllText($path, (Get-Variable -Name $name -ValueOnly))
        & $path
        $actual = Get-Content -LiteralPath $record -Raw | ConvertFrom-Json
        if ($actual.Root -cne $env:ACHIMELD_BUILD_ROOT -or $actual.Offline -or $actual.BuildOnly -or
            $actual.RunCurrent -ne ($name -eq 'launcher')) { throw 'Generated launcher changed its local launch recipe.' }
        if ($name -eq 'buildLauncher') {
            & $path -Offline -BuildOnly
            $actual = Get-Content -LiteralPath $record -Raw | ConvertFrom-Json
            if (!$actual.Offline -or !$actual.BuildOnly -or $actual.RunCurrent) { throw 'Build options were not forwarded.' }
        }
        [IO.File]::WriteAllText($record, 'untouched')
        $rejected = $false
        try { & $path -Protected } catch [Management.Automation.ParameterBindingException] { $rejected = $true }
        if (!$rejected -or [IO.File]::ReadAllText($record) -cne 'untouched') { throw 'Generated launcher accepted installation mode.' }
    }
    $rejected = $false
    try { & (Join-Path $PSScriptRoot 'Build-And-Run.ps1') -Protected }
    catch { $rejected = $_.Exception.Message -eq 'Build and Run launches from current build output only. Use the separate installer for ready builds.' }
    if (!$rejected) { throw 'Source runner accepted installation mode.' }
    Write-Output 'Generated run/build launchers preserve local options and reject installation mode; source runner rejects it before build or launch.'
} finally {
    $env:ACHIMELD_BUILD_ROOT = $savedRoot
    $env:ACHIMELD_LAUNCH_FIXTURE = $savedRecord
    $temporaryRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
    if (!$fixture.StartsWith($temporaryRoot, [StringComparison]::OrdinalIgnoreCase) -or
        (Split-Path -Leaf $fixture) -notmatch '^achimeld-launch-[a-f0-9]{32}$') { throw 'Launch fixture cleanup boundary rejected.' }
    if (Test-Path -LiteralPath $fixture) { Remove-Item -LiteralPath $fixture -Recurse -Force }
}
