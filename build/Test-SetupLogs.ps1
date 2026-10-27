[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'BuildPaths.ps1')
$run = Join-Path (Get-AchimeldBuildPaths).Artifacts ('setup-log-fixtures-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $run | Out-Null
$tokens = $null; $errors = $null
$ast = [Management.Automation.Language.Parser]::ParseFile((Join-Path (Split-Path -Parent $PSScriptRoot) 'installer/Export-SetupLog.ps1'), [ref]$tokens, [ref]$errors)
if ($errors) { throw 'Setup diagnostic script syntax failed.' }
foreach ($function in $ast.FindAll({ param($node) $node -is [Management.Automation.Language.FunctionDefinitionAst] }, $true)) {
    . ([scriptblock]::Create($function.Extent.Text))
}
# Policy fixtures use synthetic ACLs; filesystem copy/retention is exercised below.
& {
    function Get-Item { param($LiteralPath,[switch]$Force) [pscustomobject]@{Attributes=$script:fixtureAttributes} }
    function Get-Acl { param($LiteralPath) $script:fixtureAcl }
    $script:fixtureAttributes = [IO.FileAttributes]::Normal
    $script:fixtureAcl = New-SetupLogAcl
    Assert-SetupLogEntry 'fixture'
    foreach ($case in @('redirect','owner','write')) {
        $script:fixtureAttributes = [IO.FileAttributes]::Normal
        $script:fixtureAcl = New-SetupLogAcl
        if ($case -eq 'redirect') { $script:fixtureAttributes = [IO.FileAttributes]::ReparsePoint }
        if ($case -eq 'owner') { $script:fixtureAcl.SetOwner([Security.Principal.WindowsIdentity]::GetCurrent().User) }
        if ($case -eq 'write') {
            $script:fixtureAcl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new(
                [Security.Principal.SecurityIdentifier]::new('S-1-5-32-545'),'Write','Allow'))
        }
        $rejected = $false
        try { Assert-SetupLogEntry 'fixture' } catch { $rejected = $true }
        if (!$rejected) { throw "Unsafe diagnostic destination accepted: $case" }
    }
}
$export = $ast.Find({ param($node) $node -is [Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq 'Export-SetupLog' }, $true).Extent.Text
# Replace only the platform root and privileged file creation, never the copy,
# byte limit, retention selection, validation calls or file lifetime logic.
$export = $export.Replace("[Environment]::GetFolderPath('CommonApplicationData')", '$script:fixtureRoot')
. ([scriptblock]::Create($export))
$script:fixtureRoot = $run
$script:rejectEntry = $false
$script:aclApplied = $false
function Open-SetupLogOutput([string]$Path) {
    $script:aclApplied = $true
    return [IO.File]::Open($Path, 'OpenOrCreate', 'Write', 'Read')
}
function Assert-SetupLogParent([string]$Path) { if ($Path -cne $run) { throw 'Fixture escaped its root.' } }
function Assert-SetupLogEntry([string]$Path) {
    if ($script:rejectEntry) { throw 'Synthetic unsafe destination.' }
    if (!$Path.StartsWith($run + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Fixture escaped its root.' }
}
function Initialize-SetupLogDirectory([string]$Path) {
    Assert-SetupLogEntry $Path
    New-Item -ItemType Directory -Path $Path -Force | Out-Null
}
$LogPath = Join-Path $run 'source.log'
$bytes = [byte[]]::new(17MB)
$bytes[0] = 12; $bytes[-1] = 91
[IO.File]::WriteAllBytes($LogPath, $bytes)
$SessionName = 'setup-20261027-123456-1'
Export-SetupLog
$directory = Join-Path $run 'Achimeld/SetupLogs'
$destination = Join-Path $directory ($SessionName + '.log')
$saved = [IO.File]::ReadAllBytes($destination)
if (!$script:aclApplied -or $saved.Length -ne 16MB -or $saved[-1] -ne 91 -or $saved[0] -ne 0) { throw 'Bounded diagnostic tail was not preserved.' }
[IO.File]::WriteAllText($LogPath, 'short replacement')
Export-SetupLog
if ([IO.File]::ReadAllText($destination) -cne 'short replacement') { throw 'Log refresh retained stale bytes.' }
$unrelated = Join-Path $directory 'unrelated.log'
[IO.File]::WriteAllText($unrelated, 'retain')
for ($index = 2; $index -le 8; $index++) {
    $SessionName = 'setup-20261027-123456-' + $index
    Export-SetupLog
}
if (@(Get-ChildItem -LiteralPath $directory -Filter 'setup-*.log').Count -ne 5 -or !(Test-Path -LiteralPath $unrelated)) { throw 'Retention lost unrelated files or exceeded five logs.' }
$script:rejectEntry = $true
$rejected = $false
try { Export-SetupLog } catch { $rejected = $true }
if (!$rejected -or [IO.File]::ReadAllText($LogPath) -cne 'short replacement') { throw 'Failed export did not retain its original log.' }
Write-Output "Setup log ACL policy, redirection rejection, bounded copy, refresh, retention and failure fixtures passed: $run. Actual privileged ACL application is a separate acceptance check."
