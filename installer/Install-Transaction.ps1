# Dot-sourced by the pinned installer entry point, never by an application worker.
function Write-InstallProgress([string]$Stage, [int]$Position = 0, [int]$Total = 0, [string]$Detail = '') {
    $safeStage = $Stage.Replace('|', ' ').Replace("`r", ' ').Replace("`n", ' ')
    $safeDetail = $Detail.Replace('|', ' ').Replace("`r", ' ').Replace("`n", ' ')
    if ($safeDetail.Length -gt 500) { $safeDetail = $safeDetail.Substring(0, 500) }
    Write-Output "ACHIMELD|$safeStage|$Position|$Total|$safeDetail"
    [Console]::Out.Flush()
}

function Assert-NotCancelled {
    if ($CancelFile -and (Test-Path -LiteralPath $CancelFile)) { throw 'Installation cancelled.' }
}

function Get-AchimeldProcessIds {
    foreach ($name in @('Achimeld','Achimeld.App','achimeld-app','achimeld-bootstrapper')) {
        foreach ($process in [Diagnostics.Process]::GetProcessesByName($name)) {
            try { $process.Id } finally { $process.Dispose() }
        }
    }
}

function Assert-AchimeldStopped {
    if (@(Get-AchimeldProcessIds).Count) { throw 'Achimeld is running. Close it before installing, updating, repairing or uninstalling.' }
}

function Get-RegisteredInstallRoot {
    $key = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Achimeld-Development_is1'
    $value = Get-ItemProperty -LiteralPath $key -ErrorAction SilentlyContinue
    if ($value -and $value.AchimeldInstallRoot) { return [string]$value.AchimeldInstallRoot }
    return Join-Path ([Environment]::GetFolderPath('ProgramFiles')) 'Mokosh Labs\Achimeld'
}

function Resolve-InstallationRoot([string]$Path) {
    if ($Path -notmatch '^[A-Za-z]:\\' -or $Path -match '["<>|*?\x00-\x1f]' -or $Path.Substring(2).Contains(':')) {
        throw 'Choose a dedicated folder on a local Windows drive.'
    }
    $resolved = [IO.Path]::GetFullPath($Path).TrimEnd('\')
    if ($resolved -eq [IO.Path]::GetPathRoot($resolved).TrimEnd('\')) { throw 'A drive root cannot be an application folder.' }
    foreach ($special in @('Windows', 'System', 'ProgramFiles', 'ProgramFilesX86', 'CommonApplicationData', 'UserProfile', 'Desktop', 'MyDocuments')) {
        $reserved = [Environment]::GetFolderPath($special)
        if (!$reserved) { continue }
        if ($resolved.Equals($reserved.TrimEnd('\'), [StringComparison]::OrdinalIgnoreCase)) { throw 'Choose a dedicated Achimeld folder, not a shared Windows or personal folder.' }
        if ($special -in @('Windows','System') -and $resolved.StartsWith($reserved.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Windows system folders cannot contain the application.' }
    }
    Assert-NoReparse $resolved
    return $resolved
}

function Assert-AdministratorTree([string]$Path) {
    Assert-NoReparse $Path
    foreach ($entry in @((Get-Item -LiteralPath $Path -Force)) + @(Get-ChildItem -LiteralPath $Path -Recurse -Force)) {
        if ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Redirected installation entry rejected.' }
        $acl = Get-Acl -LiteralPath $entry.FullName
        if ($acl.GetOwner([Security.Principal.SecurityIdentifier]).Value -notin @('S-1-5-18','S-1-5-32-544')) { throw 'Installation entry is not administrator owned.' }
        foreach ($rule in $acl.GetAccessRules($true, $true, [Security.Principal.SecurityIdentifier])) {
            $unsafe = [Security.AccessControl.FileSystemRights]::Write -bor [Security.AccessControl.FileSystemRights]::Delete -bor
                [Security.AccessControl.FileSystemRights]::ChangePermissions -bor [Security.AccessControl.FileSystemRights]::TakeOwnership
            if ($rule.AccessControlType -eq 'Allow' -and $rule.IdentityReference.Value -notin @('S-1-5-18','S-1-5-32-544') -and ($rule.FileSystemRights -band $unsafe)) {
                throw 'Installation entry has unsafe write permissions.'
            }
        }
    }
}

function Assert-InstallationParent([string]$Path) {
    $parent = Split-Path -Parent $Path
    if (!(Test-Path -LiteralPath $parent -PathType Container)) { throw 'Choose an existing parent folder for the dedicated Achimeld folder.' }
    $trusted = @('S-1-5-18','S-1-5-32-544','S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464')
    while ($parent) {
        Assert-NoReparse $parent
        $acl = Get-Acl -LiteralPath $parent
        if ($acl.GetOwner([Security.Principal.SecurityIdentifier]).Value -notin $trusted) { throw 'The parent folder must be administrator owned so another user cannot replace Achimeld. Choose a protected parent such as Program Files.' }
        foreach ($rule in $acl.GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier])) {
            if ($rule.PropagationFlags -band [Security.AccessControl.PropagationFlags]::InheritOnly) { continue }
            $replace = [Security.AccessControl.FileSystemRights]::DeleteSubdirectoriesAndFiles -bor [Security.AccessControl.FileSystemRights]::Delete -bor
                [Security.AccessControl.FileSystemRights]::ChangePermissions -bor [Security.AccessControl.FileSystemRights]::TakeOwnership
            if ($rule.AccessControlType -eq 'Allow' -and $rule.IdentityReference.Value -notin $trusted -and ($rule.FileSystemRights -band $replace)) {
                throw 'The parent folder allows an unprivileged user to replace application directories. Choose a protected parent.'
            }
        }
        $parent = Split-Path -Parent $parent
    }
}

function Invoke-PackageCommand([string]$Directory, [string]$Command) {
    & (Join-Path $Directory 'Achimeld.exe') $Command | Out-Host
    if ($LASTEXITCODE -ne 0) { throw "Package operation failed: $Command. Close Achimeld and retry; recovery files are retained if needed." }
}

function Save-InstallState {
    $pending = Join-Path $transactionRoot 'state.new'
    $state | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $pending -Encoding utf8
    Move-Item -LiteralPath $pending -Destination (Join-Path $transactionRoot 'state.json') -Force
}

function Assert-TransactionEntry($Entry) {
    $original = [IO.Path]::GetFullPath([string]$Entry.original)
    $parent = Split-Path -Parent $original
    $leaf = Split-Path -Leaf $original
    $roots = @($installationRoot, $state.previousRoot,
        (Join-Path ([Environment]::GetFolderPath('ProgramFiles')) 'Mokosh Labs\Achimeld'),
        (Join-Path ([Environment]::GetFolderPath('ProgramFiles')) 'Achimeld-Development')) | Where-Object { $_ }
    if ($parent -notin $roots -or ($leaf -ne 'Application' -and $leaf -notmatch '^\d+\.\d+\.\d+-[a-f0-9]{16}$')) { throw 'Recovery source is outside the installation boundary.' }
    if ([IO.Path]::GetFullPath([string]$Entry.backup) -cne (Join-Path $parent ('.backup-' + $state.id + '-' + $leaf))) { throw 'Recovery backup boundary rejected.' }
    Assert-NoReparse $original
    Assert-NoReparse $Entry.backup
}

function Remove-TransactionTree([string]$Path, [string]$Parent) {
    $resolved = [IO.Path]::GetFullPath($Path)
    if (!( $resolved.StartsWith([IO.Path]::GetFullPath($Parent).TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase))) { throw 'Transaction cleanup boundary rejected.' }
    if (Test-Path -LiteralPath $resolved) {
        Assert-AdministratorTree $resolved
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}

function Restore-InstallTransaction {
    if ($state.committed) { throw 'Installation has already committed. Retained files are cleanup recovery only; rollback is no longer permitted.' }
    Write-InstallProgress 'Restoring the previous installation' 0 0 'Keep Setup open until recovery finishes.'
    $target = Join-Path $installationRoot 'Application'
    if ($state.newInstalled) {
        Assert-AdministratorTree $target
        $repair = @($state.entries | Where-Object { $_.repairExact -and $_.original -ieq $target })
        if ($repair.Count -and $state.newPolicyTouched) {
            # Its authenticated manifest is identical: restore policy at its original
            # path before putting the original (possibly damaged) bytes back.
            Invoke-PackageCommand $target '--install-policy'
        } elseif ($state.newPolicyTouched) {
            Invoke-PackageCommand $target '--remove-policy'
        }
        Remove-TransactionTree $target $installationRoot
        $state.newInstalled = $false
        Save-InstallState
    }
    foreach ($entry in $state.entries) {
        Assert-TransactionEntry $entry
        if (Test-Path -LiteralPath $entry.backup) {
            if (Test-Path -LiteralPath $entry.original) { throw 'Recovery destination is occupied; backup retained.' }
            Assert-AdministratorTree $entry.backup
            Write-InstallProgress 'Restoring files' 0 0 $entry.original
            Move-Item -LiteralPath $entry.backup -Destination $entry.original
        }
        if ($entry.policyRemoved) {
            Invoke-PackageCommand $entry.original '--install-policy'
            $entry.policyRemoved = $false
            Save-InstallState
        }
    }
    Remove-TransactionTree $transactionRoot $installationRoot
    if (!$state.rootExisted -and (Test-Path -LiteralPath $installationRoot) -and !(Get-ChildItem -LiteralPath $installationRoot -Force)) {
        Remove-Item -LiteralPath $installationRoot
    } elseif ($state.rootAcl -and (Test-Path -LiteralPath $installationRoot)) {
        $acl = [Security.AccessControl.DirectorySecurity]::new()
        $acl.SetSecurityDescriptorSddlForm($state.rootAcl)
        Set-Acl -LiteralPath $installationRoot -AclObject $acl
    }
    if ($state.createdParent -and (Test-Path -LiteralPath $state.createdParent) -and !(Get-ChildItem -LiteralPath $state.createdParent -Force)) {
        Remove-Item -LiteralPath $state.createdParent
    }
    Write-InstallProgress 'Previous state restored' 1 1 'Installation cancelled or failed; no replacement was committed.'
}

function Complete-InstallTransaction {
    # Persist the irreversible boundary before deleting the first rollback copy.
    # A cleanup retry may finish commit, but must never restore a partial backup.
    $state.committed = $true
    Save-InstallState
    foreach ($entry in $state.entries) {
        Assert-TransactionEntry $entry
        Write-InstallProgress 'Removing the previous application files' 0 0 $entry.original
        Remove-TransactionTree $entry.backup (Split-Path -Parent $entry.backup)
        $leaf = Split-Path -Leaf $entry.original
        if ($leaf -ne 'Application') {
            $key = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Achimeld-Development-' + $leaf + '_is1'
            if (Test-Path -LiteralPath $key) { Remove-Item -LiteralPath $key -Recurse -Force }
            Remove-TransactionTree (Join-Path (Split-Path -Parent $entry.original) ('uninstall-' + $leaf)) (Split-Path -Parent $entry.original)
        }
    }
    if ($state.previousRoot -and $state.previousRoot -ine $installationRoot) {
        Remove-TransactionTree (Join-Path $state.previousRoot 'Uninstall') $state.previousRoot
        if ((Test-Path -LiteralPath $state.previousRoot) -and !(Get-ChildItem -LiteralPath $state.previousRoot -Force)) {
            Remove-Item -LiteralPath $state.previousRoot
        }
    }
    Remove-TransactionTree $transactionRoot $installationRoot
    Write-InstallProgress 'Installation complete' 1 1 (Join-Path $installationRoot 'Application')
}

function New-VerifiedInstallStage([string]$Stage) {
    New-Item -ItemType Directory -Path $Stage | Out-Null
    Set-Acl -LiteralPath $Stage -AclObject (New-PackageAcl $false)
    $input = [IO.File]::Open((Join-Path $source 'Achimeld.exe'), 'Open', 'Read', 'Read')
    try {
        Assert-Verifier $input
        $output = [IO.File]::Open((Join-Path $Stage 'Achimeld.exe'), 'CreateNew', 'Write', 'None')
        try { $input.CopyTo($output) } finally { $output.Dispose() }
    } finally { $input.Dispose() }
    $verifier = Join-Path $Stage 'Achimeld.exe'
    $lock = [IO.File]::Open($verifier, 'Open', 'Read', 'Read')
    try {
        Assert-Verifier $lock
        if (!$manifest.files.Count -or $manifest.files.Count -gt 20000) { throw 'Package inventory count rejected.' }
        $files = @($manifest.files) + @(@{path='manifest.json'}, @{path='manifest.sig'})
        $position = 0
        foreach ($file in $files) {
            Assert-NotCancelled
            $position++
            if ($file.path -ceq 'Achimeld.exe') { continue }
            if (!$file.path -or $file.path.Contains('\') -or $file.path.Contains(':') -or [IO.Path]::IsPathRooted($file.path) -or ($file.path -split '/') -contains '..') { throw 'Unsafe staging inventory path.' }
            $destination = [IO.Path]::GetFullPath((Join-Path $Stage $file.path))
            if (!$destination.StartsWith($Stage + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Staging copy boundary rejected.' }
            Write-InstallProgress 'Copying replacement files' $position $files.Count $file.path
            New-Item -ItemType Directory -Path (Split-Path -Parent $destination) -Force | Out-Null
            $input = [IO.File]::Open((Join-Path $source $file.path), 'Open', 'Read', 'Read')
            try {
                if ($input.Length -gt 2GB) { throw 'Staging file exceeds inventory limit.' }
                $output = [IO.File]::Open($destination, 'CreateNew', 'Write', 'None')
                try {
                    $buffer = [byte[]]::new(1MB)
                    while (($read = $input.Read($buffer, 0, $buffer.Length)) -gt 0) {
                        Assert-NotCancelled
                        $output.Write($buffer, 0, $read)
                    }
                } finally { $output.Dispose() }
            } finally { $input.Dispose() }
        }
        Write-InstallProgress 'Verifying the complete replacement' 0 0 'Checking package signatures and every file.'
        Invoke-PackageCommand $Stage '--verify'
        $authenticated = Get-Content -LiteralPath (Join-Path $Stage 'manifest.json') -Raw | ConvertFrom-Json
        if ($authenticated.version -cne $manifest.version -or $authenticated.keyId -cne $manifest.keyId -or $authenticated.mode -cne 'development') { throw 'Authenticated package identity changed.' }
    } finally { $lock.Dispose() }
    Set-Acl -LiteralPath $Stage -AclObject (New-PackageAcl $true)
    $items = @(Get-ChildItem -LiteralPath $Stage -Recurse -Force)
    $position = 0
    foreach ($item in $items) {
        Assert-NotCancelled
        $position++
        Write-InstallProgress 'Protecting application files' $position $items.Count $item.FullName.Substring($Stage.Length + 1)
        $acl = Get-Acl -LiteralPath $item.FullName
        $acl.SetOwner([Security.Principal.SecurityIdentifier]::new('S-1-5-32-544'))
        $acl.SetAccessRuleProtection($false, $false)
        Set-Acl -LiteralPath $item.FullName -AclObject $acl
    }
}

function Start-InstallTransaction {
    Assert-AchimeldStopped
    $registered = Resolve-InstallationRoot (Get-RegisteredInstallRoot)
    if ($PreviousInstallationRoot -and (Resolve-InstallationRoot $PreviousInstallationRoot) -ine $registered) { throw 'Previous installation differs from administrator-owned registration.' }
    foreach ($otherRoot in @($registered, (Join-Path ([Environment]::GetFolderPath('ProgramFiles')) 'Mokosh Labs\Achimeld'), (Join-Path ([Environment]::GetFolderPath('ProgramFiles')) 'Achimeld-Development'))) {
        if ($installationRoot -ine $otherRoot -and
            ($installationRoot.StartsWith($otherRoot + '\', [StringComparison]::OrdinalIgnoreCase) -or $otherRoot.StartsWith($installationRoot + '\', [StringComparison]::OrdinalIgnoreCase))) {
            throw 'The new folder must not contain, or be inside, another Achimeld installation.'
        }
    }
    $createdParent = ''
    $defaultRoot = Join-Path ([Environment]::GetFolderPath('ProgramFiles')) 'Mokosh Labs\Achimeld'
    $existed = Test-Path -LiteralPath $installationRoot
    $previousAcl = ''
    if ($existed) {
        if ($installationRoot -ine $registered -and (Get-ChildItem -LiteralPath $installationRoot -Force)) { throw 'Choose an empty folder; Setup will not take ownership of unrelated files.' }
        $previousAcl = (Get-Acl -LiteralPath $installationRoot).Sddl
        if ($installationRoot -ieq $registered) { Assert-AdministratorTree $installationRoot }
    }
    $script:state = [ordered]@{schema=1;id=[guid]::NewGuid().ToString('N');root=$installationRoot;previousRoot=$registered;rootExisted=$existed;rootAcl=$previousAcl;createdParent='';entries=@();newInstalled=$false;newPolicyTouched=$false;prepared=$false;committed=$false}
    try {
    if ($installationRoot -ieq $defaultRoot -and !(Test-Path -LiteralPath (Split-Path -Parent $defaultRoot))) {
        $createdParent = Split-Path -Parent $defaultRoot
        Assert-InstallationParent $createdParent
        New-Item -ItemType Directory -Path $createdParent | Out-Null
        $state.createdParent = $createdParent
        Set-Acl -LiteralPath $createdParent -AclObject (New-PackageAcl $true)
    }
    Assert-InstallationParent $installationRoot
    if (!$existed) { New-Item -ItemType Directory -Path $installationRoot | Out-Null }
    Set-Acl -LiteralPath $installationRoot -AclObject (New-PackageAcl $true)
    New-Item -ItemType Directory -Path $transactionRoot | Out-Null
    Set-Acl -LiteralPath $transactionRoot -AclObject (New-PackageAcl $false)
    Save-InstallState
        $stage = Join-Path $transactionRoot 'payload'
        New-VerifiedInstallStage $stage
        $roots = @($installationRoot, $registered,
            (Join-Path ([Environment]::GetFolderPath('ProgramFiles')) 'Mokosh Labs\Achimeld'),
            (Join-Path ([Environment]::GetFolderPath('ProgramFiles')) 'Achimeld-Development')) | Select-Object -Unique
        foreach ($oldRoot in $roots) {
            Assert-NoReparse $oldRoot
            if (!(Test-Path -LiteralPath $oldRoot)) { continue }
            foreach ($old in Get-ChildItem -LiteralPath $oldRoot -Directory -Force) {
                if ($old.Name -ne 'Application' -and $old.Name -notmatch '^\d+\.\d+\.\d+-[a-f0-9]{16}$') { continue }
                Assert-NotCancelled
                Assert-AdministratorTree $old.FullName
                $entry = [ordered]@{original=$old.FullName;backup=(Join-Path $oldRoot ('.backup-' + $state.id + '-' + $old.Name));policyRemoved=$false;repairExact=$false}
                Assert-TransactionEntry $entry
                try { Invoke-PackageCommand $old.FullName '--verify' }
                catch {
                    # Repair corrupted files only when both signed manifest files
                    # exactly match the independently authenticated replacement.
                    $same = $old.FullName -ieq (Join-Path $installationRoot 'Application')
                    foreach ($name in @('manifest.json','manifest.sig')) {
                        $oldFile = Join-Path $old.FullName $name
                        $same = $same -and (Test-Path -LiteralPath $oldFile -PathType Leaf)
                        if ($same) { $same = (Get-FileHash -LiteralPath $oldFile -Algorithm SHA256).Hash -ceq (Get-FileHash -LiteralPath (Join-Path $stage $name) -Algorithm SHA256).Hash }
                    }
                    if (!$same) { throw 'Previous package cannot be verified and does not match the replacement. Recovery data was retained; no previous files were removed.' }
                    $entry.repairExact = $true
                }
                $state.entries += $entry
            }
        }
        # Only a fully verified prior policy can be reconstructed faithfully.
        # A damaged package is checked with its identical authenticated replacement
        # at the original path below, before touching that policy.
        foreach ($entry in $state.entries) {
            if (!$entry.repairExact) { Invoke-PackageCommand $entry.original '--check-policy' }
        }
        Save-InstallState
        foreach ($entry in $state.entries) {
            Assert-NotCancelled
            Assert-AchimeldStopped
            if (!$entry.repairExact) {
                Write-InstallProgress 'Removing the previous network policy' 0 0 $entry.original
                # Record intent before the operation, so rollback also covers a
                # process failure immediately after successful policy removal.
                $entry.policyRemoved = $true
                Save-InstallState
                Invoke-PackageCommand $entry.original '--remove-policy'
            }
            Assert-NotCancelled
            Write-InstallProgress 'Keeping previous files for rollback' 0 0 $entry.original
            Move-Item -LiteralPath $entry.original -Destination $entry.backup
        }
        Assert-NotCancelled
        Assert-AchimeldStopped
        $target = Join-Path $installationRoot 'Application'
        Write-InstallProgress 'Putting the replacement in place' 0 0 $target
        Move-Item -LiteralPath $stage -Destination $target
        $state.newInstalled = $true
        Save-InstallState
        Assert-NotCancelled
        if (@($state.entries | Where-Object { $_.repairExact }).Count) {
            Invoke-PackageCommand $target '--check-policy'
        }
        Write-InstallProgress 'Installing network policy' 0 0 $target
        $state.newPolicyTouched = $true
        Save-InstallState
        Invoke-PackageCommand $target '--install-policy'
        Assert-NotCancelled
        $state.prepared = $true
        Save-InstallState
        Write-InstallProgress 'Application prepared' 1 1 'Setup will now finish shortcuts and installation registration.'
    } catch {
        $failure = $_
        try { Restore-InstallTransaction }
        catch { throw "Rollback could not finish. Recovery files retained at $transactionRoot. $($_.Exception.Message)" }
        throw $failure
    }
}
