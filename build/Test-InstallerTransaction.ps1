[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$tokens = $null; $errors = $null
$ast = [Management.Automation.Language.Parser]::ParseFile((Join-Path $root 'installer/Install-Development.ps1'), [ref]$tokens, [ref]$errors)
if ($errors) { throw 'Installer entry syntax failed.' }
foreach ($name in @('Assert-NoReparse','New-PackageAcl','Assert-Verifier')) {
    $function = $ast.Find({param($node) $node -is [Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq $name}, $true)
    . ([scriptblock]::Create($function.Extent.Text))
}
. (Join-Path $root 'installer/Install-Transaction.ps1')
foreach ($unsafe in @('C:\','\\server\share\app','C:relative','C:\app:stream','C:\bad"name', (Join-Path $env:windir 'System32\app'))) {
    $rejected = $false
    try { Resolve-InstallationRoot $unsafe | Out-Null } catch { $rejected = $true }
    if (!$rejected) { throw "Unsafe installation destination accepted: $unsafe" }
}
$fixture = Join-Path ([IO.Path]::GetTempPath()) ('achimeld-transaction-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $fixture | Out-Null
# Exercise production filesystem orchestration with synthetic files. Only package
# execution, ACL mutation and registration discovery are mocked; no app/OS policy
# command runs, and no actual installed path is enumerated by these fixtures.
function Get-RegisteredInstallRoot { return $script:registeredRoot }
function Get-AchimeldProcessIds { if ($script:simulateRunning) { 123 } }
function Assert-AdministratorTree([string]$Path) { Assert-NoReparse $Path }
function Assert-InstallationParent([string]$Path) { Assert-NoReparse $Path }
function Set-Acl { param($LiteralPath, $AclObject) }
$nativeFolderPath = [Environment]::GetFolderPath('ProgramFiles')
function Get-ChildItem {
    param([string]$LiteralPath, [switch]$Directory, [switch]$Force, [switch]$Recurse)
    if (!$LiteralPath.StartsWith($fixture, [StringComparison]::OrdinalIgnoreCase)) { return }
    Microsoft.PowerShell.Management\Get-ChildItem @PSBoundParameters
}
function New-VerifiedInstallStage([string]$Stage) {
    New-Item -ItemType Directory -Path $Stage | Out-Null
    foreach ($name in @('manifest.json','manifest.sig','Achimeld.exe','new.txt')) {
        [IO.File]::WriteAllText((Join-Path $Stage $name), ('new-' + $name))
    }
    if ($script:cancelStage -eq 'copy') { [IO.File]::WriteAllText($CancelFile, 'cancel') }
    Assert-NotCancelled
}
function Invoke-PackageCommand([string]$Directory, [string]$Command) {
    $script:operations.Add("$Command|$Directory")
    if ($Command -eq '--verify' -and $script:damaged) { throw 'Synthetic damaged package.' }
    if ($Command -eq '--check-policy' -and $script:missingPolicy) { throw 'Synthetic absent policy.' }
    if ($Command -eq '--install-policy' -and $Directory -ieq (Join-Path $installationRoot 'Application') -and $script:failPolicy) {
        $script:failPolicy = $false
        throw 'Synthetic policy failure.'
    }
}
function Write-InstallProgress([string]$Stage, [int]$Position = 0, [int]$Total = 0, [string]$Detail = '') {
    if ($Stage -eq $script:cancelStage) { [IO.File]::WriteAllText($CancelFile, 'cancel') }
}
function Assert-Condition([bool]$Condition, [string]$Message) { if (!$Condition) { throw $Message } }
try {
    $script:simulateRunning = $true
    $blocked = $false
    try { Assert-AchimeldStopped } catch { $blocked = $true }
    Assert-Condition $blocked 'Running Achimeld did not block installation.'
    $script:simulateRunning = $false
    foreach ($scenario in @('fresh','fresh-cancel','update','update-cancel','late-cancel','policy-failure','late-rollback','repair','repair-rollback','missing-policy','repair-missing-policy','damaged-rejected','relocate','nested-rejected')) {
        $caseRoot = Join-Path $fixture $scenario
        New-Item -ItemType Directory -Path $caseRoot | Out-Null
        $script:registeredRoot = Join-Path $caseRoot 'Old Achimeld'
        $installationRoot = $script:registeredRoot
        $PreviousInstallationRoot = $script:registeredRoot
        $CancelFile = Join-Path $caseRoot 'cancel'
        $script:cancelStage = ''
        $script:damaged = $scenario -in @('repair','repair-rollback','repair-missing-policy','damaged-rejected')
        $script:missingPolicy = $scenario -in @('missing-policy','repair-missing-policy')
        $script:failPolicy = $scenario -eq 'policy-failure'
        $script:operations = [Collections.Generic.List[string]]::new()
        $hadOld = $scenario -notin @('fresh','fresh-cancel')
        if ($hadOld) {
            $oldApp = Join-Path $script:registeredRoot 'Application'
            New-Item -ItemType Directory -Path $oldApp -Force | Out-Null
            foreach ($name in @('manifest.json','manifest.sig','Achimeld.exe','old.txt')) {
                $prefix = if ($scenario -in @('repair','repair-rollback','repair-missing-policy') -and $name -in @('manifest.json','manifest.sig')) { 'new-' } else { 'old-' }
                [IO.File]::WriteAllText((Join-Path $oldApp $name), ($prefix + $name))
            }
        }
        if ($scenario -eq 'relocate') { $installationRoot = Join-Path $caseRoot 'New Achimeld ż' }
        if ($scenario -eq 'nested-rejected') { $installationRoot = Join-Path $oldApp 'Nested' }
        if ($scenario -eq 'fresh-cancel') { $script:cancelStage = 'copy' }
        if ($scenario -eq 'update-cancel') { $script:cancelStage = 'Keeping previous files for rollback' }
        if ($scenario -eq 'late-cancel') { $script:cancelStage = 'Installing network policy' }
        $transactionRoot = Join-Path $installationRoot '.setup-transaction'
        $failed = $false
        $failure = ''
        try { Start-InstallTransaction } catch { $failed = $true; $failure = $_.Exception.Message + ' ' + $_.ScriptStackTrace }
        $expectFailure = $scenario -in @('fresh-cancel','update-cancel','late-cancel','policy-failure','missing-policy','repair-missing-policy','damaged-rejected','nested-rejected')
        Assert-Condition ($failed -eq $expectFailure) "Unexpected prepare result: ${scenario}: $failure"
        if ($scenario -in @('late-rollback','repair-rollback')) { Restore-InstallTransaction }
        elseif (!$failed) {
            Assert-Condition (Test-Path -LiteralPath (Join-Path $installationRoot 'Application\new.txt')) "Replacement absent: $scenario"
            if ($hadOld) { Assert-Condition (Test-Path -LiteralPath $state.entries[0].backup) "Rollback copy absent: $scenario" }
            Complete-InstallTransaction
            Assert-Condition (!(Test-Path -LiteralPath $transactionRoot)) "Committed transaction retained: $scenario"
            $rollbackRejected = $false
            try { Restore-InstallTransaction } catch { $rollbackRejected = $true }
            Assert-Condition $rollbackRejected "Committed transaction allowed rollback: $scenario"
        }
        if (($failed -or $scenario -in @('late-rollback','repair-rollback')) -and $hadOld) {
            Assert-Condition ((Get-Content -LiteralPath (Join-Path $oldApp 'old.txt') -Raw) -ceq 'old-old.txt') "Previous bytes not restored: $scenario"
            Assert-Condition (!(Test-Path -LiteralPath (Join-Path $oldApp 'new.txt'))) "Replacement leaked after rollback: $scenario"
        }
        if ($scenario -eq 'fresh-cancel') { Assert-Condition (!(Test-Path -LiteralPath $installationRoot)) 'Cancelled fresh root retained.' }
        if ($scenario -in @('update-cancel','late-cancel','policy-failure','late-rollback')) {
            Assert-Condition ($script:operations.Contains("--install-policy|$oldApp")) "Previous policy not restored: $scenario"
        }
        if ($script:missingPolicy) {
            Assert-Condition (!($script:operations | Where-Object { $_ -match '^--(install|remove)-policy\|' })) "Rejected incomplete policy was modified: $scenario"
        }
    }
    # Recovery metadata cannot broaden deletion to unrelated directories.
    $installationRoot = Join-Path $fixture 'guard'
    $state = @{id='a' * 32;previousRoot=$installationRoot}
    $rejected = $false
    try { Assert-TransactionEntry @{original=(Join-Path $fixture 'unrelated');backup=(Join-Path $fixture '.backup')} } catch { $rejected = $true }
    Assert-Condition $rejected 'Arbitrary recovery path accepted.'
    Write-Output 'Installer transaction fixtures passed: fresh install, update, repair, relocation, cancellation, policy failure, rollback, commit and path rejection. Package execution/ACL changes are mocked; installed Windows acceptance remains separate.'
} finally {
    $resolved = [IO.Path]::GetFullPath($fixture)
    $temporaryRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
    if (!$resolved.StartsWith($temporaryRoot, [StringComparison]::OrdinalIgnoreCase) -or (Split-Path -Leaf $resolved) -notmatch '^achimeld-transaction-[a-f0-9]{32}$') { throw 'Fixture cleanup boundary rejected.' }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
