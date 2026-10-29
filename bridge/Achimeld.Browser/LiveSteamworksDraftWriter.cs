using System.Text.Json;
using Achimeld.Core;
using Microsoft.Playwright;

namespace Achimeld.Browser;

public sealed partial class PlaywrightChromeDriver
{
    public async Task ApplyLiveOperationAsync(PortalWriteOperation operation)
    {
        ArgumentNullException.ThrowIfNull(operation);
        await RequireHealthyAsync().ConfigureAwait(false);
        if (operation.Target.Kind == PortalEntityKind.Icon)
        {
            await UploadLiveIconPairAsync(operation).ConfigureAwait(false);
            return;
        }
        bool stat = operation.Target.Kind == PortalEntityKind.Stat ||
            operation.Target.LocalizationField == LocalizationField.StatDisplayName;
        string kind = stat ? "stats" : "achievements";
        string table = stat ? "statTable" : "achievementTable";
        await NavigateAsync(new Uri($"{SteamworksUrlGuard.AllowedOrigin}/apps/{kind}/{operation.AppId.Value}")).ConfigureAwait(false);
        IPage page = await EnsurePageAsync().ConfigureAwait(false);
        await page.Locator("#" + table).WaitForAsync().ConfigureAwait(false);
        using JsonDocument configuration = JsonDocument.Parse(
            await ReadConfigurationJsonAsync(operation.AppId, "fetch" + kind).ConfigureAwait(false));
        JsonElement array = stat ? configuration.RootElement : configuration.RootElement.GetProperty("achievements");
        JsonElement? existing = array.EnumerateArray().Where(item =>
            item.GetProperty(stat ? "name" : "api_name").GetString() == (operation.RenameFrom ?? operation.Target.ApiName.Value))
            .Select(static item => (JsonElement?)item).SingleOrDefault();
        if (operation.RenameFrom is not null)
        {
            if (operation.Delete || existing is null || operation.Target.Kind is not (PortalEntityKind.Stat or PortalEntityKind.Achievement))
                throw new InvalidDataException("The reviewed rename target is unavailable.");
            string identity = LivePortalIdentity.RecordId(existing.Value, "stat_id") +
                (stat ? "" : "_" + LivePortalIdentity.RecordId(existing.Value, "bit_id"));
            if (identity != operation.ExpectedSteamId || array.EnumerateArray().Any(item =>
                item.GetProperty(stat ? "name" : "api_name").GetString() != operation.RenameFrom &&
                string.Equals(item.GetProperty(stat ? "name" : "api_name").GetString(), operation.Target.ApiName.Value, StringComparison.OrdinalIgnoreCase)))
                throw new InvalidDataException("The reviewed rename identity or destination changed.");
        }
        if (operation.Delete)
        {
            if (operation.Target.Kind is not (PortalEntityKind.Stat or PortalEntityKind.Achievement) || operation.Fields.Count != 0 || operation.GeneratedUploads.Count != 0)
                throw new InvalidDataException("Invalid reviewed deletion.");
            if (existing is null) return;
            string row = stat ? "s" + LivePortalIdentity.RecordId(existing.Value, "stat_id")
                : "a" + LivePortalIdentity.RecordId(existing.Value, "stat_id") + "_" + LivePortalIdentity.RecordId(existing.Value, "bit_id");
            ILocator delete = page.Locator($"#{row} input[value='Delete']");
            if (row[1..] != operation.ExpectedSteamId) throw new InvalidDataException("Steam deletion identity changed.");
            await EnsureUniqueAsync(delete, "reviewed deletion control").ConfigureAwait(false);
            Uri deletionUri = _requestPolicy.ArmReviewedDeletion(operation.AppId.Value, row[1..]);
            EventHandler<IDialog> confirm = (_, dialog) => { if (dialog.Type == "confirm") _ = dialog.AcceptAsync(); else _ = dialog.DismissAsync(); };
            page.Dialog += confirm;
            try
            {
                await page.RunAndWaitForResponseAsync(() => delete.ClickAsync(), response =>
                    response.Url == deletionUri.AbsoluteUri).ConfigureAwait(false);
            }
            finally { page.Dialog -= confirm; }
            await RejectUnexpectedPopupsAsync().ConfigureAwait(false);
            return;
        }
        if (existing is null)
        {
            if (operation.Target.Kind == PortalEntityKind.Localization)
                throw new UnsupportedPortalStructureException("The localization target does not exist in Steamworks.");
            _requestPolicy.ArmReviewedCreation(operation.AppId.Value);
            await page.Locator($"#{table} thead input[value='New {(stat ? "Stat" : "Achievement")}']").ClickAsync().ConfigureAwait(false);
        }
        else
        {
            string row = stat ? "s" + LivePortalIdentity.RecordId(existing.Value, "stat_id")
                : "a" + LivePortalIdentity.RecordId(existing.Value, "stat_id") + "_" + LivePortalIdentity.RecordId(existing.Value, "bit_id");
            await page.Locator($"#{row} input[value='Edit']").ClickAsync().ConfigureAwait(false);
        }
        ILocator name = page.Locator(stat ? "input[id$='_statapiname']" : "input[id$='_apiname']");
        await name.WaitForAsync().ConfigureAwait(false);
        await EnsureUniqueAsync(name, "live API name editor").ConfigureAwait(false);
        string nameId = (await name.GetAttributeAsync("id").ConfigureAwait(false))!;
        string prefix = LivePortalIdentity.EditorPrefix(nameId);
        if (!stat)
        {
            // Editing text still saves the whole achievement row. Steam loads the
            // progress selection asynchronously, so saving before it is ready
            // would clear the existing link even during a text-only update.
            await page.WaitForFunctionAsync("id => document.getElementById(id)?.options[0]?.text === 'None'",
                prefix + "_progress").ConfigureAwait(false);
        }
        if (operation.Target.Kind == PortalEntityKind.Localization)
        {
            if (stat)
            {
                if (operation.Fields.Any(static field => field.Key != "English" && !string.IsNullOrEmpty(field.Value)))
                    throw new UnsupportedPortalStructureException("The Steam stat editor supports an English display name only.");
                await page.Locator($"#{prefix}_displayname").FillAsync(operation.Fields["English"]).ConfigureAwait(false);
            }
            else await FillLiveLocalizationAsync(page, prefix, operation).ConfigureAwait(false);
        }
        else
        {
            await name.FillAsync(operation.Fields["apiName"]).ConfigureAwait(false);
            string permission = operation.Fields["setBy"] switch
            {
                "GameClient" => "0", "GameServer" => "1", "OfficialGameServer" => "2",
                _ => throw new UnsupportedPortalStructureException("Unknown set-by permission.")
            };
            await page.Locator($"#{prefix}_permission").SelectOptionAsync(permission).ConfigureAwait(false);
            if (stat)
            {
                string type = operation.Fields["type"] switch
                {
                    "Int" => "INT", "Float" => "FLOAT", "AverageRate" => "AVGRATE",
                    _ => throw new UnsupportedPortalStructureException("Unknown stat type.")
                };
                await page.Locator($"#{prefix}_stattype").SelectOptionAsync(type).ConfigureAwait(false);
                foreach ((string key, string suffix) in new[] { ("minimum", "min"), ("maximum", "max"),
                    ("defaultValue", "default"), ("maximumChange", "maxchange"), ("averageRateWindowSeconds", "windowsize") })
                {
                    ILocator input = page.Locator($"#{prefix}_{suffix}");
                    if (await input.IsVisibleAsync().ConfigureAwait(false) && await input.IsEnabledAsync().ConfigureAwait(false))
                        await input.FillAsync(operation.Fields[key]).ConfigureAwait(false);
                }
                if (type != "AVGRATE")
                {
                    await page.Locator($"#{prefix}_incrementonly").SetCheckedAsync(bool.Parse(operation.Fields["incrementOnly"])).ConfigureAwait(false);
                    await page.Locator($"#{prefix}_aggregated").SetCheckedAsync(bool.Parse(operation.Fields["aggregated"])).ConfigureAwait(false);
                }
            }
            else
            {
                await page.Locator($"#{prefix}_hidden").SetCheckedAsync(bool.Parse(operation.Fields["hidden"])).ConfigureAwait(false);
                await page.WaitForFunctionAsync("id => document.getElementById(id).options[0].text === 'None'",
                    prefix + "_progress").ConfigureAwait(false);
                string progress = operation.Fields["progressStatApiName"];
                await page.Locator($"#{prefix}_progress").SelectOptionAsync(string.IsNullOrEmpty(progress) ? "-1" : progress).ConfigureAwait(false);
                if (!string.IsNullOrEmpty(progress))
                {
                    await page.Locator($"#{prefix}_minval").FillAsync("0").ConfigureAwait(false);
                    await page.Locator($"#{prefix}_maxval").FillAsync(operation.Fields["unlockValue"]).ConfigureAwait(false);
                }
            }
        }
        SteamworksUrlGuard.EnsureAllowed(new Uri(page.Url), operation.AppId);
        string endpoint = stat ? "savestat" : "saveachievement";
        _requestPolicy.ArmReviewedPost(operation.AppId.Value);
        IResponse saved = await page.RunAndWaitForResponseAsync(async () =>
            await name.Locator("xpath=ancestor::tr").Locator("input[value='Save']").ClickAsync().ConfigureAwait(false),
            response => response.Url == $"{SteamworksUrlGuard.AllowedOrigin}/apps/{endpoint}/{operation.AppId.Value}").ConfigureAwait(false);
        using JsonDocument result = JsonDocument.Parse(await saved.TextAsync().ConfigureAwait(false));
        if (!saved.Ok || !result.RootElement.TryGetProperty("saved", out JsonElement didSave) ||
            didSave.ToString() is not ("True" or "true" or "1"))
            throw new InvalidOperationException("Steamworks did not confirm the draft save.");
        await RejectUnexpectedPopupsAsync().ConfigureAwait(false);
    }

    private async Task UploadLiveIconPairAsync(PortalWriteOperation operation)
    {
        await NavigateAsync(new Uri($"{SteamworksUrlGuard.AllowedOrigin}/apps/achievements/{operation.AppId.Value}")).ConfigureAwait(false);
        IPage page = await EnsurePageAsync().ConfigureAwait(false);
        using JsonDocument configuration = JsonDocument.Parse(
            await ReadConfigurationJsonAsync(operation.AppId, "fetchachievements").ConfigureAwait(false));
        JsonElement achievement = configuration.RootElement.GetProperty("achievements").EnumerateArray()
            .Single(item => item.GetProperty("api_name").GetString() == operation.Target.ApiName.Value);
        bool achievedMatches = await VerifySavedIconAsync(achievement.GetProperty("icon").GetString() ?? "",
            operation.GeneratedUploads["achievedIcon"]).ConfigureAwait(false);
        bool unachievedMatches = await VerifySavedIconAsync(achievement.GetProperty("icon_gray").GetString() ?? "",
            operation.GeneratedUploads["unachievedIcon"]).ConfigureAwait(false);
        if (achievedMatches && unachievedMatches) return;
        string statId = LivePortalIdentity.RecordId(achievement, "stat_id"), bitId = LivePortalIdentity.RecordId(achievement, "bit_id");
        string rowId = "a" + statId + "_" + bitId;
        await page.Locator($"#{rowId} input[value='Edit']").ClickAsync().ConfigureAwait(false);
        await page.Locator("input[id$='_apiname']").WaitForAsync().ConfigureAwait(false);
        foreach ((string key, string requestType, string imageId) in new[]
            { ("achievedIcon", "achievement", rowId + "_icon"), ("unachievedIcon", "achievement_gray", rowId + "_icon_gray") })
        {
            if (key == "achievedIcon" ? achievedMatches : unachievedMatches) continue;
            SteamworksUrlGuard.EnsureAllowed(new Uri(page.Url), operation.AppId);
            ILocator form = page.Locator($"#{rowId} form").Filter(new LocatorFilterOptions
                { Has = page.Locator($"input[name='requestType'][value='{requestType}']") });
            await EnsureUniqueAsync(form, "achievement icon upload form").ConfigureAwait(false);
            if (await form.GetAttributeAsync("action").ConfigureAwait(false) != SteamworksUrlGuard.AllowedOrigin + "/images/uploadachievement" ||
                await form.Locator("input[name='appID']").InputValueAsync().ConfigureAwait(false) != operation.AppId.Value.ToString(System.Globalization.CultureInfo.InvariantCulture))
                throw new PortalSafetyException("portal.icon.target.rejected", "Icon upload form belongs to an unexpected target.");
            await form.Locator("input[type='file'][name='image']").SetInputFilesAsync(new FilePayload
                { Name = key + ".png", MimeType = "image/png", Buffer = operation.GeneratedUploads[key] }).ConfigureAwait(false);
            Task<IResponse> refreshed = page.WaitForResponseAsync(response => response.Url ==
                $"{SteamworksUrlGuard.AllowedOrigin}/apps/fetchachievement/{operation.AppId.Value}/{statId}/{bitId}");
            _requestPolicy.ArmReviewedPost(operation.AppId.Value);
            IResponse response = await page.RunAndWaitForResponseAsync(async () =>
                await form.Locator("input[type='submit'][value='Upload']").ClickAsync().ConfigureAwait(false),
                response => response.Url == SteamworksUrlGuard.AllowedOrigin + "/images/uploadachievement").ConfigureAwait(false);
            if (!response.Ok) throw new InvalidOperationException("Steamworks rejected the achievement icon upload.");
            IResponse readback = await refreshed.ConfigureAwait(false);
            using JsonDocument savedIcon = JsonDocument.Parse(await readback.TextAsync().ConfigureAwait(false));
            string savedSource = savedIcon.RootElement.GetProperty(requestType == "achievement" ? "icon" : "icon_gray").GetString() ?? "";
            if (string.IsNullOrEmpty(savedSource)) throw new InvalidOperationException("Steamworks did not save the achievement icon.");
            if (!await VerifySavedIconAsync(savedSource, operation.GeneratedUploads[key]).ConfigureAwait(false))
                throw new InvalidOperationException("Steamworks saved icon artwork that does not match the approved upload.");
            await page.WaitForFunctionAsync("args => document.getElementById(args.id)?.getAttribute('src') === args.source",
                new { id = imageId, source = savedSource }).ConfigureAwait(false);
            await RejectUnexpectedPopupsAsync().ConfigureAwait(false);
        }
    }

    private static async Task FillLiveLocalizationAsync(IPage page, string prefix, PortalWriteOperation operation)
    {
        string suffix = operation.Target.LocalizationField == LocalizationField.AchievementDisplayName ? "displayname" : "description";
        ILocator form = page.Locator($"#{prefix}_{suffix}");
        // The portal uses either a plain English field or one input per editing language.
        foreach ((string column, string steam) in new[] { ("English", "english"), ("Polish", "polish"),
            ("ChineseSimplified", "schinese"), ("ChineseTraditional", "tchinese"), ("Japanese", "japanese"),
            ("Korean", "koreana"), ("French", "french") })
        {
            string value = operation.Fields[column];
            ILocator input = form.Locator($"[name='{steam}']");
            if (await input.CountAsync().ConfigureAwait(false) == 0 && column == "English")
                input = form.Locator($"[name='{(suffix == "displayname" ? "display_name" : "description")}']");
            if (await input.CountAsync().ConfigureAwait(false) > 0)
            {
                if (await input.GetAttributeAsync("type").ConfigureAwait(false) == "hidden")
                    await input.EvaluateAsync("(input, value) => { input.value = value; input.dispatchEvent(new Event('change', { bubbles: true })); }", value).ConfigureAwait(false);
                else await input.FillAsync(value).ConfigureAwait(false);
            }
            else if (!string.IsNullOrEmpty(value))
                throw new UnsupportedPortalStructureException($"Enable '{steam}' as an editing language in Steamworks before uploading its text.");
        }
    }
}
