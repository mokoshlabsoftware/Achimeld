using System.Globalization;
using System.Security.Cryptography;
using System.Text.Json;
using System.Text.Json.Nodes;
using Achimeld.Browser;
using Achimeld.Core;
using Achimeld.Security;

internal static class SynchronizationFixtures
{
    private static readonly Guid StatIdValue = Guid.Parse("11111111-1111-4111-8111-111111111111");
    private static readonly Guid AchievementIdValue = Guid.Parse("22222222-2222-4222-8222-222222222222");
    private static readonly byte[] Png = Convert.FromBase64String("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aL1kAAAAASUVORK5CYII=");

    internal static async Task RunAsync()
    {
        int checks = 0;
        void Check(bool value) { if (!value) throw new Exception("Synchronization fixture assertion failed."); checks++; }
        async Task Reject(Func<Task> action)
        {
            try { await action(); }
            catch (Exception e) when (e is InvalidDataException or ArgumentException or JsonException) { checks++; return; }
            throw new Exception("Unsafe synchronization accepted.");
        }
        FakePortal portal = new();
        // Renaming a local icon must never let its legacy achievement-shaped
        // record enter a review, even if a caller bypasses the host projection.
        foreach (string marker in new[] { "styleAchievement", "artImage" })
        {
            foreach (JsonNode? value in new JsonNode?[] { JsonValue.Create(true), JsonValue.Create("true"), JsonValue.Create(1), null })
            {
                var localRequest = Request(achievement: true);
                JsonObject localProject = JsonNode.Parse(localRequest.Project.GetRawText())!.AsObject();
                localProject["achievements"]![0]![marker] = value;
                localProject["achievements"]![0]!["internalName"] = "Renamed canvas icon";
                var localPortal = new FakePortal();
                await Reject(() => new SynchronizationSession(localPortal).PrepareAsync(localRequest with { Project = JsonSerializer.SerializeToElement(localProject) }, 480));
                Check(localPortal.Writes == 0 && localPortal.BannerAppId == 0);
            }
            var realRequest = Request(achievement: true);
            JsonObject realProject = JsonNode.Parse(realRequest.Project.GetRawText())!.AsObject();
            realProject["achievements"]![0]![marker] = false;
            var realReview = await new SynchronizationSession(new FakePortal()).PrepareAsync(realRequest with { Project = JsonSerializer.SerializeToElement(realProject) }, 480);
            Check(realReview.Rows.Any(row => row.Key == "Achievement:ACH_WIN"));
        }
        SynchronizationSession session = new(portal);
        SyncReview review = await session.PrepareAsync(Request(), 480);
        Check(review.GameBanner is null && portal.BannerAppId == 480);
        var bannerPortal = new FakePortal { Banner = Png };
        var bannerReview = await new SynchronizationSession(bannerPortal).PrepareAsync(Request(), 480);
        Check(bannerReview.GameBanner!.SequenceEqual(Png) && bannerPortal.Writes == 0 && bannerPortal.BannerAppId == 480);
        bannerPortal = new FakePortal { Banner = new byte[2 * 1024 * 1024 + 1] };
        Check((await new SynchronizationSession(bannerPortal).PrepareAsync(Request(), 480)).GameBanner is null);
        bannerPortal = new FakePortal { Banner = [1, 2, 3] };
        Check((await new SynchronizationSession(bannerPortal).PrepareAsync(Request(), 480)).GameBanner is null);
        // A retained banner stays an ordinary User image. Its optional local
        // marker must survive subsequent preparation and the raw project result.
        JsonObject bannerProject = JsonNode.Parse(Request().Project.GetRawText())!.AsObject();
        bannerProject["assets"]!["images"]!.AsArray().Add(new JsonObject {
            ["id"] = Guid.NewGuid(), ["kind"] = "User", ["displayName"] = "Game banner", ["steamBannerAppId"] = 480,
            ["image"] = new JsonObject { ["kind"] = "RelativeToProject", ["path"] = "game.art-data-bank/banner.png" },
        });
        var retainedBannerSession = new SynchronizationSession(new FakePortal());
        var retainedBannerReview = await retainedBannerSession.PrepareAsync(Request() with { Project = JsonSerializer.SerializeToElement(bannerProject) }, 480);
        await retainedBannerSession.BeginAsync(new(retainedBannerReview.Token, []));
        JsonElement retainedBannerResult = JsonSerializer.SerializeToElement(await retainedBannerSession.FinishAsync(new(retainedBannerReview.Token)), SecurityWire.Json);
        Check(retainedBannerResult.GetProperty("project").GetProperty("assets").GetProperty("images")[0].GetProperty("steamBannerAppId").GetUInt32() == 480);
        Check(review.Rows.Single().DefaultAction == "Upload" && portal.Writes == 0);
        await Reject(() => session.BeginAsync(new(Guid.NewGuid(), [new("Stat:SCORE", "Upload")])));
        await Reject(() => session.BeginAsync(new(review.Token, [new("Stat:SCORE", "Delete")])));
        await Reject(() => session.BeginAsync(new(review.Token, [new("Stat:OTHER", "Upload")])));
        await Reject(() => session.BeginAsync(new(review.Token, [new("Stat:SCORE", "Upload"), new("Stat:SCORE", "Upload")])));
        var steps = await session.BeginAsync(new(review.Token, [new("Stat:SCORE", "Upload")]));
        Check(steps.Length == 1 && portal.Writes == 0);
        await session.StepAsync(new(review.Token, steps[0].Key));
        Check(portal.Writes == 2 && portal.Snapshot.Stats.Single().Maximum == 100);
        JsonElement result = JsonSerializer.SerializeToElement(await session.FinishAsync(new(review.Token)), SecurityWire.Json);
        Check(result.GetProperty("project").GetProperty("syncBaselines").GetArrayLength() == 1);
        Check(result.GetProperty("project").GetProperty("lastRemoteSnapshot").GetProperty("isComplete").GetBoolean());
        Check(result.GetProperty("project").GetProperty("lastVerifiedAtUtc").GetDateTimeOffset().Offset == TimeSpan.Zero);
        await Reject(() => session.StepAsync(new(review.Token, steps[0].Key)));
        await Reject(() => session.FinishAsync(new(review.Token)));

        // A completed baseline makes a second, fresh review stable.
        var againRequest = Request() with { Project = result.GetProperty("project").Clone() };
        var again = await new SynchronizationSession(portal).PrepareAsync(againRequest, 480);
        Check(again.Rows.Single().State == "InSync" && again.Rows.Single().DefaultAction == "Skip");

        portal = new(); session = new(portal); review = await session.PrepareAsync(Request(), 480);
        portal.AddStat("OTHER");
        await Reject(() => session.BeginAsync(new(review.Token, [new("Stat:SCORE", "Upload")])));
        Check(portal.Writes == 0);

        portal = new(); session = new(portal); review = await session.PrepareAsync(Request(), 480);
        await session.BeginAsync(new(review.Token, [new("Stat:SCORE", "Upload")]));
        portal.AddStat("OTHER");
        await Reject(() => session.StepAsync(new(review.Token, "Stat:SCORE")));
        Check(portal.Writes == 0);
        await Reject(() => session.StepAsync(new(review.Token, "Stat:SCORE")));

        // Missing save acknowledgement/read-back consumes the step; it is never replayed.
        portal = new() { SuppressSave = true }; session = new(portal); review = await session.PrepareAsync(Request(), 480);
        await session.BeginAsync(new(review.Token, [new("Stat:SCORE", "Upload")]));
        await Reject(() => session.StepAsync(new(review.Token, "Stat:SCORE")));
        await Reject(() => session.StepAsync(new(review.Token, "Stat:SCORE")));
        Check(portal.Writes == 1);
        // Stat/achievement namespaces can share an API name. Creating a stat
        // must not exempt the existing achievement's text from unrelated checks.
        portal = new() { ChangeSameNameAchievementText = true }; portal.AddAchievement(); portal.AddAchievementText("ACH_WIN", "Before");
        var sameName = Request();
        JsonObject sameNameProject = JsonNode.Parse(sameName.Project.GetRawText())!.AsObject(); sameNameProject["stats"]![0]!["apiName"] = "ACH_WIN";
        session = new(portal); review = await session.PrepareAsync(sameName with { Project = JsonSerializer.SerializeToElement(sameNameProject) }, 480);
        await session.BeginAsync(new(review.Token, [new("Stat:ACH_WIN", "Upload")]));
        await Reject(() => session.StepAsync(new(review.Token, "Stat:ACH_WIN")));
        Check(portal.Writes == 1);
        // Review exposes dependency changes and text on Steam-only imports.
        portal = new(); portal.AddAchievement(); portal.AddAchievementText("ACH_WIN", "Remote winner");
        review = await new SynchronizationSession(portal).PrepareAsync(Request(localStat: false), 480);
        JsonElement fields = JsonSerializer.SerializeToElement(review.Rows.Single(r => r.Key == "Achievement:ACH_WIN").Fields, SecurityWire.Json);
        Check(fields.EnumerateArray().Any(f => f.GetProperty("field").GetString() == "progressStatApiName" && f.GetProperty("steam").GetString() == "SCORE"));
        Check(fields.EnumerateArray().Any(f => f.GetProperty("steam").GetString() == "Remote winner"));

        // A remote-only import creates local data without any portal write.
        portal = new(); portal.AddStat("REMOTE"); session = new(portal);
        review = await session.PrepareAsync(Request(localStat: false), 480);
        Check(review.Rows.Single().Actions.SequenceEqual(new[] { "Skip", "Import" }));
        Check(review.Rows.Single().DefaultAction == "Import");
        await session.BeginAsync(new(review.Token, [new("Stat:REMOTE", "Import")]));
        await session.StepAsync(new(review.Token, "Stat:REMOTE"));
        result = JsonSerializer.SerializeToElement(await session.FinishAsync(new(review.Token)), SecurityWire.Json);
        Check(portal.Writes == 0 && result.GetProperty("project").GetProperty("stats")[0].GetProperty("apiName").GetString() == "REMOTE");

        // Existing remote differences require an explicit direction.
        portal = new(); portal.AddStat("SCORE", 50); session = new(portal);
        review = await session.PrepareAsync(Request(), 480);
        Check(review.Rows.Single().State == "Conflict" && review.Rows.Single().DefaultAction == "Skip");

        // Dependency closure is checked at the receiver before any mutations.
        portal = new(); session = new(portal); review = await session.PrepareAsync(Request(achievement: true), 480);
        await Reject(() => session.BeginAsync(new(review.Token, [new("Achievement:ACH_WIN", "Upload")])));
        await Reject(() => session.StepAsync(new(review.Token, "Achievement:ACH_WIN")));
        Check(portal.Writes == 0);
        portal = new(); portal.AddStat("DUPLICATE"); portal.AddStat("duplicate");
        await Reject(() => new SynchronizationSession(portal).PrepareAsync(Request(), 480));
        Check(portal.Writes == 0);
        portal = new(); session = new(portal);
        JsonObject invalidNumeric = JsonNode.Parse(Request().Project.GetRawText())!.AsObject(); invalidNumeric["stats"]![0]!["maximum"] = 100.5m;
        review = await session.PrepareAsync(Request() with { Project = JsonSerializer.SerializeToElement(invalidNumeric) }, 480);
        await Reject(() => session.BeginAsync(new(review.Token, [new("Stat:SCORE", "Upload")])));
        await Reject(() => session.StepAsync(new(review.Token, "Stat:SCORE")));
        Check(portal.Writes == 0);
        var blank = Request(); blank.Catalog["Aa1111"]["English"] = "";
        review = await new SynchronizationSession(new FakePortal()).PrepareAsync(blank, 480);
        Check(review.Rows.Single().UploadError is not null && !review.Rows.Single().Actions.Contains("Upload"));
        portal = new(); portal.AddStat("Score");
        await Reject(() => new SynchronizationSession(portal).PrepareAsync(Request(), 480));
        Check(portal.Writes == 0);
        portal = new(); portal.AddAchievement(); session = new(portal);
        JsonObject tooSmall = JsonNode.Parse(Request().Project.GetRawText())!.AsObject(); tooSmall["stats"]![0]!["maximum"] = 5;
        review = await session.PrepareAsync(Request() with { Project = JsonSerializer.SerializeToElement(tooSmall) }, 480);
        await Reject(() => session.BeginAsync(new(review.Token, [new("Stat:SCORE", "Upload")])));
        Check(portal.Writes == 0);
        portal = new() { InjectUnrelatedChange = true }; session = new(portal); review = await session.PrepareAsync(Request(), 480);
        await session.BeginAsync(new(review.Token, [new("Stat:SCORE", "Upload")]));
        await Reject(() => session.StepAsync(new(review.Token, "Stat:SCORE")));
        await Reject(() => session.StepAsync(new(review.Token, "Stat:SCORE")));
        Check(portal.Writes == 1);
        portal = new(); portal.AddStat("SCORE");
        RemoteSnapshot corrupt = portal.Snapshot with { Stats = [portal.Snapshot.Stats[0] with { ContentFingerprint = new string('a', 64) }] };
        await Reject(() => Task.Run(() => SynchronizationSnapshotPolicy.Validate(corrupt, new(480))));

        // PNG bytes are pinned by both hashes and decoded dimensions before uploads.
        portal = new(); portal.AddAchievement(); session = new(portal);
        string hash = Convert.ToHexStringLower(SHA256.HashData(Png));
        var iconRequest = Request(achievement: true) with { Icons = new() { ["ACH_WIN"] = new(hash, hash) } };
        review = await session.PrepareAsync(iconRequest, 480);
        await session.BeginAsync(new(review.Token, [new("Icon:ACH_WIN", "Upload")]));
        await Reject(() => session.StepAsync(new(review.Token, "Icon:ACH_WIN", Png, new byte[24])));
        Check(portal.Writes == 0);

        // Exact upload/read-back and raw remote icon identity survive a fresh session.
        portal = new() { ConvertIcons = true }; portal.AddAchievement(); session = new(portal); review = await session.PrepareAsync(iconRequest, 480);
        await session.BeginAsync(new(review.Token, [new("Icon:ACH_WIN", "Upload")]));
        await session.StepAsync(new(review.Token, "Icon:ACH_WIN", Png, Png));
        result = JsonSerializer.SerializeToElement(await session.FinishAsync(new(review.Token)), SecurityWire.Json);
        Check(portal.Writes == 1 && portal.Verifications > 0);
        var stable = await new SynchronizationSession(portal).PrepareAsync(iconRequest with { Project = result.GetProperty("project").Clone() }, 480);
        Check(stable.Rows.Single(r => r.Kind == "Icon").State == "InSync");
        Check(result.GetProperty("project").GetProperty("achievements")[0].GetProperty("icon").GetProperty("layers")[0].GetProperty("kind").GetString() == "Shape");

        // Imports retain only bytes matching the exact reviewed raw image identities.
        portal = new(); portal.AddAchievement(withIcons: true); session = new(portal);
        review = await session.PrepareAsync(Request(localStat: false), 480);
        await session.BeginAsync(new(review.Token, [new("Stat:SCORE", "Import"), new("Achievement:ACH_WIN", "Import")]));
        await session.StepAsync(new(review.Token, "Stat:SCORE"));
        SyncStepResult imported = await session.StepAsync(new(review.Token, "Achievement:ACH_WIN"));
        Check(imported.Artwork.Length == 2 && imported.Artwork.All(a => a.Bytes.SequenceEqual(Png)));
        result = JsonSerializer.SerializeToElement(await session.FinishAsync(new(review.Token)), SecurityWire.Json);
        Check(result.GetProperty("project").GetProperty("assets").GetProperty("images").GetArrayLength() == 2 && portal.Writes == 0);
        Check(result.GetProperty("project").GetProperty("achievements")[0].GetProperty("progressStatId").GetGuid() == result.GetProperty("project").GetProperty("stats")[0].GetProperty("id").GetGuid());
        portal = new() { ChangeDownloadedArtwork = true }; portal.AddAchievement(withIcons: true); session = new(portal);
        review = await session.PrepareAsync(Request(localStat: false), 480);
        await session.BeginAsync(new(review.Token, [new("Stat:SCORE", "Import"), new("Achievement:ACH_WIN", "Import")]));
        await session.StepAsync(new(review.Token, "Stat:SCORE"));
        await Reject(() => session.StepAsync(new(review.Token, "Achievement:ACH_WIN")));
        await Reject(() => session.FinishAsync(new(review.Token)));
        Check(portal.Writes == 0);
        Check(BoundedRaster.HasSafeDimensions(Png, 256));
        byte[] bomb = (byte[])Png.Clone(); System.Buffers.Binary.BinaryPrimitives.WriteUInt32BigEndian(bomb.AsSpan(16, 4), 100_000);
        Check(!BoundedRaster.HasSafeDimensions(bomb, 4096));
        Check(!BoundedRaster.HasSafeDimensions(new byte[] { 0xff, 0xd8, 0xff, 0xe0, 0xff, 0xff }, 256));

        // A borrowed App ID, new URL/path field and mutation outside a reviewed
        // request scope cannot enter through the dedicated synchronization DTOs.
        await Reject(() => new SynchronizationSession(new FakePortal()).PrepareAsync(Request(), 481));
        await Reject(() => Task.Run(() => JsonSerializer.Deserialize<SyncStepRequest>("{\"token\":\"00000000-0000-0000-0000-000000000000\",\"key\":\"x\",\"path\":\"C:/outside.png\"}", SecurityWire.Json)));
        SteamBrowserRequestPolicy policy = new(); policy.PinApp(480); policy.EnterRead();
        Check(!policy.Allows("POST", new("https://partner.steamgames.com/apps/savestat/480"), "xhr", false));
        policy.BeginReviewedDraft(480, PortalEntityKind.Stat);
        Check(!policy.Allows("POST", new("https://partner.steamgames.com/apps/savestat/480"), "xhr", false));
        policy.ArmReviewedPost(480);
        Check(policy.Allows("POST", new("https://partner.steamgames.com/apps/savestat/480"), "xhr", false));
        Check(!policy.Allows("POST", new("https://partner.steamgames.com/apps/savestat/480"), "xhr", false));
        Check(!policy.Allows("POST", new("https://partner.steamgames.com/apps/savestat/481"), "xhr", false));
        Check(!policy.Allows("POST", new("https://partner.steamgames.com/images/uploadachievement"), "xhr", false));
        Check(!policy.Allows("POST", new("https://partner.steamgames.com/apps/deleteachievement/480"), "xhr", false));
        policy.EndReviewedDraft();
        Check(!policy.Allows("POST", new("https://partner.steamgames.com/apps/fetchachievement/480/9/0"), "xhr", false));
        foreach (PortalEntityKind kind in new[] { PortalEntityKind.Achievement, PortalEntityKind.Stat, PortalEntityKind.Icon })
        {
            policy.BeginReviewedDraft(480, kind);
            string route = kind == PortalEntityKind.Stat ? "fetchstat/480/9" : "fetchachievement/480/9/0";
            foreach (string method in new[] { "GET", "POST" }) Check(policy.Allows(method, new("https://partner.steamgames.com/apps/" + route), "xhr", false));
            foreach (string invalid in new[] { route.Replace("/480/", "/481/"), route + "?other=1", route + "/", route.Replace("/9", "/09"), route.Replace("/9", "/4294967296") })
                Check(!policy.Allows("POST", new("https://partner.steamgames.com/apps/" + invalid), "xhr", false));
            Check(!policy.Allows("POST", new("https://partner.steamgames.com/apps/" + route), "document", true));
            policy.EndReviewedDraft();
        }
        foreach (PortalEntityKind kind in new[] { PortalEntityKind.Achievement, PortalEntityKind.Stat })
        {
            string endpoint = kind == PortalEntityKind.Stat ? "newstat" : "newachievement";
            await Reject(() => Task.Run(() => policy.ArmReviewedCreation(480)));
            policy.BeginReviewedDraft(480, kind);
            Check(!policy.Allows("POST", new($"https://partner.steamgames.com/apps/{endpoint}/480"), "xhr", false));
            policy.ArmReviewedCreation(480);
            await Reject(() => Task.Run(() => policy.ArmReviewedCreation(480)));
            Check(!policy.Allows("POST", new($"https://partner.steamgames.com/apps/{endpoint}/481"), "xhr", false));
            Check(!policy.Allows("POST", new($"https://partner.steamgames.com/apps/{endpoint}/480?appid=481"), "xhr", false));
            Check(!policy.Allows("POST", new($"https://partner.steamgames.com/apps/{endpoint}/480"), "document", true));
            Check(policy.Allows("POST", new($"https://partner.steamgames.com/apps/{endpoint}/480"), "xhr", false));
            Check(!policy.Allows("POST", new($"https://partner.steamgames.com/apps/{endpoint}/480"), "xhr", false));
            policy.ArmReviewedPost(480);
            string save = kind == PortalEntityKind.Stat ? "savestat" : "saveachievement";
            Check(policy.Allows("POST", new($"https://partner.steamgames.com/apps/{save}/480"), "xhr", false));
            policy.EndReviewedDraft();
        }
        policy.BeginReviewedDraft(480, PortalEntityKind.Icon);
        await Reject(() => Task.Run(() => policy.ArmReviewedCreation(480)));
        policy.EndReviewedDraft();
        Check(!policy.Allows("POST", new("https://partner.steamgames.com/apps/savestat/480"), "xhr", false));
        policy.BeginReviewedDraft(480, PortalEntityKind.Icon);
        Check(!policy.Allows("POST", new("https://partner.steamgames.com/images/uploadachievement"), "document", true));
        policy.ArmReviewedPost(480);
        Check(policy.Allows("POST", new("https://partner.steamgames.com/images/uploadachievement"), "document", true));
        Check(!policy.Allows("POST", new("https://partner.steamgames.com/images/uploadachievement"), "document", true));
        Check(!policy.Allows("POST", new("https://partner.steamgames.com/apps/saveachievement/480"), "xhr", false));
        Check(!policy.Allows("GET", new("https://partner.steamgames.com/apps/stats/480"), "document", true));
        policy.EndReviewedDraft();
        Check(LivePortalIdentity.RecordId(JsonSerializer.SerializeToElement(new { stat_id = 12 }), "stat_id") == "12");
        Check(LivePortalIdentity.RecordId(JsonSerializer.SerializeToElement(new { bit_id = "0" }), "bit_id") == "0");
        Check(LivePortalIdentity.EditorPrefix("a12_0_apiname") == "a12_0");
        foreach (string unsafeId in new[] { "01", "-1", "1/2", "1 input", "1],input[type=file]", "4294967296" })
            await Reject(() => Task.Run(() => LivePortalIdentity.RecordId(JsonSerializer.SerializeToElement(new { stat_id = unsafeId }), "stat_id")));
        foreach (string unsafePrefix in new[] { "_apiname", "a[0]_apiname", "a, input_apiname", "123_apiname" })
            await Reject(() => Task.Run(() => LivePortalIdentity.EditorPrefix(unsafePrefix)));
        using (ByteArrayContent content = new(Png)) Check((await BoundedSteamContent.ReadAsync(content, Png.Length)).SequenceEqual(Png));
        using (ByteArrayContent content = new(Png)) await Reject(() => BoundedSteamContent.ReadAsync(content, Png.Length - 1));
        using (StreamContent content = new(new NonSeekable(new byte[65]))) await Reject(() => BoundedSteamContent.ReadAsync(content, 64));
        using (StreamContent content = new(new NonSeekable([], stall: true)))
        using (CancellationTokenSource deadline = new(TimeSpan.FromMilliseconds(50)))
        {
            bool cancelled = false;
            try { await BoundedSteamContent.ReadAsync(content, 64, deadline.Token); }
            catch (OperationCanceledException) when (deadline.IsCancellationRequested) { cancelled = true; }
            Check(cancelled);
        }
        // General images use nullable legacy tile selectors in current saved
        // projects. Only definition fields enter the legacy mapper; artwork
        // remains exact in the retained raw project across synchronization.
        var modernRequest = Request(achievement: true);
        JsonObject modern = JsonNode.Parse(modernRequest.Project.GetRawText())!.AsObject();
        modern["achievements"]![0]!["generalTileIndex"] = null;
        modern["achievements"]![0]!["generalImageEntries"] = new JsonArray(new JsonObject { ["id"] = 0, ["imageId"] = JsonValue.Create(Guid.NewGuid()), ["tileIndex"] = null });
        session = new(new FakePortal());
        review = await session.PrepareAsync(modernRequest with { Project = JsonSerializer.SerializeToElement(modern) }, 480);
        await session.BeginAsync(new(review.Token, []));
        result = JsonSerializer.SerializeToElement(await session.FinishAsync(new(review.Token)), SecurityWire.Json);
        Check(JsonNode.DeepEquals(modern["achievements"], JsonNode.Parse(result.GetProperty("project").GetProperty("achievements").GetRawText())));

        // Unsynced means ready for synchronization, including definition and
        // icon uploads. Importing a verified definition marks the card Active.
        portal = new(); portal.AddAchievement(withIcons: true); session = new(portal);
        var awaitingRequest = Request(achievement: true);
        JsonObject awaitingProject = JsonNode.Parse(awaitingRequest.Project.GetRawText())!.AsObject();
        awaitingProject["achievements"]![0]!["steamSyncStatus"] = "Unsynced";
        string awaitingHash = Convert.ToHexStringLower(SHA256.HashData(Png));
        review = await session.PrepareAsync(awaitingRequest with { Project = JsonSerializer.SerializeToElement(awaitingProject), Icons = new() { ["ACH_WIN"] = new(awaitingHash, awaitingHash) } }, 480);
        Check(review.Rows.Single(r => r.Key == "Achievement:ACH_WIN").Actions.Contains("Upload"));
        Check(review.Rows.Single(r => r.Key == "Icon:ACH_WIN").Actions.Contains("Upload"));
        await session.BeginAsync(new(review.Token, [new("Achievement:ACH_WIN", "Import")]));
        await session.StepAsync(new(review.Token, "Achievement:ACH_WIN"));
        result = JsonSerializer.SerializeToElement(await session.FinishAsync(new(review.Token)), SecurityWire.Json);
        Check(result.GetProperty("project").GetProperty("achievements")[0].GetProperty("steamSyncStatus").GetString() == "Active" && portal.Writes == 0);

        // Inactive cards are retained byte-for-byte and cannot be selected by
        // forging a definition or icon row, whether Steam has a matching name.
        foreach (string status in new[] { "Retired" })
        {
            foreach (bool present in new[] { false, true })
            {
                portal = new(); if (present) portal.AddAchievement(withIcons: true);
                var inactive = Request(achievement: true);
                JsonObject inactiveProject = JsonNode.Parse(inactive.Project.GetRawText())!.AsObject();
                inactiveProject["achievements"]![0]!["steamSyncStatus"] = status;
                session = new(portal);
                review = await session.PrepareAsync(inactive with { Project = JsonSerializer.SerializeToElement(inactiveProject) }, 480);
                Check(!review.Rows.Any(r => r.ApiName == "ACH_WIN"));
                if (present)
                {
                    var aliasPortal = new FakePortal(); aliasPortal.AddAchievement(withIcons: true); aliasPortal.RenameAchievement("ach_win");
                    var aliasReview = await new SynchronizationSession(aliasPortal).PrepareAsync(inactive with { Project = JsonSerializer.SerializeToElement(inactiveProject) }, 480);
                    Check(!aliasReview.Rows.Any(r => r.Kind is "Achievement" or "Icon"));
                }
                await Reject(() => session.BeginAsync(new(review.Token, [new("Achievement:ACH_WIN", "Upload")])));
                await Reject(() => session.BeginAsync(new(review.Token, [new("Icon:ACH_WIN", "Upload")])));
                await session.BeginAsync(new(review.Token, []));
                result = JsonSerializer.SerializeToElement(await session.FinishAsync(new(review.Token)), SecurityWire.Json);
                Check(JsonNode.DeepEquals(inactiveProject["achievements"], JsonNode.Parse(result.GetProperty("project").GetProperty("achievements").GetRawText())) && portal.Writes == 0);
            }
        }
        foreach (JsonNode? invalid in new JsonNode?[] { null, JsonValue.Create(true), JsonValue.Create("Deleted") })
        {
            var invalidRequest = Request(achievement: true);
            JsonObject invalidProject = JsonNode.Parse(invalidRequest.Project.GetRawText())!.AsObject();
            invalidProject["achievements"]![0]!["steamSyncStatus"] = invalid;
            await Reject(() => new SynchronizationSession(new FakePortal()).PrepareAsync(invalidRequest with { Project = JsonSerializer.SerializeToElement(invalidProject) }, 480));
        }

        // A verified Steam identity rename imports a new card and preserves the
        // entire old card except its status, including its editable recipe.
        portal = new(); portal.AddAchievement(withIcons: true); session = new(portal);
        var renameRequest = Request(achievement: true);
        review = await session.PrepareAsync(renameRequest, 480);
        await session.BeginAsync(new(review.Token, []));
        result = JsonSerializer.SerializeToElement(await session.FinishAsync(new(review.Token)), SecurityWire.Json);
        JsonObject oldCard = JsonNode.Parse(result.GetProperty("project").GetProperty("achievements")[0].GetRawText())!.AsObject();
        portal.RenameAchievement("ACH_RENAMED");
        JsonObject foreignHistory = JsonNode.Parse(result.GetProperty("project").GetRawText())!.AsObject();
        foreignHistory["lastRemoteSnapshot"]!["steamAppId"] = 481;
        var foreignReview = await new SynchronizationSession(portal).PrepareAsync(renameRequest with { Project = JsonSerializer.SerializeToElement(foreignHistory) }, 480);
        Check(!foreignReview.Rows.Any(r => r.State == "RenamedOnSteam"));
        JsonObject ambiguousHistory = JsonNode.Parse(result.GetProperty("project").GetRawText())!.AsObject();
        JsonObject older = (JsonObject)ambiguousHistory["achievements"]![0]!.DeepClone();
        older["id"] = JsonValue.Create(Guid.NewGuid()); older["apiName"] = "ACH_OLDER"; older["internalId"] = 1;
        ambiguousHistory["achievements"]!.AsArray().Add(older); ambiguousHistory["nextAchievementInternalId"] = 2;
        JsonObject olderRemote = (JsonObject)ambiguousHistory["lastRemoteSnapshot"]!["achievements"]![0]!.DeepClone(); olderRemote["apiName"] = "ACH_OLDER";
        ambiguousHistory["lastRemoteSnapshot"]!["achievements"]!.AsArray().Add(olderRemote);
        var ambiguousReview = await new SynchronizationSession(portal).PrepareAsync(renameRequest with { Project = JsonSerializer.SerializeToElement(ambiguousHistory) }, 480);
        Check(!ambiguousReview.Rows.Any(r => r.State == "RenamedOnSteam"));
        session = new(portal);
        review = await session.PrepareAsync(renameRequest with { Project = result.GetProperty("project").Clone() }, 480);
        Check(!review.Rows.Any(r => r.ApiName == "ACH_WIN"));
        Check(review.Rows.Single(r => r.ApiName == "ACH_RENAMED").State == "RenamedOnSteam");
        await session.BeginAsync(new(review.Token, []));
        result = JsonSerializer.SerializeToElement(await session.FinishAsync(new(review.Token)), SecurityWire.Json);
        session = new(portal);
        review = await session.PrepareAsync(renameRequest with { Project = result.GetProperty("project").Clone() }, 480);
        Check(review.Rows.Single(r => r.ApiName == "ACH_RENAMED").State == "RenamedOnSteam");
        await Reject(() => session.BeginAsync(new(review.Token, [new("Achievement:ACH_WIN", "Upload")])));
        await session.BeginAsync(new(review.Token, [new("Achievement:ACH_RENAMED", "Import")]));
        imported = await session.StepAsync(new(review.Token, "Achievement:ACH_RENAMED"));
        result = JsonSerializer.SerializeToElement(await session.FinishAsync(new(review.Token)), SecurityWire.Json);
        JsonArray cards = JsonNode.Parse(result.GetProperty("project").GetProperty("achievements").GetRawText())!.AsArray();
        oldCard["steamSyncStatus"] = "Retired";
        Check(cards.Count == 2 && JsonNode.DeepEquals(cards[0], oldCard) && portal.Writes == 0);
        Check(cards[1]!["id"]!.GetValue<Guid>() != AchievementIdValue && imported.Artwork.Length == 2);
        Check(cards[1]!["icon"]!["layers"]!.AsArray().Count == 1 && cards[1]!["icon"]!["layers"]![0]!["kind"]!.GetValue<string>() == "StandaloneImage");
        Check(cards[1]!["tileIndex"]!.GetValue<int>() != cards[0]!["tileIndex"]!.GetValue<int>());
        JsonObject retainedProject = JsonNode.Parse(result.GetProperty("project").GetRawText())!.AsObject();
        // The host binds returned bytes into owned project-relative files.
        foreach (JsonObject image in retainedProject["assets"]!["images"]!.AsArray().OfType<JsonObject>())
            image["image"] = new JsonObject { ["kind"] = "RelativeToProject", ["path"] = "fixture.art-data-bank/owned.png" };
        review = await new SynchronizationSession(portal).PrepareAsync(renameRequest with { Project = JsonSerializer.SerializeToElement(retainedProject) }, 480);
        Check(!review.Rows.Any(r => r.ApiName == "ACH_WIN"));
        Console.WriteLine($"{checks} reviewed synchronization checks passed.");
    }

    private sealed class NonSeekable(byte[] bytes, bool stall = false) : Stream
    {
        private readonly MemoryStream input = new(bytes);
        public override bool CanRead => true;
        public override bool CanWrite => false;
        public override bool CanSeek => false;
        public override long Length => throw new NotSupportedException();
        public override long Position { get => throw new NotSupportedException(); set => throw new NotSupportedException(); }
        public override int Read(byte[] buffer, int offset, int count) => input.Read(buffer, offset, count);
        public override async ValueTask<int> ReadAsync(Memory<byte> buffer, CancellationToken cancellationToken = default)
        {
            if (stall) { await Task.Delay(Timeout.Infinite, cancellationToken); return 0; }
            return await input.ReadAsync(buffer, cancellationToken);
        }
        public override void Flush() => throw new NotSupportedException();
        public override long Seek(long offset, SeekOrigin origin) => throw new NotSupportedException();
        public override void SetLength(long value) => throw new NotSupportedException();
        public override void Write(byte[] buffer, int offset, int count) => throw new NotSupportedException();
        protected override void Dispose(bool disposing) { if (disposing) input.Dispose(); base.Dispose(disposing); }
    }

    private static PrepareSyncRequest Request(bool localStat = true, bool achievement = false)
    {
        JsonObject project = new()
        {
            ["format"] = "com.achimeld.project", ["schemaVersion"] = 1, ["projectId"] = "33333333-3333-4333-8333-333333333333",
            ["displayName"] = "Sync fixture", ["steamAppId"] = 480, ["nextAchievementInternalId"] = 1,
            ["assets"] = new JsonObject { ["atlas"] = new JsonObject { ["kind"] = "RelativeToProject", ["path"] = "atlas.png" },
                ["localizationCatalog"] = new JsonObject { ["kind"] = "RelativeToProject", ["path"] = "fixture.csv" }, ["images"] = new JsonArray(), ["atlases"] = new JsonArray() },
            ["atlas"] = new JsonObject { ["tileWidth"] = 512, ["tileHeight"] = 512, ["outputSize"] = 512, ["traversal"] = "RowMajor",
                ["unachievedIcon"] = new JsonObject { ["mode"] = "Grayscale", ["opacity"] = 1, ["tintHex"] = "#808080" } },
            ["stats"] = new JsonArray(), ["achievements"] = new JsonArray(), ["syncBaselines"] = new JsonArray(), ["tombstones"] = new JsonArray(),
        };
        if (localStat) project["stats"]!.AsArray().Add(new JsonObject { ["id"] = JsonValue.Create(StatIdValue), ["apiName"] = "SCORE", ["displayNameLocalizationId"] = "Aa1111",
            ["type"] = "Int", ["setBy"] = "GameClient", ["minimum"] = 0, ["maximum"] = 100, ["defaultValue"] = 0, ["displayOrder"] = 0 });
        if (achievement) project["achievements"]!.AsArray().Add(new JsonObject { ["id"] = JsonValue.Create(AchievementIdValue), ["apiName"] = "ACH_WIN",
            ["displayNameLocalizationId"] = "Bb2222", ["descriptionLocalizationId"] = "Cc3333", ["setBy"] = "GameClient", ["progressStatId"] = JsonValue.Create(StatIdValue), ["unlockValue"] = 10,
            ["tileIndex"] = 0, ["displayOrder"] = 0, ["internalId"] = 0, ["icon"] = new JsonObject { ["layers"] = new JsonArray(new JsonObject { ["kind"] = "Shape" }) } });
        return new(JsonSerializer.SerializeToElement(project), new() { ["Aa1111"] = new() { ["English"] = "Score" }, ["Bb2222"] = new() { ["English"] = "Winner" }, ["Cc3333"] = new() { ["English"] = "Get {0} points" } }, new());
    }

    private sealed class FakePortal : ISynchronizationPortal
    {
        public byte[]? Banner { get; init; }
        public uint BannerAppId { get; private set; }
        public Task<byte[]?> ReadGameBannerAsync(uint appId) { BannerAppId = appId; return Task.FromResult(Banner); }
        public RemoteSnapshot Snapshot { get; private set; } = new(new(480), DateTimeOffset.UtcNow, "fixture-v1", true, [], [], [], []) { SupportsDisplayOrder = false };
        public int Writes { get; private set; }
        public int Verifications { get; private set; }
        public bool SuppressSave { get; init; }
        public bool ConvertIcons { get; init; }
        public bool ChangeDownloadedArtwork { get; init; }
        public bool InjectUnrelatedChange { get; init; }
        public bool ChangeSameNameAchievementText { get; init; }
        public void AddAchievementText(string name, string text)
        {
            Dictionary<string, string> values = new() { ["English"] = text };
            string hash = CanonicalFingerprint.Compute(new[] { "English", "Polish", "ChineseSimplified", "ChineseTraditional", "Japanese", "Korean", "French" }.Select(l => new CanonicalField(l, values.GetValueOrDefault(l, "")))).EntityFingerprint;
            Snapshot = Snapshot with { Localizations = Snapshot.Localizations.Where(l => l.ApiName.Value != name || l.Field != LocalizationField.AchievementDisplayName)
                .Append(new RemoteLocalization(ProjectEntityType.Achievement, new(name), LocalizationField.AchievementDisplayName, values, hash)).ToArray() };
        }
        public void AddStat(string name, decimal maximum = 100)
        {
            RemoteStat value = new("1", new(name), StatType.Int, SetBy.GameClient, false, 0, maximum, 0, null, false, null, 0, "");
            value = value with { ContentFingerprint = CanonicalFingerprint.For(value, false).EntityFingerprint };
            Snapshot = Snapshot with { Stats = Snapshot.Stats.Append(value).ToArray() };
        }
        public void AddAchievement(bool withIcons = false)
        {
            AddStat("SCORE");
            RemoteAchievement value = new("2", new("ACH_WIN"), false, SetBy.GameClient, new SteamApiName("SCORE"), 10, 0, "");
            value = value with { ContentFingerprint = CanonicalFingerprint.For(value, false).EntityFingerprint };
            Snapshot = Snapshot with { Achievements = [value] };
            if (withIcons)
            {
                string hash = Convert.ToHexStringLower(SHA256.HashData(Png));
                Snapshot = Snapshot with { Icons = [new(new("ACH_WIN"), hash, hash, new Uri("https://cdn.steamstatic.com/a.png"), new Uri("https://cdn.steamstatic.com/b.png"),
                    CanonicalFingerprint.Compute([new("achievedIdentity", hash), new("unachievedIdentity", hash)]).EntityFingerprint)] };
            }
        }
        public void RenameAchievement(string name)
        {
            RemoteAchievement value = Snapshot.Achievements.Single() with { ApiName = new(name) };
            value = value with { ContentFingerprint = CanonicalFingerprint.For(value, false).EntityFingerprint };
            Snapshot = Snapshot with { Achievements = [value], Icons = Snapshot.Icons.Select(i => i with { AchievementApiName = new(name) }).ToArray(),
                Localizations = Snapshot.Localizations.Select(l => l.EntityType == ProjectEntityType.Achievement ? l with { ApiName = new(name) } : l).ToArray() };
        }
        public Task<RemoteSnapshot> ReadAsync(SteamAppId appId) => Task.FromResult(Snapshot);
        public Task<byte[]> DownloadArtworkAsync(Uri uri) => Task.FromResult(ChangeDownloadedArtwork ? new byte[] { 1, 2, 3 } : Png);
        public Task<bool> VerifyIconsAsync(PortalWriteOperation operation, RemoteIconMetadata? actual) { Verifications++; return Task.FromResult(actual is not null && operation.GeneratedUploads.Values.All(bytes => bytes.SequenceEqual(Png))); }
        public Task ApplyAsync(PortalWriteOperation operation)
        {
            Writes++;
            if (SuppressSave) return Task.CompletedTask;
            string name = operation.Target.ApiName.Value;
            if (operation.Target.Kind == PortalEntityKind.Stat)
            {
                var fields = operation.Fields;
                decimal? Number(string key) => fields[key].Length == 0 ? null : decimal.Parse(fields[key], CultureInfo.InvariantCulture);
                RemoteStat stat = new("1", new(name), Enum.Parse<StatType>(fields["type"]), Enum.Parse<SetBy>(fields["setBy"]), bool.Parse(fields["incrementOnly"]),
                    Number("minimum"), Number("maximum"), Number("defaultValue"), Number("maximumChange"), bool.Parse(fields["aggregated"]), Number("averageRateWindowSeconds"), 0, operation.DesiredFingerprint);
                Snapshot = Snapshot with { Stats = Snapshot.Stats.Where(s => s.ApiName.Value != name).Append(stat).ToArray() };
                if (InjectUnrelatedChange) AddStat("UNRELATED");
                if (ChangeSameNameAchievementText) AddAchievementText(name, "Changed by page");
            }
            else if (operation.Target.Kind == PortalEntityKind.Localization)
            {
                RemoteLocalization text = new(operation.Target.LocalizationField == LocalizationField.StatDisplayName ? ProjectEntityType.Stat : ProjectEntityType.Achievement,
                    new(name), operation.Target.LocalizationField!.Value, operation.Fields, operation.DesiredFingerprint);
                Snapshot = Snapshot with { Localizations = Snapshot.Localizations.Where(l => l.ApiName.Value != name || l.Field != text.Field).Append(text).ToArray() };
            }
            else if (operation.Target.Kind == PortalEntityKind.Icon)
            {
                string a = ConvertIcons ? new string('a', 64) : Convert.ToHexStringLower(SHA256.HashData(Png));
                string b = ConvertIcons ? new string('b', 64) : a;
                string fingerprint = CanonicalFingerprint.Compute([new("achievedIdentity", a), new("unachievedIdentity", b)]).EntityFingerprint;
                Snapshot = Snapshot with { Icons = [new(new(name), a, b, new Uri("https://cdn.steamstatic.com/a.png"), new Uri("https://cdn.steamstatic.com/b.png"), fingerprint)] };
            }
            return Task.CompletedTask;
        }
    }
}
