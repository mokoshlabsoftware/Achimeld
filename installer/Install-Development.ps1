[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$PackagePath,
    [Parameter(Mandatory)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$ExpectedVerifierSha256,
    [switch]$Uninstall
)
$ErrorActionPreference = 'Stop'
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
if (!$principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'Run this installation script with Windows administrator approval. The application itself must run without elevation.' }
$source = (Resolve-Path -LiteralPath $PackagePath).Path
$manifest = Get-Content -LiteralPath (Join-Path $source 'manifest.json') -Raw | ConvertFrom-Json
if ($manifest.mode -ne 'development' -or $manifest.product -ne 'Achimeld' -or $manifest.version -notmatch '^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$' -or $manifest.keyId -notmatch '^[a-f0-9]{64}$') { throw 'This script accepts locally built development packages only.' }
$installationRoot = Join-Path ([Environment]::GetFolderPath([Environment+SpecialFolder]::ProgramFiles)) 'Achimeld-Development'
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
Assert-NoReparse $installationRoot
$target = [IO.Path]::GetFullPath((Join-Path $installationRoot ($manifest.version + '-' + $manifest.keyId.Substring(0,16))))
$boundary = [IO.Path]::GetFullPath($installationRoot) + [IO.Path]::DirectorySeparatorChar
if (!$target.StartsWith($boundary, [StringComparison]::OrdinalIgnoreCase)) { throw 'Installation boundary rejected.' }
if ($Uninstall) {
    if (!(Test-Path -LiteralPath $target)) { return }
    Assert-NoReparse $target
    $lock = [IO.File]::Open((Join-Path $target 'Achimeld.exe'), 'Open', 'Read', 'Read')
    try {
        Assert-Verifier $lock
        & (Join-Path $target 'Achimeld.exe') --remove-policy
        if ($LASTEXITCODE -ne 0) { throw 'Network policy removal failed; installation retained.' }
    } finally { $lock.Dispose() }
    Remove-Item -LiteralPath $target -Recurse -Force
    return
}
if (Test-Path -LiteralPath $target) { throw 'This development version is already installed. Uninstall it explicitly before replacing it.' }
New-Item -ItemType Directory -Force -Path $installationRoot | Out-Null
$readableAcl = New-PackageAcl $true
Set-Acl -LiteralPath $installationRoot -AclObject $readableAcl
Assert-NoReparse $installationRoot
$stage = Join-Path $installationRoot ('.staging-' + [Guid]::NewGuid().ToString('N'))
$verifierLock = $null
try {
    # Copy only pinned verifier bytes into administrator-only staging. Never run
    # an executable through a caller-writable source path, even while its file is locked.
    New-Item -ItemType Directory -Path $stage | Out-Null
    Set-Acl -LiteralPath $stage -AclObject (New-PackageAcl $false)
    $input = [IO.File]::Open((Join-Path $source 'Achimeld.exe'), 'Open', 'Read', 'Read')
    try {
        Assert-Verifier $input
        $output = [IO.File]::Open((Join-Path $stage 'Achimeld.exe'), 'CreateNew', 'Write', 'None')
        try { $input.CopyTo($output) } finally { $output.Dispose() }
    } finally { $input.Dispose() }
    $verifier = Join-Path $stage 'Achimeld.exe'
    $verifierLock = [IO.File]::Open($verifier, 'Open', 'Read', 'Read')
    Assert-Verifier $verifierLock
    if (!$manifest.files.Count -or $manifest.files.Count -gt 20000) { throw 'Package inventory count rejected.' }
    # This inventory is still untrusted. Confine every destination and create
    # regular files from streams; do not recursively copy source reparse points.
    foreach ($file in @($manifest.files) + @(@{path='manifest.json'}, @{path='manifest.sig'})) {
        if ($file.path -ceq 'Achimeld.exe') { continue }
        if (!$file.path -or $file.path.Contains('\') -or $file.path.Contains(':') -or [IO.Path]::IsPathRooted($file.path) -or ($file.path -split '/') -contains '..') { throw 'Unsafe staging inventory path.' }
        $destination = [IO.Path]::GetFullPath((Join-Path $stage $file.path))
        if (!$destination.StartsWith($stage + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'Staging copy boundary rejected.' }
        New-Item -ItemType Directory -Path (Split-Path -Parent $destination) -Force | Out-Null
        $input = [IO.File]::Open((Join-Path $source $file.path), 'Open', 'Read', 'Read')
        try {
            if ($input.Length -gt 2GB) { throw 'Staging file exceeds inventory limit.' }
            $output = [IO.File]::Open($destination, 'CreateNew', 'Write', 'None')
            try { $input.CopyTo($output) } finally { $output.Dispose() }
        } finally { $input.Dispose() }
    }
    & $verifier --verify-package $stage
    if ($LASTEXITCODE -ne 0) { throw 'Protected staged package verification failed.' }
    $authenticated = Get-Content -LiteralPath (Join-Path $stage 'manifest.json') -Raw | ConvertFrom-Json
    if ($authenticated.version -cne $manifest.version -or $authenticated.keyId -cne $manifest.keyId -or $authenticated.mode -cne 'development') { throw 'Authenticated package identity differs from the selected installation directory.' }
    $verifierLock.Dispose(); $verifierLock = $null
    # Only an authenticated complete package becomes readable by normal users.
    Set-Acl -LiteralPath $stage -AclObject $readableAcl
    foreach ($item in Get-ChildItem -LiteralPath $stage -Force -Recurse) {
        $itemAcl = Get-Acl -LiteralPath $item.FullName
        $itemAcl.SetOwner([Security.Principal.SecurityIdentifier]::new('S-1-5-32-544'))
        $itemAcl.SetAccessRuleProtection($false, $false)
        Set-Acl -LiteralPath $item.FullName -AclObject $itemAcl
    }
    Move-Item -LiteralPath $stage -Destination $target
    & (Join-Path $target 'Achimeld.exe') --install-policy
    if ($LASTEXITCODE -ne 0) { throw 'Installation retained for repair: Windows policy installation failed.' }
    Write-Output "Installed development package: $target"
    Write-Output 'Start Achimeld.exe as a normal user. This development package has no official publisher identity.'
} finally {
    if ($verifierLock) { $verifierLock.Dispose() }
    if (Test-Path -LiteralPath $stage) {
        $resolvedStage = [IO.Path]::GetFullPath($stage)
        if (!$resolvedStage.StartsWith($boundary, [StringComparison]::OrdinalIgnoreCase)) { throw 'Staging cleanup boundary rejected.' }
        Assert-NoReparse $resolvedStage
        Remove-Item -LiteralPath $resolvedStage -Recurse -Force
    }
}
