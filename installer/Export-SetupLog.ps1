[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$LogPath,
    [Parameter(Mandatory)][ValidatePattern('^setup-[0-9]{8}-[0-9]{6}-[0-9]+$')][string]$SessionName
)
$ErrorActionPreference = 'Stop'

# Setup diagnostics have no application or policy authority. Never write an
# elevated log into a user-writable directory, or follow a redirected entry.
function Assert-SetupLogEntry([string]$Path) {
    $item = Get-Item -LiteralPath $Path -Force
    if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Redirected setup log path rejected.' }
    $acl = Get-Acl -LiteralPath $Path
    $trusted = @('S-1-5-18','S-1-5-32-544')
    if ($acl.GetOwner([Security.Principal.SecurityIdentifier]).Value -notin $trusted) { throw 'Setup log entry is not administrator owned.' }
    $write = [Security.AccessControl.FileSystemRights]::Write -bor [Security.AccessControl.FileSystemRights]::Delete -bor
        [Security.AccessControl.FileSystemRights]::DeleteSubdirectoriesAndFiles -bor
        [Security.AccessControl.FileSystemRights]::ChangePermissions -bor [Security.AccessControl.FileSystemRights]::TakeOwnership
    foreach ($rule in $acl.GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier])) {
        if ($rule.AccessControlType -eq 'Allow' -and $rule.IdentityReference.Value -notin $trusted -and ($rule.FileSystemRights -band $write)) {
            throw 'Setup log entry is writable by an unprivileged account.'
        }
    }
}

function New-SetupLogAcl {
    $acl = [Security.AccessControl.DirectorySecurity]::new()
    $acl.SetSecurityDescriptorSddlForm('O:BAG:BAD:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;GRGX;;;BU)')
    return $acl
}

function Initialize-SetupLogDirectory([string]$Path) {
    if (!(Test-Path -LiteralPath $Path)) {
        # Create with the ACL atomically. A pre-existing/raced directory must
        # pass validation; never take ownership of it or repair its permissions.
        [IO.FileSystemAclExtensions]::Create([IO.DirectoryInfo]::new($Path), (New-SetupLogAcl))
    }
    Assert-SetupLogEntry $Path
}

function Assert-SetupLogParent([string]$Path) {
    $trusted = @('S-1-5-18','S-1-5-32-544','S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464')
    $replace = [Security.AccessControl.FileSystemRights]::DeleteSubdirectoriesAndFiles -bor [Security.AccessControl.FileSystemRights]::Delete -bor
        [Security.AccessControl.FileSystemRights]::ChangePermissions -bor [Security.AccessControl.FileSystemRights]::TakeOwnership
    while ($Path) {
        if ((Get-Item -LiteralPath $Path -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Redirected shared data folder rejected.' }
        $acl = Get-Acl -LiteralPath $Path
        if ($acl.GetOwner([Security.Principal.SecurityIdentifier]).Value -notin $trusted) { throw 'Shared data parent is not administrator owned.' }
        foreach ($rule in $acl.GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier])) {
            if ($rule.PropagationFlags -band [Security.AccessControl.PropagationFlags]::InheritOnly) { continue }
            if ($rule.AccessControlType -eq 'Allow' -and $rule.IdentityReference.Value -notin $trusted -and ($rule.FileSystemRights -band $replace)) {
                throw 'Shared data parent permits unprivileged directory replacement.'
            }
        }
        $Path = Split-Path -Parent $Path
    }
}

function Open-SetupLogOutput([string]$Path) {
    $acl = [Security.AccessControl.FileSecurity]::new()
    $acl.SetSecurityDescriptorSddlForm('O:BAG:BAD:P(A;;FA;;;SY)(A;;FA;;;BA)(A;;GR;;;BU)')
    # Apply the ACL at creation. A FileAccess.Write handle alone cannot change
    # ownership/permissions, even when its process is elevated.
    return [IO.FileSystemAclExtensions]::Create([IO.FileInfo]::new($Path),
        [IO.FileMode]::OpenOrCreate, [Security.AccessControl.FileSystemRights]::FullControl,
        [IO.FileShare]::Read, 65536, [IO.FileOptions]::None, $acl)
}

function Export-SetupLog {
    $common = [Environment]::GetFolderPath('CommonApplicationData')
    Assert-SetupLogParent $common
    $product = Join-Path $common 'Achimeld'
    $directory = Join-Path $product 'SetupLogs'
    Initialize-SetupLogDirectory $product
    Initialize-SetupLogDirectory $directory
    $destination = Join-Path $directory ($SessionName + '.log')
    if (Test-Path -LiteralPath $destination) { Assert-SetupLogEntry $destination }
    $inputLog = [IO.File]::Open($LogPath, 'Open', 'Read', 'ReadWrite')
    try {
        $outputLog = Open-SetupLogOutput $destination
        try {
            $outputLog.SetLength(0)
            # Retain the useful tail, bounded even if a subprocess is noisy.
            $limit = 16MB
            if ($inputLog.Length -gt $limit) { $inputLog.Position = $inputLog.Length - $limit }
            $buffer = [byte[]]::new(65536)
            $remaining = $limit
            while ($remaining -gt 0 -and ($read = $inputLog.Read($buffer, 0, [Math]::Min($buffer.Length,$remaining))) -gt 0) {
                $outputLog.Write($buffer, 0, $read)
                $remaining -= $read
            }
        } finally { $outputLog.Dispose() }
    } finally { $inputLog.Dispose() }
    # Only our narrowly named, validated regular files may be pruned.
    $old = @(Get-ChildItem -LiteralPath $directory -File -Force | Where-Object {
        $_.Name -cmatch '^setup-[0-9]{8}-[0-9]{6}-[0-9]+\.log$' -and $_.FullName -ine $destination
    } | Sort-Object LastWriteTimeUtc -Descending | Select-Object -Skip 4)
    foreach ($entry in $old) {
        Assert-SetupLogEntry $entry.FullName
        Remove-Item -LiteralPath $entry.FullName -Force
    }
}

try { Export-SetupLog }
catch { Write-Error $_ -ErrorAction Continue; exit 1 }
