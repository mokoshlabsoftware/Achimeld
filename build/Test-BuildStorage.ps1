[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
& (Join-Path $PSScriptRoot 'Test-BuildAndRun.ps1')
. (Join-Path $PSScriptRoot 'BuildPaths.ps1')
$repository = Split-Path -Parent $PSScriptRoot
$first = Get-AchimeldBuildPaths -Date ([datetime]::new(2026,12,31))
$next = Get-AchimeldBuildPaths -Date ([datetime]::new(2027,1,1))
if ((Split-Path -Leaf $first.Archive) -cne '31_12_2026' -or (Split-Path -Leaf $next.Archive) -cne '01_01_2027' -or
    $first.Artifacts -cne $next.Artifacts -or $first.Cache -cne $next.Cache) { throw 'Date rollover or stable current/cache layout failed.' }
foreach ($invalid in @($repository, (Join-Path $repository 'nested-build'))) {
    $rejected = $false
    try { $null = Get-AchimeldBuildPaths -BuildRoot $invalid } catch { $rejected = $true }
    if (!$rejected) { throw 'In-repository build storage was accepted.' }
}
Initialize-AchimeldBuildPaths
foreach ($project in Get-ChildItem -LiteralPath @((Join-Path $repository 'bridge'),(Join-Path $repository 'tools')) -Filter '*.csproj' -File -Recurse) {
    $result = & dotnet msbuild $project.FullName '-getProperty:BaseOutputPath,BaseIntermediateOutputPath,RestorePackagesPath' -nologo
    if ($LASTEXITCODE -ne 0) { throw 'Cannot inspect managed output paths.' }
    $properties = ($result -join "`n" | ConvertFrom-Json).Properties
    foreach ($property in $properties.PSObject.Properties) {
        $path = [IO.Path]::GetFullPath($property.Value)
        if (!$path.StartsWith($env:ACHIMELD_BUILD_ROOT + '\', [StringComparison]::OrdinalIgnoreCase)) {
            throw "Managed output escapes build storage: $($project.Name) $($property.Name) $path"
        }
    }
}
# Scan ignored files too: ignoring binaries does not prevent local build leakage.
$files = [Collections.Generic.List[string]]::new()
$directories = [Collections.Generic.Stack[string]]::new()
$directories.Push($repository)
while ($directories.Count) {
    $directory = $directories.Pop()
    foreach ($item in Get-ChildItem -LiteralPath $directory -Force) {
        if ($directory -eq $repository -and $item.Name -eq '.git') { continue }
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Cannot scan reparse source entry: $($item.FullName)" }
        if ($item.PSIsContainer) { $directories.Push($item.FullName) }
        else { $files.Add($item.FullName) }
    }
}
$binaries = @($files | Where-Object { [IO.Path]::GetExtension($_) -in @('.exe','.dll','.pdb','.lib','.obj','.rlib','.rmeta','.nupkg','.so','.dylib','.a','.o','.wasm') })
if ($binaries.Count) { throw ('Compiled binaries remain in source: ' + ($binaries -join ', ')) }
Write-Output 'External Rust/.NET storage, stable current paths, date rollover and source binary scan passed.'
& (Join-Path $PSScriptRoot 'Test-DevelopmentProcesses.ps1')
