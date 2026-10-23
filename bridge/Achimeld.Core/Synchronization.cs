using System.Diagnostics.CodeAnalysis;
using System.Globalization;
using System.Security.Cryptography;
using System.Text;

namespace Achimeld.Core;

public readonly record struct SyncOperationId(string Value)
{
    public override string ToString() => Value ?? string.Empty;
}

public sealed record SyncWarning(string Code, string Message, ProjectEntityType EntityType, Guid? LocalId = null);

public sealed record SyncEntityStatus(
    ProjectEntityType EntityType,
    Guid? LocalId,
    SteamApiName ApiName,
    SyncState State,
    string? LocalFingerprint,
    string? RemoteFingerprint,
    string? BaselineFingerprint);

public static class SyncFingerprint
{
    public const string Absent = "absent";
}

public sealed record SyncTargetFingerprint(
    ProjectEntityType EntityType,
    string TargetKey,
    LocalizationField? LocalizationField,
    string ExpectedOldFingerprint,
    string DesiredFingerprint);

public abstract record SyncOperation
{
    protected SyncOperation(
        SyncOperationId id,
        ProjectEntityType entityType,
        string targetKey,
        IReadOnlySet<SyncOperationId>? dependsOn = null)
    {
        Id = id;
        EntityType = entityType;
        TargetKey = targetKey;
        DependsOn = dependsOn ?? new HashSet<SyncOperationId>();
    }

    public SyncOperationId Id { get; init; }
    public ProjectEntityType EntityType { get; init; }
    public string TargetKey { get; init; }
    public IReadOnlySet<SyncOperationId> DependsOn { get; init; }
    public bool Included { get; init; } = true;
    public bool RequiresExplicitInclusion { get; init; }
    public IReadOnlyList<SyncTargetFingerprint> Targets { get; init; } = [];
}

public sealed record DeleteAchievement : SyncOperation
{
    public DeleteAchievement(SyncOperationId id, SteamApiName apiName)
        : base(id, ProjectEntityType.Achievement, apiName.Value) { }
}

public sealed record UpsertStat : SyncOperation
{
    public UpsertStat(SyncOperationId id, StatDefinition stat, bool isRequiredByAchievement = false)
        : base(id, ProjectEntityType.Stat, stat.ApiName.Value)
    {
        Stat = stat;
        IsRequiredByAchievement = isRequiredByAchievement;
    }

    public StatDefinition Stat { get; init; }
    public bool IsRequiredByAchievement { get; init; }
}

public sealed record UpsertAchievement : SyncOperation
{
    public UpsertAchievement(
        SyncOperationId id,
        AchievementDefinition achievement,
        IReadOnlySet<SyncOperationId> dependsOn)
        : base(id, ProjectEntityType.Achievement, achievement.ApiName.Value, dependsOn) =>
        Achievement = achievement;

    public AchievementDefinition Achievement { get; init; }
}

public sealed record UpsertLocalization : SyncOperation
{
    public UpsertLocalization(
        SyncOperationId id,
        ProjectEntityType entityType,
        string targetKey,
        IReadOnlyList<ProjectLocalizationRow> rows,
        IReadOnlySet<SyncOperationId> dependsOn)
        : base(id, entityType, targetKey, dependsOn) => Rows = rows;

    public IReadOnlyList<ProjectLocalizationRow> Rows { get; init; }
}

public sealed record UploadIconPair : SyncOperation
{
    public UploadIconPair(
        SyncOperationId id,
        AchievementDefinition achievement,
        IReadOnlySet<SyncOperationId> dependsOn)
        : base(id, ProjectEntityType.Achievement, achievement.ApiName.Value, dependsOn) =>
        Achievement = achievement;

    public AchievementDefinition Achievement { get; init; }
}

public sealed record ImportRemote : SyncOperation
{
    public ImportRemote(
        SyncOperationId id,
        ProjectEntityType entityType,
        string targetKey,
        string remoteFingerprint)
        : base(id, entityType, targetKey)
    {
        RemoteFingerprint = remoteFingerprint;
        Included = false;
        RequiresExplicitInclusion = true;
    }

    public string RemoteFingerprint { get; init; }
}

public sealed record VerifyTargets : SyncOperation
{
    public VerifyTargets(SyncOperationId id, IReadOnlySet<SyncOperationId> dependsOn)
        : base(id, ProjectEntityType.Achievement, "*", dependsOn)
    {
    }
}

public sealed record SyncPlan(
    SteamAppId AppId,
    IReadOnlyList<SyncEntityStatus> Entities,
    IReadOnlyList<SyncOperation> Operations,
    IReadOnlyList<SyncWarning> Warnings)
{
    public bool HasConflicts => Entities.Any(static entity => entity.State == SyncState.Conflict);

    public SyncPlan ApplyExclusions(IEnumerable<SyncOperationId> explicitlyExcluded)
        => ApplySelection(explicitlyExcluded, []);

    public SyncPlan ApplySelection(
        IEnumerable<SyncOperationId> explicitlyExcluded,
        IEnumerable<SyncOperationId> explicitlyIncluded)
    {
        ArgumentNullException.ThrowIfNull(explicitlyExcluded);
        ArgumentNullException.ThrowIfNull(explicitlyIncluded);
        HashSet<SyncOperationId> excluded = [.. explicitlyExcluded];
        HashSet<SyncOperationId> explicitIncludes = [.. explicitlyIncluded];
        foreach (DeleteAchievement deletion in Operations.OfType<DeleteAchievement>())
        {
            if (Operations.OfType<ImportRemote>().Any(import =>
                import.EntityType == ProjectEntityType.Achievement && import.TargetKey == deletion.TargetKey &&
                explicitIncludes.Contains(import.Id)))
                excluded.Add(deletion.Id);
        }
        if (Operations.Any(operation =>
                explicitIncludes.Contains(operation.Id) && !operation.RequiresExplicitInclusion))
        {
            throw new ArgumentException(
                "Only operations marked as requiring explicit inclusion can be explicitly included.",
                nameof(explicitlyIncluded));
        }

        bool changed;
        do
        {
            changed = false;
            foreach (SyncOperation operation in Operations.Where(static operation => operation is not VerifyTargets))
            {
                if (!excluded.Contains(operation.Id) && operation.DependsOn.Any(excluded.Contains))
                {
                    changed |= excluded.Add(operation.Id);
                }
            }
        }
        while (changed);

        HashSet<SyncOperationId> includedTargets = Operations
            .Where(operation => operation is not VerifyTargets and not ImportRemote &&
                (!operation.RequiresExplicitInclusion || explicitIncludes.Contains(operation.Id)) &&
                !excluded.Contains(operation.Id))
            .Select(static operation => operation.Id)
            .ToHashSet();
        return this with
        {
            Operations = Operations
                .Select(operation => operation is VerifyTargets
                    ? operation with
                    {
                        DependsOn = includedTargets,
                        Included = includedTargets.Count > 0 && !excluded.Contains(operation.Id),
                    }
                    : operation with
                    {
                        Included = !excluded.Contains(operation.Id) &&
                            (!operation.RequiresExplicitInclusion || explicitIncludes.Contains(operation.Id)),
                    })
                .ToArray(),
        };
    }
}

public sealed class IncompleteRemoteSnapshotException : InvalidOperationException
{
    public IncompleteRemoteSnapshotException()
        : base("Synchronization planning requires a complete remote snapshot.")
    {
    }
}

public sealed class SyncPlanner(bool recreateRenamedAchievements = false)
{
    [SuppressMessage("Performance", "CA1822:Mark members as static", Justification = "The planner is an injectable application service.")]
    public SyncPlan CreatePlan(
        AchimeldProject project,
        RemoteSnapshot remote,
        ProjectLocalizationCatalog? localizationCatalog = null)
    {
        ArgumentNullException.ThrowIfNull(project);
        ArgumentNullException.ThrowIfNull(remote);
        if (!remote.IsComplete)
        {
            throw new IncompleteRemoteSnapshotException();
        }

        if (project.AppId != remote.AppId)
        {
            throw new InvalidOperationException("The remote snapshot belongs to a different Steam App ID.");
        }

        List<SyncEntityStatus> statuses = [];
        List<SyncWarning> warnings = [];
        List<SyncOperation> operations = [];

        Dictionary<StatId, SyncOperationId> statOperations =
            PlanStats(project, remote, localizationCatalog, statuses, warnings, operations);
        PlanAchievements(project, remote, localizationCatalog, statOperations, statuses, warnings, operations, recreateRenamedAchievements);

        foreach (RemoteStat stat in remote.Stats.Where(remoteStat =>
                     project.Stats.All(local => local.ApiName != remoteStat.ApiName)))
        {
            RemoteTombstone? tombstone = project.Tombstones.FirstOrDefault(
                value => value.EntityType == ProjectEntityType.Stat && value.ApiName == stat.ApiName);
            SyncState state = tombstone is null ? SyncState.SteamOnly : SyncState.PendingDeletion;
            statuses.Add(new(ProjectEntityType.Stat, tombstone?.LocalId, stat.ApiName, state, null,
                stat.ContentFingerprint, null));
            if (state == SyncState.SteamOnly)
            {
                operations.Add(new ImportRemote(
                    OperationId("import-stat", stat.ApiName.Value, stat.ContentFingerprint),
                    ProjectEntityType.Stat,
                    stat.ApiName.Value,
                    stat.ContentFingerprint));
            }
        }

        foreach (RemoteAchievement achievement in remote.Achievements.Where(remoteAchievement =>
                     project.Achievements.All(local => local.ApiName != remoteAchievement.ApiName)))
        {
            RemoteTombstone? tombstone = project.Tombstones.FirstOrDefault(
                value => value.EntityType == ProjectEntityType.Achievement && value.ApiName == achievement.ApiName);
            SyncState state = SyncState.PendingDeletion;
            statuses.Add(new(ProjectEntityType.Achievement, tombstone?.LocalId, achievement.ApiName, state, null,
                achievement.ContentFingerprint, null));
            AchievementDefinition? replacement = recreateRenamedAchievements ? project.Achievements.FirstOrDefault(local =>
                project.SyncBaselines.Any(baseline => baseline.EntityType == ProjectEntityType.Achievement &&
                    baseline.LocalId == local.Id.Value && baseline.ApiName == achievement.ApiName && local.ApiName != baseline.ApiName)) : null;
            operations.Add(new DeleteAchievement(
                OperationId("delete-achievement", achievement.ApiName.Value, achievement.ContentFingerprint),
                achievement.ApiName)
            {
                DependsOn = replacement is null ? new HashSet<SyncOperationId>() : operations
                    .Where(operation => operation.TargetKey == replacement.ApiName.Value && operation is not ImportRemote)
                    .Select(static operation => operation.Id).ToHashSet(),
                Targets = [new(ProjectEntityType.Achievement, achievement.ApiName.Value, null,
                    achievement.ContentFingerprint, SyncFingerprint.Absent)],
            });
            if (tombstone is null)
                operations.Add(new ImportRemote(
                    OperationId("import-achievement", achievement.ApiName.Value, achievement.ContentFingerprint),
                    ProjectEntityType.Achievement, achievement.ApiName.Value, achievement.ContentFingerprint));
        }

        List<SyncOperation> ordered = Order(operations);
        SyncOperation[] automaticallyIncluded = ordered.Where(static operation => operation.Included).ToArray();
        if (automaticallyIncluded.Length > 0)
        {
            HashSet<SyncOperationId> dependencies =
                automaticallyIncluded.Select(static operation => operation.Id).ToHashSet();
            VerifyTargets verify = new(OperationId("verify", string.Join("|", ordered.Select(static value => value.Id.Value))), dependencies);
            ordered = [.. ordered, verify];
        }

        return new SyncPlan(project.AppId, statuses, ordered, warnings);
    }

    private static Dictionary<StatId, SyncOperationId> PlanStats(
        AchimeldProject project,
        RemoteSnapshot remote,
        ProjectLocalizationCatalog? localizationCatalog,
        List<SyncEntityStatus> statuses,
        List<SyncWarning> warnings,
        List<SyncOperation> operations)
    {
        Dictionary<StatId, SyncOperationId> result = [];
        HashSet<StatId> referencedStats = project.Achievements
            .Where(static achievement => achievement.ProgressStatId is not null)
            .Select(static achievement => achievement.ProgressStatId!.Value)
            .ToHashSet();
        foreach (StatDefinition stat in project.Stats)
        {
            FieldSyncBaseline? baseline = FindBaseline(project, ProjectEntityType.Stat, stat.Id.Value);
            RemoteStat? remoteStat = remote.Stats.FirstOrDefault(value => value.ApiName == stat.ApiName);
            CanonicalFingerprintResult local = CanonicalFingerprint.For(stat, remote.SupportsDisplayOrder);
            bool renamed = WarnIfRenamed(stat.Id.Value, stat.ApiName, baseline, ProjectEntityType.Stat, warnings);
            SyncState state = renamed
                ? SyncState.Conflict
                : ClassifyDefinition(local, remoteStat is null ? null : CanonicalFingerprint.For(remoteStat, remote.SupportsDisplayOrder), baseline, remote.SupportsDisplayOrder);
            statuses.Add(new(ProjectEntityType.Stat, stat.Id.Value, stat.ApiName, state, local.EntityFingerprint,
                remoteStat?.ContentFingerprint, BaselineFingerprint(baseline)));

            if (state is SyncState.LocalOnly or SyncState.LocalChanged)
            {
                SyncOperationId id = OperationId("upsert-stat", stat.ApiName.Value, local.EntityFingerprint);
                operations.Add(new UpsertStat(id, stat, referencedStats.Contains(stat.Id))
                {
                    Targets =
                    [
                        new(
                            ProjectEntityType.Stat,
                            stat.ApiName.Value,
                            null,
                            remoteStat?.ContentFingerprint ?? SyncFingerprint.Absent,
                            local.EntityFingerprint),
                    ],
                });
                result.Add(stat.Id, id);
                if (localizationCatalog is not null)
                {
                    ProjectLocalizationRow[] rows = localizationCatalog.Rows
                        .Where(row => row.Id == stat.DisplayNameLocalizationId)
                        .ToArray();
                    operations.Add(new UpsertLocalization(
                        OperationId("localization-stat", stat.ApiName.Value, RowsFingerprint(rows)),
                        ProjectEntityType.Stat,
                        stat.ApiName.Value,
                        rows,
                        new HashSet<SyncOperationId> { id })
                    {
                        Targets = LocalizationTargets(
                            ProjectEntityType.Stat, stat.ApiName, rows, remote),
                    });
                }
            }
            else if (state == SyncState.InSync && localizationCatalog is not null)
            {
                ProjectLocalizationRow[] rows = localizationCatalog.Rows
                    .Where(row => row.Id == stat.DisplayNameLocalizationId).ToArray();
                SyncTargetFingerprint[] targets = LocalizationTargets(ProjectEntityType.Stat, stat.ApiName, rows, remote);
                if (targets.Any(static target => target.ExpectedOldFingerprint != target.DesiredFingerprint))
                    operations.Add(new UpsertLocalization(
                        OperationId("localization-stat", stat.ApiName.Value, RowsFingerprint(rows)),
                        ProjectEntityType.Stat, stat.ApiName.Value, rows, new HashSet<SyncOperationId>())
                    { Targets = targets });
            }
            else if (state == SyncState.SteamChanged && remoteStat is not null)
            {
                operations.Add(new ImportRemote(OperationId("import-stat", stat.ApiName.Value, remoteStat.ContentFingerprint),
                    ProjectEntityType.Stat, stat.ApiName.Value, remoteStat.ContentFingerprint));
            }
        }

        return result;
    }

    private static void PlanAchievements(
        AchimeldProject project,
        RemoteSnapshot remote,
        ProjectLocalizationCatalog? localizationCatalog,
        Dictionary<StatId, SyncOperationId> statOperations,
        List<SyncEntityStatus> statuses,
        List<SyncWarning> warnings,
        List<SyncOperation> operations,
        bool recreateRenamedAchievements)
    {
        Dictionary<StatId, SteamApiName> statNames = project.Stats.ToDictionary(static stat => stat.Id, static stat => stat.ApiName);
        foreach (AchievementDefinition achievement in project.Achievements)
        {
            FieldSyncBaseline? baseline = FindBaseline(project, ProjectEntityType.Achievement, achievement.Id.Value);
            RemoteAchievement? remoteAchievement = remote.Achievements.FirstOrDefault(value => value.ApiName == achievement.ApiName);
            SteamApiName? progressName = achievement.ProgressStatId is StatId id && statNames.TryGetValue(id, out SteamApiName name)
                ? name
                : null;
            CanonicalFingerprintResult local = CanonicalFingerprint.For(achievement, progressName, remote.SupportsDisplayOrder);
            bool renamed = WarnIfRenamed(achievement.Id.Value, achievement.ApiName, baseline,
                ProjectEntityType.Achievement, warnings);
            if (renamed && recreateRenamedAchievements && baseline is not null)
            {
                RemoteAchievement? previous = remote.Achievements.FirstOrDefault(item => item.ApiName == baseline.ApiName);
                bool previousUnchanged = previous is null || (remote.SupportsDisplayOrder
                    ? previous.ContentFingerprint == BaselineFingerprint(baseline)
                    : FieldsMatchBaseline(CanonicalFingerprint.For(previous, false), baseline));
                bool replacementMatches = remoteAchievement is null ||
                    CanonicalFingerprint.For(remoteAchievement, remote.SupportsDisplayOrder).EntityFingerprint == local.EntityFingerprint;
                // A prior attempt may already have created the replacement. Finish
                // only if both the replacement and original still match the plan.
                if (previousUnchanged && replacementMatches && (previous is not null || remoteAchievement is not null))
                    renamed = false;
            }
            SyncState state = renamed
                ? SyncState.Conflict
                : ClassifyDefinition(local, remoteAchievement is null ? null : CanonicalFingerprint.For(remoteAchievement, remote.SupportsDisplayOrder), baseline, remote.SupportsDisplayOrder);
            statuses.Add(new(ProjectEntityType.Achievement, achievement.Id.Value, achievement.ApiName, state,
                local.EntityFingerprint, remoteAchievement?.ContentFingerprint, BaselineFingerprint(baseline)));

            if (state is SyncState.LocalOnly or SyncState.LocalChanged)
            {
                HashSet<SyncOperationId> dependencies = [];
                if (achievement.ProgressStatId is StatId statId && statOperations.TryGetValue(statId, out SyncOperationId statOperation))
                {
                    dependencies.Add(statOperation);
                }

                SyncOperationId achievementOperation =
                    OperationId("upsert-achievement", achievement.ApiName.Value, local.EntityFingerprint);
                operations.Add(new UpsertAchievement(achievementOperation, achievement, dependencies)
                {
                    Targets =
                    [
                        new(
                            ProjectEntityType.Achievement,
                            achievement.ApiName.Value,
                            null,
                            remoteAchievement?.ContentFingerprint ?? SyncFingerprint.Absent,
                            local.EntityFingerprint),
                    ],
                });
                AddAchievementDependents(
                    project, remote, achievement, localizationCatalog, achievementOperation, operations);
            }
            else if (state == SyncState.InSync)
            {
                AddAchievementDependents(project, remote, achievement, localizationCatalog, null, operations);
            }
            else if (state == SyncState.SteamChanged && remoteAchievement is not null)
            {
                operations.Add(new ImportRemote(OperationId("import-achievement", achievement.ApiName.Value, remoteAchievement.ContentFingerprint),
                    ProjectEntityType.Achievement, achievement.ApiName.Value, remoteAchievement.ContentFingerprint));
            }
        }
    }

    private static void AddAchievementDependents(
        AchimeldProject project,
        RemoteSnapshot remote,
        AchievementDefinition achievement,
        ProjectLocalizationCatalog? catalog,
        SyncOperationId? achievementOperation,
        List<SyncOperation> operations)
    {
        HashSet<SyncOperationId> dependency = achievementOperation is SyncOperationId id ? [id] : [];
        if (catalog is not null)
        {
            LocalizationId[] ids =
            [
                achievement.DisplayNameLocalizationId,
                achievement.DescriptionLocalizationId,
            ];
            ProjectLocalizationRow[] rows = catalog.Rows.Where(row => ids.Contains(row.Id))
                .Select(row => AchievementText.Resolve(row, achievement.UnlockValue)).ToArray();
            operations.Add(new UpsertLocalization(
                OperationId("localization-achievement", achievement.ApiName.Value, RowsFingerprint(rows)),
                ProjectEntityType.Achievement,
                achievement.ApiName.Value,
                rows,
                dependency)
            {
                Targets = LocalizationTargets(
                    ProjectEntityType.Achievement, achievement.ApiName, rows, remote),
            });
        }

        List<CanonicalField> iconFields =
        [
            new("iconKind", achievement.IconKind),
            new("atlasId", achievement.AtlasId?.Value),
            new("standaloneImageId", achievement.StandaloneImageId?.Value),
            new("generalImageId", achievement.GeneralImageId?.Value),
            new("generalAtlasId", achievement.GeneralAtlasId?.Value),
            new("generalTileIndex", achievement.GeneralTileIndex),
            new("generalImages", achievement.GeneralImageEntries is null ? null :
                string.Join(";", achievement.GeneralImageEntries.Select(entry => $"{entry.Id}:{entry.ImageId?.Value}:{entry.AtlasId?.Value}:{entry.TileIndex}"))),
            new("atlasPath", ResolveIconPath(project, achievement)),
            new("tileIndex", achievement.TileIndex),
            new("tileWidth", project.Atlas.TileWidth),
            new("tileHeight", project.Atlas.TileHeight),
            new("outputSize", project.Atlas.OutputSize),
            new("outputWidth", project.Atlas.EffectiveOutputWidth),
            new("outputHeight", project.Atlas.EffectiveOutputHeight),
            new("unachievedMode", project.Atlas.EffectiveUnachievedIcon.Mode),
            new("unachievedOpacity", project.Atlas.EffectiveUnachievedIcon.Opacity),
            new("unachievedTint", project.Atlas.EffectiveUnachievedIcon.TintHex),
            new("unachievedImageId", achievement.UnachievedImageId?.Value),
            new("unachievedImagePath", achievement.UnachievedImageId is ImageAssetId lockedImage
                ? project.ResolveStandaloneImage(lockedImage)?.Image.Path : null),
        ];
        if (achievement.Icon is not null)
        {
            iconFields.Add(new("iconRecipe", FormatIconRecipe(project, achievement)));
        }

        string desiredIcon = CanonicalFingerprint.Compute(iconFields).EntityFingerprint;
        RemoteIconMetadata? remoteIcon = remote.Icons.FirstOrDefault(
            value => value.AchievementApiName == achievement.ApiName);
        operations.Add(new UploadIconPair(
            OperationId(
                "icons",
                achievement.ApiName.Value,
                desiredIcon),
            achievement,
            dependency)
        {
            Targets =
            [
                new(
                    ProjectEntityType.Achievement,
                    achievement.ApiName.Value,
                    null,
                    remoteIcon?.ContentFingerprint ?? SyncFingerprint.Absent,
                    desiredIcon),
            ],
        });
    }

    private static SyncTargetFingerprint[] LocalizationTargets(
        ProjectEntityType entityType,
        SteamApiName apiName,
        IEnumerable<ProjectLocalizationRow> rows,
        RemoteSnapshot remote) =>
        rows.Select(row =>
        {
            LocalizationField field = row.Owner?.Field ??
                throw new InvalidOperationException("A synchronization localization row requires an owner.");
            RemoteLocalization? current = remote.Localizations.FirstOrDefault(value =>
                value.EntityType == entityType &&
                value.ApiName == apiName &&
                value.Field == field);
            return new SyncTargetFingerprint(
                entityType,
                apiName.Value,
                field,
                current?.ContentFingerprint ?? SyncFingerprint.Absent,
                CanonicalFingerprint.For(row).EntityFingerprint);
        }).OrderBy(static target => target.LocalizationField).ToArray();

    private static bool FieldsMatchBaseline(CanonicalFingerprintResult fingerprint, FieldSyncBaseline baseline) =>
        fingerprint.FieldFingerprints.Where(static field => field.Key != "displayOrder")
            .All(field => baseline.FieldFingerprints.GetValueOrDefault(field.Key) == field.Value);

    private static SyncState ClassifyDefinition(CanonicalFingerprintResult local,
        CanonicalFingerprintResult? remote, FieldSyncBaseline? baseline, bool supportsDisplayOrder)
    {
        if (supportsDisplayOrder || baseline is null || remote is null || local.EntityFingerprint == remote.EntityFingerprint)
            return Classify(local.EntityFingerprint, remote?.EntityFingerprint, BaselineFingerprint(baseline));
        // Older saved baselines included local layout order. Compare the Steam fields
        // individually so a migration or an unrelated Steam row cannot look like an edit.
        return (!FieldsMatchBaseline(local, baseline), !FieldsMatchBaseline(remote, baseline)) switch
        {
            (false, false) => SyncState.InSync,
            (true, false) => SyncState.LocalChanged,
            (false, true) => SyncState.SteamChanged,
            _ => SyncState.Conflict,
        };
    }

    public static SyncState Classify(string? local, string? remote, string? baseline)
    {
        if (local is null)
        {
            return remote is null ? SyncState.InSync : SyncState.SteamOnly;
        }

        if (remote is null)
        {
            return baseline is null ? SyncState.LocalOnly : SyncState.LocalChanged;
        }

        if (string.Equals(local, remote, StringComparison.Ordinal))
        {
            return SyncState.InSync;
        }

        if (baseline is null)
        {
            return SyncState.Conflict;
        }

        bool localChanged = !string.Equals(local, baseline, StringComparison.Ordinal);
        bool remoteChanged = !string.Equals(remote, baseline, StringComparison.Ordinal);
        return (localChanged, remoteChanged) switch
        {
            (false, true) => SyncState.SteamChanged,
            (true, false) => SyncState.LocalChanged,
            (true, true) => SyncState.Conflict,
            _ => SyncState.Conflict,
        };
    }

    private static bool WarnIfRenamed(
        Guid localId,
        SteamApiName apiName,
        FieldSyncBaseline? baseline,
        ProjectEntityType entityType,
        List<SyncWarning> warnings)
    {
        if (baseline is null || baseline.ApiName == apiName)
        {
            return false;
        }

        warnings.Add(new SyncWarning(
            "sync.apiName.rename",
            $"Changing API name '{baseline.ApiName}' to '{apiName}' requires remote recreation. Synchronize can recreate an unchanged achievement draft.",
            entityType,
            localId));
        return true;
    }

    private static FieldSyncBaseline? FindBaseline(AchimeldProject project, ProjectEntityType type, Guid id) =>
        project.SyncBaselines.FirstOrDefault(value => value.EntityType == type && value.LocalId == id);

    private static string? BaselineFingerprint(FieldSyncBaseline? baseline)
    {
        if (baseline is null)
        {
            return null;
        }

        return baseline.EntityFingerprint ??
            CanonicalFingerprint.Compute(
                baseline.FieldFingerprints.Select(static pair => new CanonicalField(pair.Key, pair.Value)))
            .EntityFingerprint;
    }

    private static string RowsFingerprint(IEnumerable<ProjectLocalizationRow> rows) =>
        CanonicalFingerprint.Compute(rows.Select(
            row => new CanonicalField(row.Id.Value, CanonicalFingerprint.For(row).EntityFingerprint)))
        .EntityFingerprint;

    private static SyncOperationId OperationId(string kind, params string[] identity)
    {
        byte[] hash = SHA256.HashData(Encoding.UTF8.GetBytes(kind + "\n" + string.Join("\n", identity)));
        return new SyncOperationId(Convert.ToHexStringLower(hash.AsSpan(0, 16)));
    }

    private static List<SyncOperation> Order(IEnumerable<SyncOperation> operations)
    {
        List<SyncOperation> remaining = operations
            .OrderBy(Priority)
            .ThenBy(static operation => operation.TargetKey, StringComparer.Ordinal)
            .ThenBy(static operation => operation.Id.Value, StringComparer.Ordinal)
            .ToList();
        List<SyncOperation> ordered = [];
        HashSet<SyncOperationId> emitted = [];

        while (remaining.Count > 0)
        {
            int index = remaining.FindIndex(operation => operation.DependsOn.All(emitted.Contains));
            if (index < 0)
            {
                throw new InvalidOperationException("Synchronization operation dependencies contain a cycle.");
            }

            SyncOperation next = remaining[index];
            remaining.RemoveAt(index);
            ordered.Add(next);
            emitted.Add(next.Id);
        }

        return ordered;
    }

    private static int Priority(SyncOperation operation) => operation switch
    {
        DeleteAchievement => -1,
        UpsertStat { IsRequiredByAchievement: true } => 0,
        UpsertStat => 1,
        ImportRemote { EntityType: ProjectEntityType.Stat } => 2,
        UpsertAchievement => 3,
        ImportRemote { EntityType: ProjectEntityType.Achievement } => 4,
        UpsertLocalization => 5,
        UploadIconPair => 6,
        VerifyTargets => 7,
        _ => throw new ArgumentOutOfRangeException(nameof(operation)),
    };

    private static string ResolveIconPath(AchimeldProject project, AchievementDefinition achievement)
    {
        if (achievement.Icon is null)
        {
            if (achievement.IconKind == AchievementIconKind.StandaloneImage)
            {
                StandaloneImageDefinition? image = project.ResolveStandaloneImage(achievement);
                return image is null ? string.Empty : $"{image.Image.Kind}:{image.Image.Path}";
            }

            AtlasDefinition atlas = project.Assets.EffectiveAtlases.FirstOrDefault(
                value => achievement.AtlasId is null || value.Id == achievement.AtlasId)
                ?? project.Assets.EffectiveAtlases[0];
            return $"{atlas.Image.Kind}:{atlas.Image.Path}:{atlas.Columns}x{atlas.Rows}";
        }

        return FormatIconRecipe(project, achievement);
    }

    private static string FormatIconRecipe(AchimeldProject project, AchievementDefinition achievement)
    {
        IconRecipe recipe = achievement.ResolvedIcon;
        if (recipe.IsSimpleLegacy(achievement.TileIndex, achievement.AtlasId, achievement.StandaloneImageId))
        {
            return string.Empty;
        }

        IEnumerable<string> layers = recipe.Layers.Select(layer => layer.Kind switch
        {
            IconLayerKind.AtlasTile => FormatAtlasLayer(project, layer),
            IconLayerKind.StandaloneImage => FormatImageLayer(project, layer),
            IconLayerKind.SteamImportedArtwork => FormatSteamArtworkLayer(project, layer),
            IconLayerKind.AchievementImage => FormatImageLayer(project, layer),
            IconLayerKind.Text => FormatTextLayer(layer),
            IconLayerKind.Text3d => $"text3d:{(layer.Text3d ?? new()).Fingerprint()}:{FormatTextLayer(layer)}",
            IconLayerKind.Artifact3d => $"artifact3d:{layer.Visible}:{layer.BlendMode}:{(layer.Artifact3d ?? new()).Fingerprint()}{FormatTransform(layer)}",
            IconLayerKind.Solid => FormatSolidLayer(layer),
            IconLayerKind.Shape => $"shape:{layer.Visible}:{layer.BlendMode}:{(layer.Shape ?? new()).Fingerprint()}{FormatTransform(layer)}",
            IconLayerKind.Gradient => FormatGradientLayer(layer),
            IconLayerKind.PerlinNoise => $"perlinNoise:{layer.Visible}:{layer.BlendMode}:{(layer.PerlinNoise ?? new()).Fingerprint()}{FormatTransform(layer)}",
            IconLayerKind.SdfVignette => $"sdfVignette:{layer.Visible}:{layer.BlendMode}:{(layer.SdfVignette ?? new()).Fingerprint()}{FormatTransform(layer)}",
            IconLayerKind.Particle => $"particles:{layer.Visible}:{layer.BlendMode}:{(layer.Particles ?? new()).Fingerprint()}{FormatTransform(layer)}",
            IconLayerKind.Explosion => $"explosion:{layer.Visible}:{layer.BlendMode}:{(layer.Explosion ?? new()).Fingerprint()}{FormatTransform(layer)}",
            IconLayerKind.Curves => $"curves:{layer.Visible}:{layer.Curves?.Fingerprint()}{FormatTransform(layer)}",
            IconLayerKind.Contrast or IconLayerKind.HueSaturation or IconLayerKind.SelectiveColor or IconLayerKind.GradientOverlay or IconLayerKind.UnsharpMask =>
                $"adjustment:{layer.Kind}:{layer.Visible}:{layer.BlendMode}:{layer.Adjustment?.Fingerprint()}:{layer.Gradient?.Fingerprint()}{FormatTransform(layer)}",
            IconLayerKind.Folder => $"folder:{layer.Id}:{layer.Visible}:{layer.BlendMode}:{layer.MaskLayerId}{FormatTransform(layer)}",
            IconLayerKind.MaskReference => $"maskReference:{layer.MaskSourceLayerId}:{layer.Visible}:{layer.BlendMode}{FormatTransform(layer)}",
            _ => layer.Kind.ToString(),
        });
        return $"{recipe.BackgroundColorHex}|{string.Join(";", layers)}";
    }

    private static string FormatAtlasLayer(AchimeldProject project, IconLayer layer)
    {
        string transform = FormatTransform(layer);
        try
        {
            AtlasDefinition atlas = project.ResolveAtlas(layer.AtlasId);
            return $"atlas:{layer.Visible}:{layer.BlendMode}:{atlas.Image.Kind}:{atlas.Image.Path}:{atlas.Columns}x{atlas.Rows}:{layer.TileIndex}{transform}";
        }
        catch (InvalidOperationException)
        {
            return $"atlas:{layer.Visible}:{layer.BlendMode}:{layer.AtlasId?.Value:D}:{layer.TileIndex}{transform}";
        }
    }

    private static string FormatImageLayer(AchimeldProject project, IconLayer layer)
    {
        if (layer.ImageId is not ImageAssetId imageId)
        {
            return $"image:{layer.Visible}:{layer.BlendMode}:{FormatTransform(layer)}";
        }

        StandaloneImageDefinition? image = project.ResolveStandaloneImage(imageId);
        return image is null
            ? $"image:{layer.Visible}:{layer.BlendMode}:{imageId.Value:D}{FormatTransform(layer)}"
            : $"image:{layer.Visible}:{layer.BlendMode}:{image.Image.Kind}:{image.Image.Path}{FormatTransform(layer)}";
    }

    private static string FormatSteamArtworkLayer(AchimeldProject project, IconLayer layer) =>
        "steam-" + FormatImageLayer(project, layer);

    private static string FormatTextLayer(IconLayer layer)
    {
        IconTextStyle text = layer.Text ?? IconTextStyle.Default();
        return string.Join(
            ":",
            "text",
            layer.Visible,
            layer.BlendMode,
            text.Content,
            text.FontFamily,
            text.FontSize.ToString(CultureInfo.InvariantCulture),
            text.ColorHex,
            text.OutlineEnabled,
            text.OutlineColorHex,
            text.OutlineWidth.ToString(CultureInfo.InvariantCulture),
            text.Alignment,
            text.OutlinePlacement) + FormatTransform(layer);
    }

    private static string FormatSolidLayer(IconLayer layer) =>
        $"solid:{layer.Visible}:{layer.BlendMode}:{layer.SolidColorHex}{FormatTransform(layer)}";

    private static string FormatGradientLayer(IconLayer layer)
    {
        IconGradientStyle gradient = layer.Gradient ?? IconGradientStyle.Default();
        return $"gradient:{layer.Visible}:{layer.BlendMode}:{gradient.Fingerprint()}{FormatTransform(layer)}";
    }

    private static string FormatTransform(IconLayer layer)
    {
        string result = layer.Scale == 1
            ? string.Empty
            : $":scale={layer.Scale.ToString("G17", CultureInfo.InvariantCulture)}";
        if (layer.ParentFolderId is { } parent) result += $":folder={parent}";
        // Preserve legacy alpha-folder fingerprints; other masks previously had
        // no identity even though the compositor could apply them.
        if (layer.MaskLayerId is { } mask && layer.Kind != IconLayerKind.Folder) result += $":mask={mask}";
        if (layer.MaskLayerId is not null && (layer.MaskMode != IconMaskMode.Alpha || layer.MaskInvert))
            result += $":maskOptions={layer.MaskMode},{layer.MaskInvert}";
        if (layer.Rotation != 0) result += $":rotation={layer.Rotation.ToString("G17", CultureInfo.InvariantCulture)}";
        if (layer.OffsetX != 0 || layer.OffsetY != 0)
        {
            result += $":offset={layer.OffsetX.ToString("G17", CultureInfo.InvariantCulture)},{layer.OffsetY.ToString("G17", CultureInfo.InvariantCulture)}";
        }
        if (layer.Opacity != 1)
        {
            result += $":opacity={layer.Opacity.ToString("G17", CultureInfo.InvariantCulture)}";
        }
        foreach (IconLayerEffect effect in layer.EffectiveEffects)
        {
            result += $":effect={effect.Id.Value:D},{effect.Kind},{effect.Visible},{effect.ColorHex}," +
                $"{effect.Width.ToString("G17", CultureInfo.InvariantCulture)},{effect.Placement},{effect.OutlineShape}," +
                $"{effect.OffsetX.ToString("G17", CultureInfo.InvariantCulture)}," +
                $"{effect.OffsetY.ToString("G17", CultureInfo.InvariantCulture)}," +
                effect.Blur.ToString("G17", CultureInfo.InvariantCulture);
            if (effect.Kind == IconLayerEffectKind.GradientOverlay)
                result += $":gradient={effect.Gradient?.Fingerprint()}";
            if (effect.Kind == IconLayerEffectKind.Skew)
                result += FormattableString.Invariant($":skew={effect.SkewX:G17},{effect.SkewY:G17}");
            if (effect.Kind == IconLayerEffectKind.Perspective)
                result += FormattableString.Invariant($":perspective={effect.PerspectiveX:G17},{effect.PerspectiveY:G17}");
            if (effect.Kind == IconLayerEffectKind.Pinch)
                result += FormattableString.Invariant($":pinch={effect.Amount:G17}");
            if (effect.Kind == IconLayerEffectKind.LensBlur)
                result += FormattableString.Invariant($":lens={effect.Aberration:G17}");
            if (effect.Kind == IconLayerEffectKind.InnerLight)
                result += $":inner={effect.BlurEnabled},{effect.BlurColorHex},{effect.BlurBlendMode},{effect.BlurOpacity.ToString("G17", CultureInfo.InvariantCulture)}";
            if (effect.Kind is IconLayerEffectKind.InnerLight or IconLayerEffectKind.BoxBlur)
                result += $":alpha={effect.AffectOutline},{effect.Cutout.ToString("G17", CultureInfo.InvariantCulture)}";
            if (effect.BlendMode != IconBlendMode.Normal || effect.Opacity != 1)
                result += $":overlay={effect.BlendMode},{effect.Opacity.ToString("G17", CultureInfo.InvariantCulture)}";
        }
        return result;
    }
}
