# Only the fixed development package may be cleaned up. An editor, supervisor,
# verifier, worker or unknown command always leaves the normal build guard intact.
function Test-AchimeldMcpProcess {
    param($Process, [string]$PackagePath)
    $executable = Join-Path ([IO.Path]::GetFullPath($PackagePath)) 'Achimeld.exe'
    if (!$Process.ExecutablePath -or !$Process.ExecutablePath.Equals($executable, [StringComparison]::OrdinalIgnoreCase)) { return $false }
    $escaped = [regex]::Escape($executable)
    return $Process.CommandLine -cmatch ('^\s*(?i:"' + $escaped + '"|' + $escaped + ')\s+--mcp\s*$')
}

function Stop-AchimeldIdleDevelopmentMcp {
    param([Parameter(Mandatory)][string]$PackagePath)
    $prefix = [IO.Path]::GetFullPath($PackagePath).TrimEnd('\', '/') + '\'
    $processes = @(Get-CimInstance Win32_Process -Filter "Name='Achimeld.exe' OR Name='Achimeld.App.exe' OR Name='achimeld-app.exe' OR Name='achimeld-node-host.exe' OR Name='Achimeld.SteamWorker.exe'" |
        Where-Object { $_.ExecutablePath -and $_.ExecutablePath.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase) })
    if (!$processes.Count -or @($processes | Where-Object { !(Test-AchimeldMcpProcess $_ $PackagePath) }).Count) { return }
    foreach ($candidate in $processes) {
        $process = Get-Process -Id $candidate.ProcessId -ErrorAction SilentlyContinue
        if (!$process) { continue }
        try {
            # Pin the actual process handle before validating CIM's earlier PID
            # observation. A recycled PID must never select a different process.
            $null = $process.Handle
            if ($process.HasExited) { continue }
            # CIM exposes microseconds; Get-Process retains 100-nanosecond ticks.
            $startedTicks = $process.StartTime.ToUniversalTime().Ticks
            $observedTicks = $candidate.CreationDate.ToUniversalTime().Ticks
            if (!$process.Path.Equals($candidate.ExecutablePath, [StringComparison]::OrdinalIgnoreCase) -or
                ($startedTicks - ($startedTicks % 10)) -ne ($observedTicks - ($observedTicks % 10))) {
                throw 'Development process identity changed; retry after closing Achimeld.'
            }
            $process.Kill()
            if (!$process.WaitForExit(5000)) { throw 'The idle editor connector did not close.' }
            Write-Host ('Closed an idle editor connector from the previous build (process ' + $candidate.ProcessId + ').')
        } finally { $process.Dispose() }
    }
}
