[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$PackagePath,
    [Parameter(Mandatory)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$ExpectedVerifierSha256,
    [string]$InstallationRoot = (Join-Path ([Environment]::GetFolderPath('ProgramFiles')) 'Mokosh Labs\Achimeld'),
    [string]$PreviousInstallationRoot = '',
    [string]$CancelFile = '',
    [ValidateSet('Install','Prepare','Commit','Rollback')][string]$Action = 'Install',
    [switch]$Uninstall
)
$ErrorActionPreference = 'Stop'
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
if (!$principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'Run this installation script with Windows administrator approval. The application itself must run without elevation.' }

function Assert-NoReparse([string]$Path) {
    $candidate = [IO.Path]::GetFullPath($Path)
    while ($candidate) {
        if ((Test-Path -LiteralPath $candidate) -and ((Get-Item -LiteralPath $candidate -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Reparse installation path rejected.' }
        $candidate = Split-Path -Parent $candidate
    }
}
function New-PackageAcl([bool]$Readable) {
    $acl = [Security.AccessControl.DirectorySecurity]::new()
    $acl.SetAccessRuleProtection($true, $false)
    $acl.SetOwner([Security.Principal.SecurityIdentifier]::new('S-1-5-32-544'))
    $rules = @(@('S-1-5-18','FullControl'), @('S-1-5-32-544','FullControl'))
    if ($Readable) { $rules += ,@('S-1-5-32-545','ReadAndExecute') }
    foreach ($rule in $rules) {
        $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new([Security.Principal.SecurityIdentifier]::new($rule[0]),$rule[1],'ContainerInherit,ObjectInherit','None','Allow'))
    }
    return $acl
}
function Assert-Verifier([IO.Stream]$Stream) {
    $hash = [Security.Cryptography.SHA256]::Create()
    try { $actual = [Convert]::ToHexString($hash.ComputeHash($Stream)).ToLowerInvariant() } finally { $hash.Dispose() }
    $Stream.Position = 0
    if ($actual -ne $ExpectedVerifierSha256.ToLowerInvariant()) { throw 'Verifier differs from the independently selected build digest; no executable was run.' }
}

. (Join-Path $PSScriptRoot 'Install-Transaction.ps1')
$installationRoot = Resolve-InstallationRoot $InstallationRoot
$transactionRoot = Join-Path $installationRoot '.setup-transaction'
$source = (Resolve-Path -LiteralPath $PackagePath).Path
if ($Action -in @('Commit','Rollback')) {
    if (!(Test-Path -LiteralPath $transactionRoot)) { return }
    Assert-AdministratorTree $transactionRoot
    $state = Get-Content -LiteralPath (Join-Path $transactionRoot 'state.json') -Raw | ConvertFrom-Json
    if ($state.schema -ne 1 -or $state.id -notmatch '^[a-f0-9]{32}$' -or $state.root -ine $installationRoot) { throw 'Installation recovery identity rejected.' }
    if ($Action -eq 'Rollback') { Restore-InstallTransaction }
    else {
        if (!$state.prepared) { throw 'An unfinished transaction cannot be committed.' }
        Complete-InstallTransaction
    }
    return
}
if ($Uninstall) {
    Assert-AchimeldStopped
    $target = Join-Path $installationRoot 'Application'
    if (!(Test-Path -LiteralPath $target)) { return }
    Assert-AdministratorTree $target
    $lock = [IO.File]::Open((Join-Path $target 'Achimeld.exe'), 'Open', 'Read', 'Read')
    try { Assert-Verifier $lock; Invoke-PackageCommand $target '--remove-policy' }
    finally { $lock.Dispose() }
    Remove-TransactionTree $target $installationRoot
    return
}
if (Test-Path -LiteralPath $transactionRoot) { throw 'An unfinished installation needs recovery. Run this installer with Action Rollback before retrying; recovery files were retained.' }
$manifest = Get-Content -LiteralPath (Join-Path $source 'manifest.json') -Raw | ConvertFrom-Json
if ($manifest.mode -cne 'development' -or $manifest.product -cne 'Achimeld' -or $manifest.version -notmatch '^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$' -or $manifest.keyId -notmatch '^[a-f0-9]{64}$') { throw 'This script accepts locally built development packages only.' }
Start-InstallTransaction
if ($Action -eq 'Install') { Complete-InstallTransaction }
