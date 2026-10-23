[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$tokens=$null; $errors=$null
$ast = [Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot 'Audit.ps1'), [ref]$tokens, [ref]$errors)
if ($errors.Count) { throw 'Audit script syntax failed.' }
$definition = $ast.Find({param($node) $node -is [Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq 'Run-Check'}, $true)
. ([scriptblock]::Create($definition.Extent.Text))
$output = Join-Path ([IO.Path]::GetTempPath()) ('achimeld-audit-failure-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $output | Out-Null
$checks = [Collections.Generic.List[object]]::new()
$source = @{commit='fixture';dirty=$false}
$started = [DateTimeOffset]::UtcNow.ToString('O')
$FailFast = $true
try {
    $stopped=$false
    try { Run-Check 'format' 'source' @() { throw 'Deliberate formatting failure fixture.' } }
    catch { $stopped = $_.Exception.Message -match "Check 'format' failed" }
    if (!$stopped) { throw 'Failed preflight did not stop execution.' }
    $report = Get-Content -LiteralPath (Join-Path $output 'evidence.json') -Raw | ConvertFrom-Json
    if ($report.selectedChecksPassed -or $report.checks[0].status -ne 'test-failed') { throw 'Failure evidence was not retained.' }
    if (@($report.checks | Where-Object status -eq 'not-run').Count -ne 18) { throw 'Unexecuted checks were not recorded.' }
    if (@($report.checks | Where-Object id -eq 'rust-tests')[0].status -ne 'not-run') { throw 'Compilation was not marked unexecuted.' }
    Write-Output 'Early audit failure stops execution and retains failure and skipped-check evidence.'
} finally {
    Get-ChildItem -LiteralPath $output -File | ForEach-Object { Remove-Item -LiteralPath $_.FullName }
    Remove-Item -LiteralPath $output
}
