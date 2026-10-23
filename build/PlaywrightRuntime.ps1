# The Windows Node host owns a kill-on-close Job for the driver and all browser
# descendants. Playwright's shell-based taskkill fallback is outside that recipe.
function Set-AchimeldPlaywrightRuntime {
    param([Parameter(Mandatory)][string]$PackageDirectory)
    $relative = '.playwright/package/lib/coreBundle.js'
    $path = Join-Path $PackageDirectory $relative
    $originalHash = '3258d1cf334c6afc95f22aa9c292436cb976b391e0437f1359c83b84f0cb9d66'
    if ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() -cne $originalHash) {
        throw 'Unrecognized Playwright 1.62.0 runtime; review the supervised cleanup patch before updating dependencies.'
    }
    $original = @(
        '          const taskkillProcess = childProcess.spawnSync(`taskkill /pid ${spawnedProcess.pid} /T /F`, { shell: true });'
        '          const [stdout2, stderr2] = [taskkillProcess.stdout.toString(), taskkillProcess.stderr.toString()];'
        '          if (stdout2)'
        '            options.log(`[pid=${spawnedProcess.pid}] taskkill stdout: ${stdout2}`);'
        '          if (stderr2)'
        '            options.log(`[pid=${spawnedProcess.pid}] taskkill stderr: ${stderr2}`);'
    ) -join "`n"
    $replacement = @(
        '          // Achimeld: terminate the browser directly, without a shell or taskkill.'
        '          // The authenticated Node host Job terminates descendants on driver exit.'
        '          spawnedProcess.kill("SIGKILL");'
    ) -join "`n"
    $source = [IO.File]::ReadAllText($path)
    $position = $source.IndexOf($original, [StringComparison]::Ordinal)
    if ($position -lt 0 -or $position -ne $source.LastIndexOf($original, [StringComparison]::Ordinal)) {
        throw 'Playwright forced cleanup patch must match exactly one reviewed call.'
    }
    [IO.File]::WriteAllText($path, $source.Replace($original, $replacement), [Text.UTF8Encoding]::new($false))
    [ordered]@{
        id = 'supervised-windows-cleanup-v1'; path = $relative
        upstreamSha256 = $originalHash
        patchedSha256 = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
    }
}
