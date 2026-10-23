using System.Globalization;
using System.Text.Json;
using System.Text.Json.Nodes;
using Achimeld.Browser;
using Achimeld.Core;
using Achimeld.Persistence;

internal static class AchievementTextFixtures
{
    private static readonly string[] Languages = ["English", "Polish", "ChineseSimplified", "ChineseTraditional", "Japanese", "Korean", "French"];
    private static readonly JsonSerializerOptions Json = new(JsonSerializerDefaults.Web);

    internal static async Task RunAsync()
    {
        CultureInfo previous = CultureInfo.CurrentCulture;
        try
        {
            CultureInfo.CurrentCulture = CultureInfo.GetCultureInfo("pl-PL");
            foreach ((decimal value, string expected) in new (decimal, string)[]
            {
                (5, "5"), (1.2300m, "1.23"), (0, "0"), (-0.000m, "0"),
                (0.0000000000000000000000000001m, "0.0000000000000000000000000001"),
                (decimal.MaxValue, "79228162514264337593543950335"),
            })
                Require(AchievementText.Resolve("{0}/{0} {1}", value) == expected + "/" + expected + " {1}", "Exact decimal token replacement failed.");
            Require(AchievementText.Resolve("Find {0} sticks", null) == "Find {0} sticks", "Missing values must preserve templates.");
            Require(AchievementText.Resolve("{1} {0:000}", 5) == "{1} {0:000}", "Only the exact token is supported.");

            AchievementDefinition achievement = new(AchievementId.New(), new("FIND_STICKS"), new("Name01"), new("Desc01"), false,
                SetBy.GameClient, null, 5, 0, 0);
            AchimeldProject project = AchimeldProject.Create("Unlock text fixture", new(480),
                new(PathReference.RelativeToProject("atlas.png"), PathReference.RelativeToProject("project.csv")), new(512, 512))
                with { Achievements = [achievement] };
            ProjectLocalizationRow template = new(new("Name01"), "Context {0}", "Find {0} sticks", "Znajdź {0} patyków",
                Owner: new(ProjectEntityType.Achievement, achievement.Id.Value, LocalizationField.AchievementDisplayName));
            RemoteAchievement remoteAchievement = new(null, achievement.ApiName, false, SetBy.GameClient, null, null, 0,
                CanonicalFingerprint.For(achievement, null).EntityFingerprint);
            RemoteSnapshot snapshot = new(new(480), DateTimeOffset.UtcNow, "fixture", true, [], [remoteAchievement], [], []);
            SyncPlan plan = new SyncPlanner().CreatePlan(project, snapshot, new([template]));
            ProjectLocalizationRow planned = plan.Operations.OfType<UpsertLocalization>().Single().Rows.Single();
            Require(planned.English == "Find 5 sticks" && planned.Polish == "Znajdź 5 patyków" && planned.Context == "Context {0}", "Planner did not resolve localized achievement text.");
            Require(template.English == "Find {0} sticks" && planned.French == "", "Planner changed source templates or empty translations.");

            V1ProjectDto dto = new()
            {
                Format = AchimeldProject.FormatIdentifier, SchemaVersion = 1, ProjectId = project.Id.Value,
                DisplayName = project.DisplayName, SteamAppId = 480,
                Assets = new() { Atlas = new() { Kind = "RelativeToProject", Path = "atlas.png" }, LocalizationCatalog = new() { Kind = "RelativeToProject", Path = "project.csv" } },
                Atlas = new() { TileWidth = 512, TileHeight = 512, Traversal = "RowMajor", UnachievedIcon = new() { Mode = "Grayscale", Opacity = 1, TintHex = "#808080" } },
                Achievements = [new() { Id = achievement.Id.Value, ApiName = achievement.ApiName.Value, InternalName = achievement.ApiName.Value,
                    DisplayNameLocalizationId = "Name01", DescriptionLocalizationId = "Desc01", SetBy = "GameClient", UnlockValue = 5 }],
            };
            Dictionary<string, Dictionary<string, string>> catalog = new()
            {
                ["Name01"] = Languages.ToDictionary(l => l, l => l + " find {0} sticks"),
                ["Desc01"] = Languages.ToDictionary(l => l, l => l + " collect {0}, keep {0}. {1}"),
            };
            dto.Achievements[0].Icon = new() { Layers = [new() { Id = Guid.NewGuid(), Kind = "Solid", Effects = [
                new() { Id = Guid.NewGuid(), Kind = "Skew", SkewX = 24.5, SkewY = -10 },
                new() { Id = Guid.NewGuid(), Kind = "Perspective", PerspectiveX = 0.25, PerspectiveY = -0.125 },
                new() { Id = Guid.NewGuid(), Kind = "Pinch", Amount = 0.75 },
                new() { Id = Guid.NewGuid(), Kind = "LensBlur", Blur = 8, Aberration = 12 }] }] };
            dto.Achievements[0].Icon!.Layers.Add(new() {
                Id = Guid.NewGuid(), Kind = "Text3d", DisplayName = "3D text",
                Text = new() { Content = "3D", FontFamily = "Segoe UI", FontSize = 192, ColorHex = "#FFFFCC55", Alignment = "Center" },
                Text3d = new(Projection: "Orthographic", Extrusion: 90, RotationX: -20, RotationY: 110,
                    PositionZ: 75, Shading: "Glossy", Roughness: 0.8,
                    Push: -2.5, Emboss: 6, Output: "EmbossMaskMinusFace", ShadowStrength: 0.75, SmoothNormals: true,
                    Reflectivity: 0.8)
            });
            V1IconLayerDto noiseMask = new() {
                Id=Guid.NewGuid(), Kind="PerlinNoise", DisplayName="Noise mask",
                PerlinNoise=new(71,3.4,4,0.6,2.2,[true,false,true,false],true,
                    new(IconGradientKind.Linear,90,[new(0,"#00102030"),new(1,"#FFFFFFFF")])) };
            dto.Achievements[0].Icon!.Layers.Add(noiseMask);
            dto.Achievements[0].Icon!.Layers.Add(new() {
                Id = Guid.NewGuid(), Kind = "Artifact3d", DisplayName = "3D artifact",
                Artifact3d = new() {Output = "AlphaMask", Parts = [new() {Primitive = "Torus", SweepAngle = 180, Size = [120,24,180]}]}
            });
            dto.Achievements[0].Icon!.Layers[0].MaskLayerId=noiseMask.Id;
            dto.Achievements[0].Icon!.Layers[0].MaskMode="Luminance";
            dto.Achievements[0].Icon!.Layers[0].MaskInvert=true;
            FakePortal portal = new(snapshot);
            PrepareSyncRequest request = new(JsonSerializer.SerializeToElement(dto, Json), catalog, []);
            SynchronizationSession session = new(portal);
            SyncReview review = await session.PrepareAsync(request, 480);
            string key = "Achievement:FIND_STICKS";
            SyncReviewRow reviewed = review.Rows.Single(r => r.Key == key);
            string fields = JsonSerializer.Serialize(reviewed.Fields, Json);
            Require(fields.Contains("English find 5 sticks", StringComparison.Ordinal) && !fields.Contains("{0}", StringComparison.Ordinal), "Review must show resolved text.");
            await session.BeginAsync(new(review.Token, [new(key, "Upload")]));
            await session.StepAsync(new(review.Token, key));
            JsonNode saved = JsonSerializer.SerializeToNode(await session.FinishAsync(new(review.Token)), Json)!;
            Require(JsonNode.DeepEquals(saved["project"]!["achievements"]![0]!["icon"], JsonSerializer.SerializeToNode(dto.Achievements[0].Icon, Json)),
                "Synchronization failed to retain spatial effect recipes.");
            Require(portal.Writes.Count == 2, "Expected name and description uploads.");
            foreach (string language in Languages)
            {
                Require(portal.Writes[0].Fields[language] == language + " find 5 sticks", "Name upload retained a token.");
                Require(portal.Writes[1].Fields[language] == language + " collect 5, keep 5. {1}", "Description upload retained a token.");
                Require(saved["catalog"]!["Name01"]![language]!.GetValue<string>() == catalog["Name01"][language], "Synchronization overwrote the source template.");
            }
            PrepareSyncRequest AfterSave() => new(JsonSerializer.SerializeToElement(saved["project"], Json), catalog, []);
            SynchronizationSession unchanged = new(portal);
            Require((await unchanged.PrepareAsync(AfterSave(), 480)).Rows.Single(r => r.Key == key).State == "InSync", "Resolved text did not match its synchronization baseline.");
            saved["project"]!["achievements"]![0]!["unlockValue"] = 12.5m;
            SynchronizationSession changed = new(portal);
            SyncReview updated = await changed.PrepareAsync(AfterSave(), 480);
            Require(updated.Rows.Single(r => r.Key == key).State == "LocalChanged", "Changing the unlock value must schedule text updates.");
            await changed.BeginAsync(new(updated.Token, [new(key, "Upload")]));
            await changed.StepAsync(new(updated.Token, key));
            Require(portal.Writes[^2].Fields["English"] == "English find 12.5 sticks", "Updated numeric text was not uploaded.");
        }
        finally { CultureInfo.CurrentCulture = previous; }
        Console.WriteLine("Achievement unlock text, planner, review, upload and baseline fixtures passed.");
    }

    private static void Require(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }

    private sealed class FakePortal(RemoteSnapshot snapshot) : ISynchronizationPortal
    {
        private RemoteSnapshot current = snapshot;
        internal List<PortalWriteOperation> Writes { get; } = [];
        public Task<RemoteSnapshot> ReadAsync(SteamAppId appId) => Task.FromResult(current);
        public Task<byte[]> DownloadArtworkAsync(Uri uri) => throw new InvalidOperationException("Unexpected artwork download.");
        public Task<bool> VerifyIconsAsync(PortalWriteOperation operation, RemoteIconMetadata? actual) =>
            throw new InvalidOperationException("Unexpected icon verification.");
        public Task ApplyAsync(PortalWriteOperation operation)
        {
            Require(operation.Target.Kind == PortalEntityKind.Localization, "Unexpected definition or icon write.");
            RemoteLocalization? previous = current.Localizations.SingleOrDefault(l => l.Field == operation.Target.LocalizationField);
            Require(operation.ExpectedOldFingerprint == (previous?.ContentFingerprint ?? SyncFingerprint.Absent), "Localization pin did not match.");
            string Text(string language) => operation.Fields[language];
            ProjectLocalizationRow actual = new(new("Check1"), "", Text("English"), Text("Polish"), Text("ChineseSimplified"), Text("ChineseTraditional"), Text("Japanese"), Text("Korean"), Text("French"));
            string fingerprint = CanonicalFingerprint.For(actual).EntityFingerprint;
            Require(operation.DesiredFingerprint == fingerprint, "Fingerprint was not computed from the uploaded text.");
            RemoteLocalization remote = new(ProjectEntityType.Achievement, operation.Target.ApiName,
                operation.Target.LocalizationField!.Value, operation.Fields, fingerprint);
            current = current with { Localizations = current.Localizations.Where(l => l.Field != remote.Field).Append(remote).ToArray() };
            Writes.Add(operation);
            return Task.CompletedTask;
        }
    }
}
