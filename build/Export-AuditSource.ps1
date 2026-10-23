[CmdletBinding()]
param([Parameter(Mandatory)][string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
$repositoryRoot = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'BuildPaths.ps1')
Initialize-AchimeldBuildPaths
$output = [IO.Path]::GetFullPath($OutputDirectory)
$boundary = [IO.Path]::GetFullPath(($env:ACHIMELD_ARTIFACT_ROOT)) + [IO.Path]::DirectorySeparatorChar
if (!$output.StartsWith($boundary, [StringComparison]::OrdinalIgnoreCase)) { throw 'Source archive must be inside the external current build directory.' }
if (Test-Path -LiteralPath $output) { throw 'Source archive destination already exists.' }
$paths = (& git -C $repositoryRoot ls-files --cached --others --exclude-standard -z) -split "`0"
if ($LASTEXITCODE -ne 0) { throw 'Source inventory unavailable.' }
$allowedRoots = @('crates','bridge','build','installer','tests','tools','docs')
$allowedRootFiles = @('Cargo.toml','Cargo.lock','rust-toolchain.toml','README.md','ROADMAP.md','AGENTS.md','CHANGELOG.md','.gitlab-ci.yml','.gitignore','.gitattributes','.editorconfig','build-and-run.bat','Directory.Build.props','.cargo/config.toml')
$allowedExtensions = @('.rs','.toml','.cs','.csproj','.ps1','.js','.json','.html','.md','.yml','.config','.props','.targets','.csv','.png','.ico','.rgba')
$allowedFontInputs = @(
    'crates/achimeld-ui/assets/fonts/NotoSansCJK-Regular.ttc',
    'crates/achimeld-ui/assets/fonts/NotoSansCJK-Bold.ttc',
    'crates/achimeld-ui/assets/fonts/OFL.txt'
)
$files = @($paths | Where-Object { $_ } | Sort-Object -Unique | ForEach-Object {
    $relative = $_
    $top = ($relative -split '/')[0]
    if ($relative -notin $allowedRootFiles -and $relative -notin $allowedFontInputs -and
        ($top -notin $allowedRoots -or [IO.Path]::GetExtension($relative) -notin $allowedExtensions)) { return }
    if (($relative -split '/') -contains '..' -or [IO.Path]::IsPathRooted($relative)) { throw 'Source path boundary rejected.' }
    $source = Join-Path $repositoryRoot $relative
    if (!(Test-Path -LiteralPath $source -PathType Leaf)) { return }
    $parent = Get-Item -LiteralPath $source -Force
    while ($parent -and $parent.FullName.StartsWith($repositoryRoot, [StringComparison]::OrdinalIgnoreCase)) {
        if ($parent.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Reparse source archive input rejected.' }
        $parent = if ($parent -is [IO.DirectoryInfo]) { $parent.Parent } else { $parent.Directory }
    }
    [ordered]@{ path=$relative; sha256=(Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash.ToLowerInvariant() }
})
foreach ($file in $files) {
    $destination = Join-Path (Join-Path $output 'source') $file.path
    New-Item -ItemType Directory -Path (Split-Path -Parent $destination) -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $repositoryRoot $file.path) -Destination $destination
    if ((Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash.ToLowerInvariant() -ne $file.sha256) {
        throw "Source changed during archive creation: '$($file.path)'. Wait for source edits to finish, then rerun the build."
    }
}
$head = & git -C $repositoryRoot rev-parse HEAD
if ($LASTEXITCODE -ne 0) { throw 'Source commit unavailable.' }
$status = & git -C $repositoryRoot status --porcelain
if ($LASTEXITCODE -ne 0) { throw 'Source status unavailable.' }
$index = [ordered]@{ schemaVersion=1; commit="$head".Trim(); dirty=[bool]$status; files=$files;
    limitations='Exact local application source snapshot including uncommitted contents. Third-party runtime dependencies are identified by locked NuGet packages and the browser runtime lock. Independent reproducibility is separate.' }
[IO.File]::WriteAllText((Join-Path $output 'source-index.json'), ($index | ConvertTo-Json -Depth 8) + "`n", [Text.UTF8Encoding]::new($false))
