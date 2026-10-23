using System.Diagnostics.CodeAnalysis;
using System.Text.RegularExpressions;

namespace Achimeld.Browser;

public sealed partial class RedactionService
{
    public const string Mask = "[REDACTED]";

    private static readonly HashSet<string> SensitiveQueryNames = new(
        [
            "access_token",
            "api_key",
            "apikey",
            "auth",
            "authorization",
            "cookie",
            "key",
            "login",
            "oauth_token",
            "password",
            "session",
            "sessionid",
            "steamguard",
            "steamlogin",
            "steamloginsecure",
            "storage_state",
            "token",
        ],
        StringComparer.OrdinalIgnoreCase);

    [SuppressMessage("Performance", "CA1822:Mark members as static", Justification = "Service is injected into diagnostics.")]
    public string Redact(string? value)
    {
        if (string.IsNullOrEmpty(value))
        {
            return value ?? string.Empty;
        }

        string redacted = SecretAssignment().Replace(
            value,
            static match => match.Groups["name"].Value + "=" + Mask);
        redacted = AuthorizationValue().Replace(redacted, "Authorization: " + Mask);
        return Url().Replace(redacted, match =>
            Uri.TryCreate(match.Value, UriKind.Absolute, out Uri? uri)
                ? RedactUri(uri)
                : Mask);
    }

    public static string RedactUri(Uri uri)
    {
        ArgumentNullException.ThrowIfNull(uri);
        if (!uri.IsAbsoluteUri)
        {
            return Mask;
        }

        UriBuilder builder = new(uri)
        {
            UserName = string.Empty,
            Password = string.Empty,
        };
        List<string> pairs = [];
        foreach (string pair in uri.Query.TrimStart('?').Split('&', StringSplitOptions.RemoveEmptyEntries))
        {
            string[] parts = pair.Split('=', 2);
            string decodedName = Uri.UnescapeDataString(parts[0].Replace('+', ' '));
            string value = parts.Length == 2 ? parts[1] : string.Empty;
            pairs.Add(parts[0] + "=" +
                (IsSensitiveQueryName(decodedName) ? Uri.EscapeDataString(Mask) : value));
        }

        builder.Query = string.Join("&", pairs);
        return builder.Uri.AbsoluteUri;
    }

    public IReadOnlyDictionary<string, string> RedactMetadata(
        IEnumerable<KeyValuePair<string, string>> metadata)
    {
        ArgumentNullException.ThrowIfNull(metadata);
        return metadata.ToDictionary(
            static item => item.Key,
            item => IsSensitiveQueryName(item.Key) ? Mask : Redact(item.Value),
            StringComparer.Ordinal);
    }

    private static bool IsSensitiveQueryName(string name) =>
        SensitiveQueryNames.Contains(
            name.Replace("-", string.Empty, StringComparison.Ordinal)
                .Replace("_", string.Empty, StringComparison.Ordinal)) ||
        SensitiveQueryNames.Contains(name) ||
        name.Contains("password", StringComparison.OrdinalIgnoreCase) ||
        name.Contains("cookie", StringComparison.OrdinalIgnoreCase) ||
        name.Contains("authorization", StringComparison.OrdinalIgnoreCase) ||
        name.Contains("steamguard", StringComparison.OrdinalIgnoreCase) ||
        name.Contains("apikey", StringComparison.OrdinalIgnoreCase) ||
        name.Contains("storage", StringComparison.OrdinalIgnoreCase) &&
            name.Contains("state", StringComparison.OrdinalIgnoreCase);

    [GeneratedRegex(
        @"(?<name>cookie|set-cookie|password|passwd|steam[\s_-]*guard|steamloginsecure|sessionid|api[\s_-]*key|access[\s_-]*token|storage[\s_-]*state)[""']?\s*[:=]\s*(?:""[^""]*""|'[^']*'|[^\s,;&}]+)",
        RegexOptions.IgnoreCase | RegexOptions.CultureInvariant)]
    private static partial Regex SecretAssignment();

    [GeneratedRegex(
        @"authorization[""']?\s*:\s*[""']?(?:bearer|basic)?\s*[^\s,"";}]+[""']?",
        RegexOptions.IgnoreCase | RegexOptions.CultureInvariant)]
    private static partial Regex AuthorizationValue();

    [GeneratedRegex(
        @"https?://[^\s""'<>]+",
        RegexOptions.IgnoreCase | RegexOptions.CultureInvariant)]
    private static partial Regex Url();
}

public sealed record BrowserDiagnosticEvent(
    DateTimeOffset TimestampUtc,
    string EventName,
    IReadOnlyDictionary<string, string> Metadata);

public sealed class MaskedScreenshot
{
    internal MaskedScreenshot(byte[] pngBytes)
    {
        PngBytes = pngBytes;
    }

    public ReadOnlyMemory<byte> PngBytes { get; }
}

public interface IScreenshotMasker
{
    Task<byte[]> MaskSensitiveRegionsAsync(
        ReadOnlyMemory<byte> rawPng,
        CancellationToken cancellationToken = default);
}

public sealed class DiagnosticScreenshotFactory
{
    private readonly IScreenshotMasker _masker;

    public DiagnosticScreenshotFactory(IScreenshotMasker masker)
    {
        _masker = masker ?? throw new ArgumentNullException(nameof(masker));
    }

    public async Task<MaskedScreenshot> CreateMaskedAsync(
        ReadOnlyMemory<byte> rawPng,
        CancellationToken cancellationToken = default)
    {
        byte[] masked = await _masker
            .MaskSensitiveRegionsAsync(rawPng, cancellationToken)
            .ConfigureAwait(false);
        if (masked.Length == 0)
        {
            throw new InvalidOperationException("The screenshot masker returned no diagnostic image.");
        }

        return new MaskedScreenshot(masked);
    }
}

public sealed class BrowserDiagnostics
{
    private readonly RedactionService _redaction;
    private readonly List<BrowserDiagnosticEvent> _events = [];
    private readonly List<MaskedScreenshot> _screenshots = [];

    public BrowserDiagnostics(RedactionService? redaction = null)
    {
        _redaction = redaction ?? new RedactionService();
    }

    public IReadOnlyList<BrowserDiagnosticEvent> Events => _events.AsReadOnly();
    public IReadOnlyList<MaskedScreenshot> Screenshots => _screenshots.AsReadOnly();

    public void AddMetadata(
        string eventName,
        IEnumerable<KeyValuePair<string, string>> metadata)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(eventName);
        _events.Add(new BrowserDiagnosticEvent(
            DateTimeOffset.UtcNow,
            eventName,
            _redaction.RedactMetadata(metadata)));
    }

    public void AddMaskedScreenshot(MaskedScreenshot screenshot)
    {
        ArgumentNullException.ThrowIfNull(screenshot);
        _screenshots.Add(screenshot);
    }
}
