using System.Diagnostics;
using System.IO.Pipes;
using System.Runtime.InteropServices;
using System.Security.Cryptography;

namespace Achimeld.Security;

public sealed record SupervisionRequest(string Operation, string Nonce, string? ProbeToken = null, string? FailureCode = null);
public sealed record DevelopmentNetworkTest(bool Passed, string Stage);
public sealed record SupervisionStatus(string Nonce, string SessionId, int SupervisorPid, string PackageVersion,
    string PackageMode, bool NetworkEnforced, bool MonitoringActive, string MonitoringCoverage,
    int ProxyPort, string BrowserExecutable, string ProfileDirectory, int ProbePort, bool? UdpProbeDenied = null, string? FailureCode = null, string? ReportJson = null);

public sealed class SupervisionSession
{
    private readonly string _pipeName;
    private readonly string _root;
    private readonly SupervisionStatus _initial;

    private SupervisionSession(string pipeName, string root, SupervisionStatus status)
    {
        _pipeName = pipeName;
        _root = root;
        _initial = status;
        Status = status;
    }

    public SupervisionStatus Status { get; private set; }
    public string PackageRoot => _root;

    public async Task ReportPolicyFailureAsync(CancellationToken cancellationToken) =>
        _ = await ExchangeAsync(_pipeName, _root, "policyFailure", cancellationToken, failureCode: NetworkSelfTest.LastFailureCode).ConfigureAwait(false);

    public async Task<bool> ConfirmUdpDenialAsync(bool ipv6, string probeToken, CancellationToken cancellationToken)
    {
        if (!Status.NetworkEnforced) throw new InvalidOperationException("Windows network isolation is not enabled for this session.");
        SupervisionStatus status = await ExchangeAsync(_pipeName,_root,ipv6 ? "udpProbe6" : "udpProbe4",cancellationToken,probeToken).ConfigureAwait(false);
        ValidateContinuity(_initial, status);
        ValidateHealthy(status);
        return status.UdpProbeDenied == true;
    }

    public async Task<string> ExportReportAsync(CancellationToken cancellationToken)
    {
        await RequireHealthyAsync(cancellationToken).ConfigureAwait(false);
        SupervisionStatus status = await ExchangeAsync(_pipeName, _root, "report", cancellationToken).ConfigureAwait(false);
        ValidateContinuity(_initial, status);
        if (status.ReportJson is null)
            throw new InvalidDataException("The security report could not be authenticated.");
        return status.ReportJson;
    }

    public static async Task<SupervisionSession> ConnectAsync(CancellationToken cancellationToken)
    {
        if (!OperatingSystem.IsWindows()) throw new PlatformNotSupportedException("Achimeld security requires Windows.");
        string pipeName = Environment.GetEnvironmentVariable("ACHIMELD_SUPERVISION_PIPE") ?? "";
        if (!pipeName.StartsWith("Achimeld.Security.", StringComparison.Ordinal) || pipeName.Length > 100 ||
            pipeName.Any(character => !char.IsAsciiLetterOrDigit(character) && character != '.'))
            throw new InvalidOperationException("Start Achimeld through Achimeld.exe to establish security supervision.");
        string root = Path.TrimEndingDirectorySeparator(Path.GetFullPath(AppContext.BaseDirectory));
        string configuredRoot = Path.TrimEndingDirectorySeparator(Path.GetFullPath(Environment.GetEnvironmentVariable("ACHIMELD_PACKAGE_ROOT") ?? ""));
        if (!string.Equals(root, configuredRoot, StringComparison.OrdinalIgnoreCase))
            throw new InvalidDataException("The supervision session belongs to another installation.");
        SupervisionStatus status = await ExchangeAsync(pipeName, root, "hello", cancellationToken).ConfigureAwait(false);
        if (status.SessionId.Length != 64 || !status.SessionId.All(char.IsAsciiHexDigit))
            throw new InvalidDataException("Invalid supervision session identity.");
        ValidateHealthy(status);
        string browser = Path.GetFullPath(status.BrowserExecutable);
        if (!browser.StartsWith(Path.Combine(root,"browser") + Path.DirectorySeparatorChar,StringComparison.OrdinalIgnoreCase))
            throw new InvalidDataException("The supervised browser is outside the verified package.");
        DedicatedProfilePolicy.RejectReparsePoints(browser);
        _ = DedicatedProfilePolicy.Resolve(status.ProfileDirectory);
        return new SupervisionSession(pipeName, root, status);
    }

    public async Task RequireHealthyAsync(CancellationToken cancellationToken)
    {
        SupervisionStatus status = await ExchangeAsync(_pipeName, _root, "status", cancellationToken).ConfigureAwait(false);
        ValidateContinuity(_initial, status);
        Status = status;
        ValidateHealthy(status);
    }

    public async Task ClearProfileAsync(CancellationToken cancellationToken)
    {
        await RequireHealthyAsync(cancellationToken).ConfigureAwait(false);
        SupervisionStatus status = await ExchangeAsync(_pipeName, _root, "clearProfile", cancellationToken).ConfigureAwait(false);
        ValidateContinuity(_initial, status);
        if (status.FailureCode is not null)
            throw new InvalidOperationException("The dedicated browser profile could not be cleared. Close the Steam browser and try again.");
    }

    public static void ValidateContinuity(SupervisionStatus initial, SupervisionStatus next)
    {
        if (initial.SupervisorPid != next.SupervisorPid || initial.SessionId != next.SessionId ||
            initial.PackageVersion != next.PackageVersion || initial.PackageMode != next.PackageMode ||
            initial.NetworkEnforced != next.NetworkEnforced ||
            initial.ProxyPort != next.ProxyPort || initial.ProbePort != next.ProbePort ||
            initial.BrowserExecutable != next.BrowserExecutable || initial.ProfileDirectory != next.ProfileDirectory ||
            initial.MonitoringCoverage != next.MonitoringCoverage)
            throw new InvalidDataException("Security supervision configuration changed.");
        ValidateHealthy(next);
    }

    private static void ValidateHealthy(SupervisionStatus status)
    {
        // NetworkEnforced distinguishes a user session from an explicitly
        // protected installation. Both require authenticated active supervision;
        // ValidateContinuity pins the protection level for the entire session.
        if (!status.MonitoringActive || status.FailureCode is not null)
            throw new InvalidOperationException("Security supervision is unavailable; Steam automation is stopped.");
        if (status.PackageMode is not ("development" or "official")) throw new InvalidDataException("Unknown package mode.");
        if (status.ProxyPort is < 1024 or > 65535 || status.ProbePort is < 1024 or > 65535) throw new InvalidDataException("Invalid supervised endpoint.");
    }

    private static async Task<SupervisionStatus> ExchangeAsync(string pipeName, string root, string operation,
        CancellationToken cancellationToken, string? probeToken = null, string? failureCode = null)
    {
        using var timeout = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        timeout.CancelAfter(TimeSpan.FromSeconds(5));
        await using var pipe = new NamedPipeClientStream(".", pipeName, PipeDirection.InOut,
            PipeOptions.Asynchronous | PipeOptions.CurrentUserOnly);
        await pipe.ConnectAsync(timeout.Token).ConfigureAwait(false);
        if (!GetNamedPipeServerProcessId(pipe.SafePipeHandle.DangerousGetHandle(), out uint pid))
            throw new InvalidDataException("Cannot authenticate the security supervisor.");
        using Process supervisor = Process.GetProcessById(checked((int)pid));
        string expected = Path.GetFullPath(Path.Combine(root, "Achimeld.exe"));
        if (!string.Equals(supervisor.MainModule?.FileName, expected, StringComparison.OrdinalIgnoreCase))
            throw new InvalidDataException("The supervision endpoint belongs to an unexpected executable.");
        string nonce = Convert.ToHexString(RandomNumberGenerator.GetBytes(32));
        await SecurityWire.WriteAsync(pipe, new SupervisionRequest(operation, nonce, probeToken, failureCode), timeout.Token).ConfigureAwait(false);
        SupervisionStatus status = await SecurityWire.ReadAsync<SupervisionStatus>(pipe, timeout.Token, 64 * 1024).ConfigureAwait(false);
        if (status.Nonce != nonce || status.SupervisorPid != pid)
            throw new InvalidDataException("The supervision response did not match this request.");
        // The native server may disconnect only after the response is consumed.
        await pipe.WriteAsync("ACK1"u8.ToArray(), timeout.Token).ConfigureAwait(false);
        return status;
    }

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool GetNamedPipeServerProcessId(nint pipe, out uint serverProcessId);
}
