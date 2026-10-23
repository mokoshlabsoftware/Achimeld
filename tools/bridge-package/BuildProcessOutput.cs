using System.Diagnostics;

internal static class BuildProcessOutput
{
    internal static async Task RunAsync(ProcessStartInfo start, string stage, string failure)
    {
        // Hidden children need explicit pipes: inherited console handles may be
        // unavailable when dotnet itself is inside a PowerShell pipeline.
        start.RedirectStandardOutput = true;
        start.RedirectStandardError = true;
        Console.WriteLine(stage + "...");
        var elapsed = Stopwatch.StartNew();
        using var process = Process.Start(start) ?? throw new InvalidOperationException(failure);
        Task output = ForwardAsync(process.StandardOutput, Console.Out);
        Task errors = ForwardAsync(process.StandardError, Console.Error);
        Task exited = process.WaitForExitAsync();
        while (await Task.WhenAny(exited, Task.Delay(TimeSpan.FromSeconds(15))) != exited)
            Console.WriteLine($"{stage} is still running ({elapsed.Elapsed:mm\\:ss} elapsed).");
        await exited;
        await Task.WhenAll(output, errors);
        if (process.ExitCode != 0)
            throw new InvalidOperationException($"{failure} Exit code: {process.ExitCode}.");
        Console.WriteLine($"{stage} complete ({elapsed.Elapsed:mm\\:ss}).");
    }

    private static async Task ForwardAsync(StreamReader source, TextWriter destination)
    {
        while (await source.ReadLineAsync() is { } line)
            await destination.WriteLineAsync(line);
    }
}
