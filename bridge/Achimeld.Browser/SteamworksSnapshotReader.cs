using System.Globalization;
using System.Xml.Linq;
using Achimeld.Core;

namespace Achimeld.Browser;

public interface ICompletePortalDocumentSource
{
    Task<ManagedPageDocument> ReadCompleteConfigurationAsync(
        SteamAppId appId,
        CancellationToken cancellationToken = default);
}

public interface ISteamworksSnapshotReader
{
    Task<RemoteSnapshot> ReadSnapshotAsync(
        SteamAppId appId,
        CancellationToken cancellationToken = default);
}

public sealed class ManagedChromePortalDocumentSource : ICompletePortalDocumentSource
{
    private readonly IManagedChromeSession _session;

    public ManagedChromePortalDocumentSource(IManagedChromeSession session)
    {
        _session = session ?? throw new ArgumentNullException(nameof(session));
    }

    public Task<ManagedPageDocument> ReadCompleteConfigurationAsync(
        SteamAppId appId,
        CancellationToken cancellationToken = default) =>
        _session.ReadCompleteConfigurationAsync(
            appId,
            cancellationToken);
}

public sealed class SteamworksSnapshotReader :
    ISteamworksSnapshotReader
{
    private readonly ICompletePortalDocumentSource _source;
    private readonly SteamworksPageClassifier _classifier;
    private readonly TimeProvider _timeProvider;

    public SteamworksSnapshotReader(
        ICompletePortalDocumentSource source,
        SteamworksPageClassifier? classifier = null,
        TimeProvider? timeProvider = null)
    {
        _source = source ?? throw new ArgumentNullException(nameof(source));
        _classifier = classifier ?? new SteamworksPageClassifier();
        _timeProvider = timeProvider ?? TimeProvider.System;
    }

    public async Task<RemoteSnapshot> ReadSnapshotAsync(
        SteamAppId appId,
        CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        ManagedPageDocument document = await _source
            .ReadCompleteConfigurationAsync(appId, cancellationToken)
            .ConfigureAwait(false);
        cancellationToken.ThrowIfCancellationRequested();

        SteamworksPageClassification classification = _classifier.Classify(document, appId);
        if (!classification.IsAutomationSafe ||
            classification.PageKind != SteamworksPageKind.StatsAndAchievements)
        {
            throw new UnsupportedPortalStructureException(
                $"A complete snapshot cannot be read: {classification.Reason}");
        }

        XDocument html;
        try
        {
            html = XDocument.Parse(document.Html, LoadOptions.None);
        }
        catch (Exception exception) when (exception is System.Xml.XmlException or ArgumentException)
        {
            throw new UnsupportedPortalStructureException(
                "The supported fixture reader requires sanitized, well-formed static HTML.");
        }

        XElement root = html.Root ??
            throw new UnsupportedPortalStructureException("The portal document has no root element.");
        string revision = Required(root, "data-steamworks-revision");
        if (revision != SteamworksSelectorCatalog.SupportedRevision && revision != LiveSteamworksConfiguration.Revision)
            throw new UnsupportedPortalStructureException("Unsupported configuration revision.");
        RequireEqual(
            Required(root, "data-appid"),
            appId.Value.ToString(CultureInfo.InvariantCulture),
            "App ID");

        XElement statsSection = RequiredSection(root, "stats");
        XElement achievementsSection = RequiredSection(root, "achievements");
        XElement localizationsSection = RequiredSection(root, "localizations");
        XElement iconsSection = RequiredSection(root, "icons");

        RemoteStat[] stats = statsSection.Elements("article")
            .Select(element => ParseStat(element, revision != LiveSteamworksConfiguration.Revision))
            .OrderBy(static item => item.DisplayOrder)
            .ThenBy(static item => item.ApiName.Value, StringComparer.Ordinal)
            .ToArray();
        RemoteAchievement[] achievements = achievementsSection.Elements("article")
            .Select(element => ParseAchievement(element, revision != LiveSteamworksConfiguration.Revision))
            .OrderBy(static item => item.DisplayOrder)
            .ThenBy(static item => item.ApiName.Value, StringComparer.Ordinal)
            .ToArray();
        RemoteLocalization[] localizations = localizationsSection.Elements("article")
            .Select(ParseLocalization)
            .OrderBy(static item => item.ApiName.Value, StringComparer.Ordinal)
            .ThenBy(static item => item.Field)
            .ToArray();
        RemoteIconMetadata[] icons = iconsSection.Elements("article")
            .Select(ParseIcon)
            .OrderBy(static item => item.AchievementApiName.Value, StringComparer.Ordinal)
            .ToArray();

        string revisionFingerprint =
            CanonicalFingerprint.ComputeValue(revision);

        return new RemoteSnapshot(
            appId,
            _timeProvider.GetUtcNow(),
            revisionFingerprint,
            IsComplete: true,
            stats,
            achievements,
            localizations,
            icons) { SupportsDisplayOrder = revision != LiveSteamworksConfiguration.Revision };
    }

    private static RemoteStat ParseStat(XElement element, bool supportsDisplayOrder)
    {
        SteamApiName apiName = new(Required(element, "data-api-name"));
        StatType type = RequiredEnum<StatType>(element, "data-type");
        SetBy setBy = RequiredEnum<SetBy>(element, "data-set-by");
        bool incrementOnly = RequiredBoolean(element, "data-increment-only");
        decimal? minimum = OptionalDecimal(element, "data-minimum");
        decimal? maximum = OptionalDecimal(element, "data-maximum");
        decimal? defaultValue = OptionalDecimal(element, "data-default");
        decimal? maximumChange = OptionalDecimal(element, "data-maximum-change");
        bool aggregated = RequiredBoolean(element, "data-aggregated");
        decimal? averageWindow = OptionalDecimal(element, "data-average-window");
        int displayOrder = RequiredInteger(element, "data-display-order");

        string fingerprint = CanonicalFingerprint.Compute(
        [
            new("aggregated", aggregated),
            new("apiName", apiName.Value),
            new("averageRateWindowSeconds", averageWindow),
            new("defaultValue", defaultValue),
            new("displayOrder", supportsDisplayOrder ? (int?)displayOrder : null),
            new("incrementOnly", incrementOnly),
            new("maximum", maximum),
            new("maximumChange", maximumChange),
            new("minimum", minimum),
            new("setBy", setBy),
            new("type", type),
        ]).EntityFingerprint;

        return new RemoteStat(
            Optional(element, "data-steam-id"),
            apiName,
            type,
            setBy,
            incrementOnly,
            minimum,
            maximum,
            defaultValue,
            maximumChange,
            aggregated,
            averageWindow,
            displayOrder,
            fingerprint);
    }

    private static RemoteAchievement ParseAchievement(XElement element, bool supportsDisplayOrder)
    {
        SteamApiName apiName = new(Required(element, "data-api-name"));
        bool hidden = RequiredBoolean(element, "data-hidden");
        SetBy setBy = RequiredEnum<SetBy>(element, "data-set-by");
        string? progressValue = Optional(element, "data-progress-stat");
        SteamApiName? progressStat = string.IsNullOrEmpty(progressValue)
            ? null
            : new SteamApiName(progressValue);
        decimal? unlockValue = OptionalDecimal(element, "data-unlock-value");
        int displayOrder = RequiredInteger(element, "data-display-order");

        string fingerprint = CanonicalFingerprint.Compute(
        [
            new("apiName", apiName.Value),
            new("displayOrder", supportsDisplayOrder ? (int?)displayOrder : null),
            new("hidden", hidden),
            new("progressStatApiName", progressStat?.Value),
            new("setBy", setBy),
            new("unlockValue", unlockValue),
        ]).EntityFingerprint;

        return new RemoteAchievement(
            Optional(element, "data-steam-id"),
            apiName,
            hidden,
            setBy,
            progressStat,
            unlockValue,
            displayOrder,
            fingerprint);
    }

    private static RemoteLocalization ParseLocalization(XElement element)
    {
        ProjectEntityType entityType = RequiredEnum<ProjectEntityType>(element, "data-entity-type");
        SteamApiName apiName = new(Required(element, "data-api-name"));
        LocalizationField field = RequiredEnum<LocalizationField>(element, "data-field");
        Dictionary<string, string> parsedValues = element.Elements("span")
            .Select(item => new
            {
                Language = Required(item, "data-language"),
                Value = item.Value,
            })
            .ToDictionary(static item => item.Language, static item => item.Value, StringComparer.Ordinal);
        string[] supportedLanguages = ProjectLocalizationCatalog.Columns.Skip(2).ToArray();
        if (parsedValues.Keys.Any(language => !supportedLanguages.Contains(language, StringComparer.Ordinal)))
        {
            throw new UnsupportedPortalStructureException(
                $"Localization '{apiName}' contains an unsupported language.");
        }

        Dictionary<string, string> values = supportedLanguages.ToDictionary(
            static language => language,
            language => parsedValues.GetValueOrDefault(language, string.Empty),
            StringComparer.Ordinal);
        if (string.IsNullOrWhiteSpace(values["English"]))
        {
            throw new UnsupportedPortalStructureException(
                $"Localization '{apiName}' does not contain required English text.");
        }

        string fingerprint = CanonicalFingerprint.Compute(
            values.OrderBy(static item => item.Key, StringComparer.Ordinal)
                .Select(static item => new CanonicalField(item.Key, item.Value))).EntityFingerprint;
        return new RemoteLocalization(entityType, apiName, field, values, fingerprint);
    }

    private static RemoteIconMetadata ParseIcon(XElement element)
    {
        SteamApiName apiName = new(Required(element, "data-api-name"));
        string? achievedIdentity = Optional(element, "data-achieved-identity");
        string? unachievedIdentity = Optional(element, "data-unachieved-identity");
        Uri? achievedPreview = OptionalUri(element, "data-achieved-preview");
        Uri? unachievedPreview = OptionalUri(element, "data-unachieved-preview");
        string fingerprint = CanonicalFingerprint.Compute(
        [
            new("achievedIdentity", achievedIdentity),
            new("unachievedIdentity", unachievedIdentity),
        ]).EntityFingerprint;
        return new RemoteIconMetadata(
            apiName,
            achievedIdentity,
            unachievedIdentity,
            achievedPreview,
            unachievedPreview,
            fingerprint);
    }

    private static XElement RequiredSection(XElement root, string id) =>
        root.Descendants("section").SingleOrDefault(
            item => string.Equals(Optional(item, "id"), id, StringComparison.Ordinal))
        ?? throw new UnsupportedPortalStructureException($"Required '{id}' section is missing or duplicated.");

    private static string Required(XElement element, string attribute) =>
        Optional(element, attribute) is { Length: > 0 } value
            ? value
            : throw new UnsupportedPortalStructureException($"Required attribute '{attribute}' is missing.");

    private static string? Optional(XElement element, string attribute) =>
        element.Attribute(attribute)?.Value;

    private static Uri? OptionalUri(XElement element, string attribute)
    {
        string? value = Optional(element, attribute);
        return string.IsNullOrWhiteSpace(value)
            ? null
            : Uri.TryCreate(value, UriKind.Absolute, out Uri? uri) &&
              string.Equals(uri.Scheme, Uri.UriSchemeHttps, StringComparison.Ordinal)
                ? uri
                : throw new UnsupportedPortalStructureException(
                    $"Attribute '{attribute}' is not an absolute HTTPS URI.");
    }

    private static T RequiredEnum<T>(XElement element, string attribute)
        where T : struct, Enum =>
        Enum.TryParse(Required(element, attribute), ignoreCase: false, out T value)
            ? value
            : throw new UnsupportedPortalStructureException($"Attribute '{attribute}' has an unsupported value.");

    private static bool RequiredBoolean(XElement element, string attribute) =>
        bool.TryParse(Required(element, attribute), out bool value)
            ? value
            : throw new UnsupportedPortalStructureException($"Attribute '{attribute}' is not Boolean.");

    private static int RequiredInteger(XElement element, string attribute) =>
        int.TryParse(Required(element, attribute), NumberStyles.Integer, CultureInfo.InvariantCulture, out int value)
            ? value
            : throw new UnsupportedPortalStructureException($"Attribute '{attribute}' is not an integer.");

    private static decimal? OptionalDecimal(XElement element, string attribute)
    {
        string? value = Optional(element, attribute);
        if (string.IsNullOrWhiteSpace(value))
        {
            return null;
        }

        return decimal.TryParse(value, NumberStyles.Number, CultureInfo.InvariantCulture, out decimal parsed)
            ? parsed
            : throw new UnsupportedPortalStructureException($"Attribute '{attribute}' is not decimal.");
    }

    private static void RequireEqual(string actual, string expected, string name)
    {
        if (!string.Equals(actual, expected, StringComparison.Ordinal))
        {
            throw new UnsupportedPortalStructureException($"The portal {name} does not match the expected value.");
        }
    }
}
