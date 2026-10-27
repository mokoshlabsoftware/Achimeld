[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$repositoryRoot = Split-Path -Parent $PSScriptRoot
$tokens = $null; $parseErrors = $null
$ast = [Management.Automation.Language.Parser]::ParseFile((Join-Path $repositoryRoot 'installer/Install-Development.ps1'), [ref]$tokens, [ref]$parseErrors)
if ($parseErrors.Count) { throw 'Installer syntax rejected.' }
# Exercise the exact pure helpers without invoking elevation, filesystem
# installation, WFP changes, or any package executable.
foreach ($name in @('Assert-Verifier','New-PackageAcl')) {
    $function = $ast.Find({ param($node) $node -is [Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq $name }, $true)
    if (!$function) { throw "Installer helper absent: $name" }
    . ([scriptblock]::Create($function.Extent.Text))
}
$bytes = [Text.Encoding]::UTF8.GetBytes('sanitized verifier identity test')
$ExpectedVerifierSha256 = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($bytes)).ToLowerInvariant()
$stream = [IO.MemoryStream]::new($bytes)
try { Assert-Verifier $stream; if ($stream.Position -ne 0) { throw 'Pinned verifier stream not reset.' } } finally { $stream.Dispose() }
$ExpectedVerifierSha256 = '0' * 64
$stream = [IO.MemoryStream]::new($bytes)
$rejected = $false
try { Assert-Verifier $stream } catch { $rejected = $_.Exception.Message.StartsWith('Verifier differs') } finally { $stream.Dispose() }
if (!$rejected) { throw 'Mismatched verifier was not rejected.' }
foreach ($readable in @($false, $true)) {
    $acl = New-PackageAcl $readable
    if (!$acl.AreAccessRulesProtected) { throw 'Staging ACL inherits caller permissions.' }
    $users = @($acl.GetAccessRules($true,$false,[Security.Principal.SecurityIdentifier]) | Where-Object { $_.IdentityReference.Value -eq 'S-1-5-32-545' })
    if ([bool]$users.Count -ne $readable) { throw 'Unverified staging is readable by normal users.' }
}
Write-Output 'Installer verifier pin, stream reset and administrator-only staging ACL checks passed. No installation or policy change performed.'
& (Join-Path $PSScriptRoot 'Test-SetupLogs.ps1')
