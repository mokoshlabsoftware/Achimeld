[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$cleanup = Join-Path $PSScriptRoot 'Clear-CodeArtifacts.ps1'
$repository = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$fixture = Join-Path ([IO.Path]::GetTempPath()) ('achimeld-cleanup-' + [Guid]::NewGuid().ToString('N'))
$fixture = [IO.Path]::GetFullPath($fixture)
if ($fixture.StartsWith($repository.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Cleanup fixtures must live outside source.'
}
$junction = Join-Path $fixture 'current/worker-fixture/linked'
try {
    $remove = @('current/worker-fixture/worker.exe', ('current/bridge-package-' + ('a' * 32) + '/worker.dll'),
        ('current/bridge-trust-' + ('b' * 16) + '.json'), 'current/offline-nuget.config')
    $keep = @('current/target/debug/incremental/output.obj', 'current/dotnet/obj/worker/output.dll',
        'current/package/Achimeld.exe', 'current/Run-Achimeld.ps1', 'current/Build-And-Run.ps1',
        'current/last-archive.txt', 'current/audit-fixture/evidence.json', 'current/desktop-install-fixture.log',
        'current/bridge-package-not-a-build/keep.txt', 'cache/nuget/dependency.nupkg',
        '15_10_2026/build-fixture/Achimeld.exe', 'source-art.png')
    foreach ($relative in @($remove) + @($keep)) {
        $file = Join-Path $fixture $relative
        New-Item -ItemType Directory -Path (Split-Path -Parent $file) -Force | Out-Null
        [IO.File]::WriteAllText($file, $relative)
    }
    & $cleanup -BuildRoot $fixture
    foreach ($relative in $remove) {
        if (Test-Path -LiteralPath (Join-Path $fixture $relative)) { throw "Code artifact survived: $relative" }
    }
    foreach ($relative in $keep) {
        if ([IO.File]::ReadAllText((Join-Path $fixture $relative)) -cne $relative) { throw "Preserved file changed: $relative" }
    }
    & $cleanup -BuildRoot $fixture
    & $cleanup -BuildRoot (Join-Path $fixture 'absent')
    foreach ($invalid in @($repository, (Join-Path $repository 'nested-build'))) {
        $rejected = $false
        try { & $cleanup -BuildRoot $invalid } catch { $rejected = $true }
        if (!$rejected) { throw 'In-source cleanup root accepted.' }
    }
    # A junction inside a deletion target must reject the whole cleanup, leaving
    # both ordinary artifacts and the linked files untouched.
    New-Item -ItemType Directory -Path (Join-Path $fixture 'current/worker-fixture') -Force | Out-Null
    $ordinary = Join-Path $fixture ('current/bridge-trust-' + ('c' * 16) + '.json')
    [IO.File]::WriteAllText($ordinary, 'keep')
    New-Item -ItemType Junction -Path $junction -Target (Join-Path $fixture 'cache') | Out-Null
    $rejected = $false
    try { & $cleanup -BuildRoot $fixture } catch { $rejected = $true }
    if (!$rejected -or !(Test-Path -LiteralPath $ordinary) -or
        !(Test-Path -LiteralPath (Join-Path $fixture 'cache/nuget/dependency.nupkg'))) {
        throw 'Junction preflight did not preserve the deletion set and linked data.'
    }
    Remove-Item -LiteralPath $junction -Force
    # The build root itself cannot redirect cleanup through a junction either.
    New-Item -ItemType Junction -Path $junction -Target $fixture | Out-Null
    $rejected = $false
    try { & $cleanup -BuildRoot $junction } catch { $rejected = $true }
    if (!$rejected) { throw 'Linked build root accepted.' }
    Remove-Item -LiteralPath $junction -Force
    & $cleanup -BuildRoot $fixture
    Write-Output 'Code artifact cleanup, retained packages/cache/evidence, repeat runs and directory-link rejection passed.'
} finally {
    # Both removals are confined to this test's absolute temporary directory.
    if (Test-Path -LiteralPath $junction) { Remove-Item -LiteralPath $junction -Force }
    if (Test-Path -LiteralPath $fixture) { Remove-Item -LiteralPath $fixture -Recurse -Force }
}
