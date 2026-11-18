using Achimeld.Core;
using Microsoft.Playwright;
using System.Globalization;
using System.Text.Json;

namespace Achimeld.Browser;

public sealed partial class PlaywrightChromeDriver
{
    internal string? ReviewedModernAchievementIdentity(string name, string? setId) =>
        _modernExport is not null && _modernAchievementRecords.TryGetValue(name, out var record) && record.SetId == setId
            ? record.Identity : null;

    private async Task<bool> TryApplyReviewedModernAchievementAsync(PortalWriteOperation operation)
    {
        bool move = operation.Target.Kind == PortalEntityKind.AchievementSet;
        if (!move && (_modernExport is null || operation.ExpectedSteamId is null ||
            operation.Delete || operation.RenameFrom is not null || operation.Target.Kind is not
                (PortalEntityKind.Achievement or PortalEntityKind.Localization))) return false;

        string name = operation.Target.ApiName.Value;
        if (!_modernAchievementRecords.TryGetValue(name, out var record) ||
            record.Identity != operation.ExpectedSteamId ||
            move && (record.SetId ?? SyncFingerprint.Absent) != operation.ExpectedOldFingerprint)
            throw new UnsupportedPortalStructureException("The reviewed Steamworks achievement identity changed.");
        Dictionary<string, string> fields = move ? new()
        {
            ["appid"] = operation.AppId.Value.ToString(CultureInfo.InvariantCulture),
            ["groupid"] = operation.DesiredFingerprint == SyncFingerprint.Absent ? "0" : operation.DesiredFingerprint,
            ["names"] = JsonSerializer.Serialize(new[] { name }),
        } : ModernAchievementDraft.Definition(name, record, operation.ReviewedAchievementDefinition ?? operation.Fields);
        if (operation.Target.Kind == PortalEntityKind.Localization)
            ModernAchievementDraft.AddLocalization(fields, record, operation, _modernExport!.LocalizationCsv);

        Uri destination = _requestPolicy.ArmReviewedModernForm(operation.AppId.Value, move, fields);
        SetSyncDiagnosticEndpoint(move ? "ajaxmoveachievements" : "ajaxcreateorupdateachievement");
        await PostReviewedModernFormAsync(operation.AppId, destination, fields).ConfigureAwait(false);
        return true;
    }

    private async Task UploadReviewedModernIconsAsync(PortalWriteOperation operation)
    {
        if (!_modernAchievementRecords.TryGetValue(operation.Target.ApiName.Value, out var record) ||
            record.Identity != operation.ExpectedSteamId)
            throw new UnsupportedPortalStructureException("The reviewed Steamworks achievement identity changed.");
        string app = operation.AppId.ToString();
        foreach ((string key, string type, string suffix) in new[]
            { ("achievedIcon", "achievement", "a"), ("unachievedIcon", "achievement_gray", "g") })
        {
            Dictionary<string, string> fields = new()
            {
                ["appID"] = app, ["statID"] = record.StatId, ["bit"] = record.BitId, ["requestType"] = type,
            };
            byte[] image = operation.GeneratedUploads[key];
            string fileName = $"{app}_{record.StatId}_{record.BitId}_{suffix}.png";
            Uri destination = _requestPolicy.ArmReviewedModernIcon(operation.AppId.Value, fields, fileName, image);
            SetSyncDiagnosticEndpoint("uploadachievement");
            await PostReviewedModernFormAsync(operation.AppId, destination, fields, fileName, image).ConfigureAwait(false);
        }
    }

    private async Task PostReviewedModernFormAsync(SteamAppId appId, Uri destination,
        Dictionary<string, string> fields, string? fileName = null, byte[]? image = null)
    {
        IPage page = await EnsurePageAsync().ConfigureAwait(false);
        SteamworksUrlGuard.EnsureAllowed(new Uri(page.Url), appId);
        SetSyncDiagnosticWriteState("save-armed");
        int status = await page.EvaluateAsync<int>("""
            async ({path, fields, fileName, image}) => {
                const body = new FormData();
                for (const [key, value] of Object.entries(fields)) body.append(key, value);
                if (image !== null) {
                    const bytes = Uint8Array.fromBase64(image);
                    body.append('image', new File([bytes], fileName, {type: 'image/png'}));
                }
                const response = await fetch(path, {
                    method: 'POST', credentials: 'same-origin', redirect: 'error',
                    body, signal: AbortSignal.timeout(20000)
                });
                if (!response.ok) return -response.status;
                const data = await response.text();
                if (data.length > 65536) return -response.status;
                try { return Number(JSON.parse(data)?.success) === 1 ? response.status : -response.status; }
                catch { return -response.status; }
            }
            """, new { path = destination.AbsolutePath, fields, fileName,
                image = image is null ? null : Convert.ToBase64String(image) }).ConfigureAwait(false);
        SetSyncDiagnosticResponseStatus(Math.Abs(status));
        _requestPolicy.RequireHealthy();
        if (status <= 0)
            throw new InvalidOperationException("Steamworks did not confirm the reviewed achievement draft save.");
        SetSyncDiagnosticWriteState("save-confirmed");
        _modernImportNeedsFreshPage = true;
        await RejectUnexpectedPopupsAsync().ConfigureAwait(false);
    }

    // The new editor keeps its achievement cards in the management view after
    // a complete CSV read. The legacy table writer must not navigate away from
    // that view or attempt its old Save endpoint.
    private async Task<bool> TryApplyModernLocalizationAsync(PortalWriteOperation operation)
    {
        IPage page = await EnsurePageAsync().ConfigureAwait(false);
        if (await page.GetByText("Manage Achievements", new() { Exact = true }).CountAsync()
            .ConfigureAwait(false) != 1 || await page.Locator("#achievementTable").CountAsync()
            .ConfigureAwait(false) != 0) return false;
        if (operation.Target.Kind != PortalEntityKind.Localization ||
            operation.Target.LocalizationField is not (LocalizationField.AchievementDisplayName or
                LocalizationField.AchievementDescription))
            throw new UnsupportedPortalStructureException("The reviewed modern achievement edit is not supported.");
        if (await TryApplyModernLocalizationCsvAsync(page, operation).ConfigureAwait(false)) return true;
        SetSyncDiagnosticWriteState("modern-editor-select");
        ILocator cardName = page.GetByText(operation.Target.ApiName.Value, new() { Exact = true });
        if (await cardName.CountAsync().ConfigureAwait(false) != 1)
            throw new UnsupportedPortalStructureException("The reviewed achievement card is unavailable.");
        ILocator card = cardName.Locator("xpath=../..");
        ILocator? edit = await FindModernCardEditAsync(page, card).ConfigureAwait(false);
        if (edit is null)
            throw new UnsupportedPortalStructureException("The reviewed achievement edit control is ambiguous.");
        await edit.ClickAsync().ConfigureAwait(false);
        _requestPolicy.RequireHealthy();
        await page.WaitForTimeoutAsync(1500).ConfigureAwait(false);
        SetSyncDiagnosticWriteState("modern-editor-field");
        string label = operation.Target.LocalizationField == LocalizationField.AchievementDisplayName
            ? "Display Name" : "Description";
        ILocator? field = await FindModernTextFieldAsync(page, label, operation.Target.ApiName.Value)
            .ConfigureAwait(false);
        if (field is null)
        {
            int buttonShape = await card.EvaluateAsync<int>("""
                element => {
                    const buttons = [...element.querySelectorAll('button')].slice(0, 15);
                    let textCount = 0, imageCount = 0, svgCount = 0;
                    let edit = false, menu = false;
                    for (const button of buttons) {
                        if ((button.innerText || '').trim()) textCount++;
                        if (button.querySelector('img')) imageCount++;
                        if (button.querySelector('svg')) svgCount++;
                        const label = [button.innerText, button.title,
                            button.getAttribute('aria-label'), button.getAttribute('data-tooltip-content')]
                            .filter(Boolean).join(' ').toLowerCase();
                        if (label.includes('edit')) edit = true;
                        if (label.includes('action') || label.includes('more') ||
                            label.includes('option')) menu = true;
                    }
                    return buttons.length | (Math.min(textCount, 7) << 4) |
                        (Math.min(imageCount, 7) << 7) | (Math.min(svgCount, 7) << 10) |
                        (edit ? 1 << 13 : 0) | (menu ? 1 << 14 : 0);
                }
                """).ConfigureAwait(false);
            SetSyncDiagnosticWriteState($"modern-card-shape-{buttonShape}");
        }
        if (field is null || !operation.Fields.TryGetValue("English", out string? desired))
            throw new UnsupportedPortalStructureException("The reviewed achievement text field is ambiguous.");
        await field.FillAsync(desired).ConfigureAwait(false);
        _requestPolicy.RequireHealthy();
        SetSyncDiagnosticWriteState("modern-editor-save-control");
        ILocator save = page.GetByRole(AriaRole.Button, new() { Name = "Save", Exact = true });
        if (await save.CountAsync().ConfigureAwait(false) != 1)
            throw new UnsupportedPortalStructureException("The modern achievement save control is ambiguous.");
        Uri saveUri = new($"https://partner.steamgames.com/achievements/ajaxcreateorupdateachievement/{operation.AppId.Value}");
        _requestPolicy.ArmReviewedModernAchievementPost(operation.AppId.Value,
            operation.Target.ApiName.Value, desired);
        SetSyncDiagnosticEndpoint("ajaxcreateorupdateachievement");
        SetSyncDiagnosticWriteState("save-armed");
        Task<IResponse> pendingSave = page.RunAndWaitForResponseAsync(() => save.ClickAsync(),
            response => response.Url == saveUri.AbsoluteUri);
        while (!pendingSave.IsCompleted)
        {
            await Task.WhenAny(pendingSave, Task.Delay(200)).ConfigureAwait(false);
            if (_requestPolicy.Rejection != BrowserRequestRejection.None)
            {
                _ = pendingSave.ContinueWith(task => _ = task.Exception,
                    TaskContinuationOptions.OnlyOnFaulted);
                _requestPolicy.RequireHealthy();
            }
        }
        IResponse saved = await pendingSave.ConfigureAwait(false);
        SetSyncDiagnosticResponseStatus(saved.Status);
        _requestPolicy.RequireHealthy();
        if (!saved.Ok) throw new InvalidOperationException("Steamworks did not confirm the achievement draft save.");
        SetSyncDiagnosticWriteState("save-confirmed");
        await RejectUnexpectedPopupsAsync().ConfigureAwait(false);
        return true;
    }

    private async Task<bool> TryApplyModernLocalizationCsvAsync(IPage page, PortalWriteOperation operation)
    {
        if (_modernExport is null || !operation.Fields.TryGetValue("English", out string? desired))
            throw new UnsupportedPortalStructureException("The reviewed localization export is unavailable.");
        string field = operation.Target.LocalizationField == LocalizationField.AchievementDisplayName
            ? "name" : "description";
        byte[] csv = SteamworksModernLocalizationImport.BuildOneAchievement(
            _modernExport.LocalizationCsv, operation.Target.ApiName.Value, field, desired);
        ILocator tab = page.GetByText("Bulk Import/Export", new() { Exact = true });
        if (await tab.CountAsync().ConfigureAwait(false) != 1)
            throw new UnsupportedPortalStructureException("The modern Steamworks import tab is unavailable.");
        await tab.ClickAsync().ConfigureAwait(false);
        await page.WaitForTimeoutAsync(500).ConfigureAwait(false);
        ILocator inputs = page.Locator("input[type='file']");
        ILocator select = page.GetByText("Select Files...", new() { Exact = true });
        int inputCount = await inputs.CountAsync().ConfigureAwait(false);
        int selectCount = await select.CountAsync().ConfigureAwait(false);
        int mask = 3 | (inputCount == 1 ? 4 : 0) | (selectCount == 1 ? 8 : 0);
        SetSyncDiagnosticWriteState($"modern-import-shape-{mask}");
        if (inputCount != 1)
            throw new UnsupportedPortalStructureException("The modern Steamworks file input is ambiguous.");
        await inputs.SetInputFilesAsync(new FilePayload
        {
            Name = $"{operation.AppId.Value}-achievements-localization.csv",
            MimeType = "text/csv",
            Buffer = csv,
        }).ConfigureAwait(false);
        await page.WaitForTimeoutAsync(1500).ConfigureAwait(false);
        _requestPolicy.RequireHealthy();
        if (await page.GetByText("Pending Changes", new() { Exact = true }).CountAsync()
            .ConfigureAwait(false) == 1) mask |= 16;
        if (await page.GetByText("Save imported data", new() { Exact = true }).CountAsync()
            .ConfigureAwait(false) == 1) mask |= 32;
        if (await page.GetByText(operation.Target.ApiName.Value, new() { Exact = true }).CountAsync()
            .ConfigureAwait(false) == 1) mask |= 64;
        if (await page.GetByText(desired, new() { Exact = true }).CountAsync()
            .ConfigureAwait(false) == 1) mask |= 128;
        int actions = await page.EvaluateAsync<int>("""
            ({api, desired}) => {
                const visible = element => !!(element.offsetWidth || element.offsetHeight ||
                    element.getClientRects().length);
                const controls = [...document.querySelectorAll('button,a,[role="button"]')]
                    .filter(visible);
                const save = controls.filter(element => /save/i.test((element.innerText || '').trim()));
                const body = document.body?.innerText || '';
                const ids = new Set(body.match(/\bACHIEVEMENT_[A-Za-z0-9_]{1,80}\b/g) || []);
                let flags = 0;
                if (save.length === 1) flags |= 1;
                if (save.length === 1 && !save[0].disabled &&
                    save[0].getAttribute('aria-disabled') !== 'true') flags |= 2;
                if (save.length === 1 && /import/i.test(save[0].innerText || '')) flags |= 4;
                if (save.length === 1 && /change|data/i.test(save[0].innerText || '')) flags |= 8;
                if (controls.some(element => /apply/i.test(element.innerText || ''))) flags |= 16;
                if (ids.size === 1 && ids.has(api)) flags |= 32;
                if (body.includes(desired)) flags |= 64;
                if (![...document.querySelectorAll('[role="alert"]')].some(visible)) flags |= 128;
                return flags;
            }
            """, new { api = operation.Target.ApiName.Value, desired }).ConfigureAwait(false);
        SetSyncDiagnosticWriteState($"modern-import-actions-{actions}");
        int preview = await page.EvaluateAsync<int>("""
            ({api, desired}) => {
                const heading = [...document.querySelectorAll('*')].filter(element =>
                    element.children.length === 0 &&
                    (element.textContent || '').trim() === 'Pending Changes');
                let mask = heading.length === 1 ? 1 : 0;
                if (heading.length !== 1) return mask;
                let parent = heading[0];
                for (let depth = 1; depth <= 5 && parent; depth++, parent = parent.parentElement) {
                    const text = parent.innerText || '';
                    if (!text.includes(api) || !text.includes(desired)) continue;
                    mask |= 2;
                    const ids = new Set(text.match(/\bACHIEVEMENT_[A-Za-z0-9_]{1,80}\b/g) || []);
                    if (ids.size === 1 && ids.has(api)) mask |= 4;
                    if (text.includes(desired)) mask |= 8;
                    if (/\bname\b/i.test(text)) mask |= 16;
                    break;
                }
                if (![...document.querySelectorAll('[role="alert"]')].some(element =>
                    element.offsetWidth || element.offsetHeight || element.getClientRects().length))
                    mask |= 32;
                return mask;
            }
            """, new { api = operation.Target.ApiName.Value, desired }).ConfigureAwait(false);
        ILocator saveImport = page.GetByRole(AriaRole.Button, new() { Name = "Save", Exact = true });
        if (await saveImport.CountAsync().ConfigureAwait(false) == 1)
        {
            preview |= 64;
            if (await saveImport.IsEnabledAsync().ConfigureAwait(false)) preview |= 128;
        }
        SetSyncDiagnosticWriteState($"modern-preview-shape-{preview}");
        uint saveScope = await page.EvaluateAsync<uint>("""
            ({api, desired}) => {
                const visible = element => !!(element.offsetWidth || element.offsetHeight ||
                    element.getClientRects().length);
                const buttons = [...document.querySelectorAll('button,[role="button"]')]
                    .filter(element => visible(element) &&
                        (element.innerText || '').trim() === 'Save');
                if (buttons.length !== 1) return 0;
                let element = buttons[0];
                let mask = 0;
                for (let depth = 0; depth < 7 && element; depth++, element = element.parentElement) {
                    const text = element.innerText || '';
                    let bits = 0;
                    if (element.querySelector('input[type="file"]')) bits |= 1;
                    if (text.includes('Pending Changes')) bits |= 2;
                    if (text.includes(desired)) bits |= 4;
                    if (text.includes(api)) bits |= 8;
                    mask |= bits << (depth * 4);
                }
                return mask >>> 0;
            }
            """, new { api = operation.Target.ApiName.Value, desired }).ConfigureAwait(false);
        bool scopedSave = Enumerable.Range(1, 4).Any(depth =>
            ((saveScope >> (depth * 4)) & 15) == 15);
        if (!scopedSave)
        {
            SetSyncDiagnosticWriteState($"modern-save-scope-{saveScope}");
            throw new UnsupportedPortalStructureException("The modern Steamworks import save scope needs qualification.");
        }
        if ((preview & 239) != 239)
        {
            uint scope = await page.EvaluateAsync<uint>("""
                ({api, desired}) => {
                    const heading = [...document.querySelectorAll('*')].filter(element =>
                        element.children.length === 0 &&
                        (element.textContent || '').trim() === 'Pending Changes');
                    if (heading.length !== 1) return 0;
                    let element = heading[0];
                    let mask = 0;
                    for (let depth = 0; depth < 7 && element; depth++, element = element.parentElement) {
                        const text = element.innerText || '';
                        const ids = new Set(text.match(/\bACHIEVEMENT_[A-Za-z0-9_]{1,80}\b/g) || []);
                        let nibble = 0;
                        if (text.includes(desired)) nibble |= 1;
                        if (ids.has(api)) nibble |= 2;
                        if ([...ids].some(id => id !== api)) nibble |= 4;
                        if (/\bname\b/i.test(text)) nibble |= 8;
                        mask |= nibble << (depth * 4);
                    }
                    return mask >>> 0;
                }
                """, new { api = operation.Target.ApiName.Value, desired }).ConfigureAwait(false);
            SetSyncDiagnosticWriteState($"modern-preview-scope-{scope}");
            uint targetScope = await page.EvaluateAsync<uint>("""
                ({api, desired}) => {
                    const leaves = [...document.querySelectorAll('*')].filter(element =>
                        element.children.length === 0 &&
                        (element.textContent || '').trim() === desired);
                    if (leaves.length !== 1) return leaves.length > 1 ? 1 : 0;
                    let element = leaves[0];
                    let mask = 0;
                    for (let depth = 0; depth < 6 && element; depth++, element = element.parentElement) {
                        const text = element.innerText || '';
                        const ids = new Set(text.match(/\bACHIEVEMENT_[A-Za-z0-9_]{1,80}\b/g) || []);
                        let bits = 0;
                        if (ids.has(api)) bits |= 1;
                        if ([...ids].some(id => id !== api)) bits |= 2;
                        if (text.includes('Pending Changes')) bits |= 4;
                        if (/\bname\b/i.test(text)) bits |= 8;
                        if (/\bsave\b/i.test(text)) bits |= 16;
                        mask |= bits << (depth * 5);
                    }
                    return mask >>> 0;
                }
                """, new { api = operation.Target.ApiName.Value, desired }).ConfigureAwait(false);
            SetSyncDiagnosticWriteState($"modern-target-scope-{targetScope}");
            if (preview != 251 || ((targetScope >> 10) & 31) != 1 ||
                ((targetScope >> 15) & 31) != 9 || ((targetScope >> 20) & 31) != 9)
            {
                SetSyncDiagnosticWriteState($"modern-probe-gate-{preview}");
                throw new UnsupportedPortalStructureException("The modern Steamworks import preview is ambiguous.");
            }
        }
        Uri saveUri = new($"https://partner.steamgames.com/achievements/ajaxcreateorupdateachievement/{operation.AppId.Value}");
        _requestPolicy.ArmReviewedModernAchievementPost(operation.AppId.Value,
            operation.Target.ApiName.Value, desired,
            SteamworksModernLocalizationImport.KnownApiNames(_modernExport.LocalizationCsv),
            localizationImport: true);
        SetSyncDiagnosticEndpoint("ajaxcreateorupdateachievement");
        SetSyncDiagnosticWriteState("save-armed");
        Task<IResponse> pendingSave = page.RunAndWaitForResponseAsync(() => saveImport.ClickAsync(),
            response => response.Url == saveUri.AbsoluteUri);
        while (!pendingSave.IsCompleted)
        {
            await Task.WhenAny(pendingSave, Task.Delay(200)).ConfigureAwait(false);
            if (_requestPolicy.Rejection != BrowserRequestRejection.None)
            {
                _ = pendingSave.ContinueWith(task => _ = task.Exception,
                    TaskContinuationOptions.OnlyOnFaulted);
                _requestPolicy.RequireHealthy();
            }
        }
        IResponse saved = await pendingSave.ConfigureAwait(false);
        SetSyncDiagnosticResponseStatus(saved.Status);
        _requestPolicy.RequireHealthy();
        if (!saved.Ok) throw new InvalidOperationException("Steamworks did not confirm the localization import save.");
        SetSyncDiagnosticWriteState("save-confirmed");
        _modernImportNeedsFreshPage = true;
        await RejectUnexpectedPopupsAsync().ConfigureAwait(false);
        return true;
    }

    private async Task<ILocator?> FindModernCardEditAsync(IPage page, ILocator card)
    {
        ILocator exact = card.GetByRole(AriaRole.Button, new() { Name = "Edit", Exact = true });
        if (await exact.CountAsync().ConfigureAwait(false) == 1) return exact;
        ILocator buttons = card.Locator("button");
        int count = await buttons.CountAsync().ConfigureAwait(false);
        if (count != 2) return null;
        int mask = 1;
        List<ILocator> candidates = [];
        for (int index = 0; index < count; index++)
        {
            ILocator button = buttons.Nth(index);
            string? title = await button.GetAttributeAsync("title").ConfigureAwait(false);
            string? aria = await button.GetAttributeAsync("aria-label").ConfigureAwait(false);
            string? hint = await button.GetAttributeAsync("data-tooltip-content").ConfigureAwait(false);
            ILocator svg = button.Locator("svg");
            string? svgClass = await svg.CountAsync().ConfigureAwait(false) == 1
                ? await svg.GetAttributeAsync("class").ConfigureAwait(false) : null;
            bool namedEdit = new[] { title, aria, hint }.Any(value =>
                value is not null && (value.Equals("Edit", StringComparison.OrdinalIgnoreCase) ||
                value.Equals("Edit Achievement", StringComparison.OrdinalIgnoreCase)));
            bool iconEdit = svgClass is not null &&
                (svgClass.Contains("pencil", StringComparison.OrdinalIgnoreCase) ||
                    svgClass.Contains("edit", StringComparison.OrdinalIgnoreCase));
            if (namedEdit || iconEdit)
            {
                candidates.Add(button);
                mask |= 1 << (1 + index);
                continue;
            }
            await button.HoverAsync().ConfigureAwait(false);
            await page.WaitForTimeoutAsync(350).ConfigureAwait(false);
            ILocator tooltip = page.Locator("[role='tooltip']:visible");
            if (await tooltip.CountAsync().ConfigureAwait(false) > 0) mask |= 1 << (3 + index);
            if (await tooltip.GetByText("Edit", new() { Exact = true }).CountAsync()
                .ConfigureAwait(false) == 1 || await tooltip.GetByText("Edit Achievement", new() { Exact = true })
                .CountAsync().ConfigureAwait(false) == 1)
            {
                candidates.Add(button);
                mask |= 1 << (5 + index);
            }
        }
        if (candidates.Count == 1) return candidates[0];
        SetSyncDiagnosticWriteState($"modern-card-shape-{mask}");
        return null;
    }

    private async Task<ILocator?> FindModernTextFieldAsync(IPage page, string label, string apiName)
    {
        // Steam's newer editor may render a visual field caption without an
        // HTML label association. Each fallback is limited to the exact
        // reviewed caption and must resolve to one visible editor control.
        string[] captions = label == "Display Name" ? ["Display Name", "Achievement Name", "Name"] : [label];
        foreach (string caption in captions)
        {
            ILocator[] candidates =
            [
                page.GetByLabel(caption, new() { Exact = true }),
                page.GetByPlaceholder(caption, new() { Exact = true }),
                page.Locator($"input[aria-label='{caption}'], textarea[aria-label='{caption}']"),
                page.GetByText(caption, new() { Exact = true }).Locator("xpath=..").Locator("input, textarea"),
                page.GetByText(caption, new() { Exact = true }).Locator("xpath=../..").Locator("input, textarea"),
            ];
            foreach (ILocator candidate in candidates)
                if (await candidate.CountAsync().ConfigureAwait(false) == 1 &&
                    await candidate.IsVisibleAsync().ConfigureAwait(false) &&
                    await candidate.IsEnabledAsync().ConfigureAwait(false))
                    return candidate;
        }
        int mask = 0;
        if (await page.GetByText("Display Name", new() { Exact = true }).CountAsync().ConfigureAwait(false) > 0) mask |= 1;
        if (await page.GetByText("Name", new() { Exact = true }).CountAsync().ConfigureAwait(false) > 0) mask |= 2;
        if (await page.GetByText("Achievement Name", new() { Exact = true }).CountAsync().ConfigureAwait(false) > 0) mask |= 4;
        if (await page.GetByText("API Name", new() { Exact = true }).CountAsync().ConfigureAwait(false) > 0) mask |= 8;
        if (await page.GetByText("English", new() { Exact = true }).CountAsync().ConfigureAwait(false) > 0) mask |= 16;
        if (await page.GetByRole(AriaRole.Button, new() { Name = "Save", Exact = true }).CountAsync().ConfigureAwait(false) > 0) mask |= 32;
        if (await page.GetByRole(AriaRole.Button, new() { Name = "Edit", Exact = true }).CountAsync().ConfigureAwait(false) > 0) mask |= 64;
        if (await page.Locator("input:not([type='hidden']), textarea").CountAsync().ConfigureAwait(false) > 0) mask |= 128;
        if (await page.GetByRole(AriaRole.Dialog).CountAsync().ConfigureAwait(false) > 0) mask |= 256;
        ILocator dialog = page.GetByRole(AriaRole.Dialog);
        if (await dialog.CountAsync().ConfigureAwait(false) == 1)
        {
            int inputs = await dialog.Locator("input:visible, textarea:visible").CountAsync().ConfigureAwait(false);
            if (inputs == 1) mask |= 512;
            else if (inputs == 2) mask |= 1024;
            else if (inputs > 2) mask |= 2048;
            ILocator english = dialog.GetByText("English", new() { Exact = true });
            if (await english.CountAsync().ConfigureAwait(false) == 1)
            {
                if (await english.Locator("xpath=..").Locator("input:visible, textarea:visible")
                    .CountAsync().ConfigureAwait(false) == 1) mask |= 4096;
                if (await english.Locator("xpath=../..").Locator("input:visible, textarea:visible")
                    .CountAsync().ConfigureAwait(false) == 1) mask |= 8192;
                int rowInputs = await english.Locator("xpath=../../..").Locator("input:visible, textarea:visible")
                    .CountAsync().ConfigureAwait(false);
                if (rowInputs == 1) mask |= 16384;
                else if (rowInputs == 2) mask |= 32768;
            }
        }
        if (await page.GetByRole(AriaRole.Button, new() { Name = "Save Changes", Exact = true })
            .CountAsync().ConfigureAwait(false) > 0) mask |= 65536;
        if (await page.GetByRole(AriaRole.Button, new() { Name = "Apply", Exact = true })
            .CountAsync().ConfigureAwait(false) > 0) mask |= 131072;
        if (await page.GetByRole(AriaRole.Button, new() { Name = "Update", Exact = true })
            .CountAsync().ConfigureAwait(false) > 0) mask |= 262144;
        if (await page.GetByRole(AriaRole.Button, new() { Name = "Done", Exact = true })
            .CountAsync().ConfigureAwait(false) > 0) mask |= 524288;
        int dialogs = await dialog.CountAsync().ConfigureAwait(false);
        if (dialogs == 1) mask |= 1048576;
        else if (dialogs > 1) mask |= 2097152;
        ILocator pageEnglish = page.GetByText("English", new() { Exact = true });
        if (await pageEnglish.CountAsync().ConfigureAwait(false) == 1)
        {
            if (await pageEnglish.Locator("xpath=..").Locator("input:visible, textarea:visible")
                .CountAsync().ConfigureAwait(false) > 0) mask |= 4194304;
            if (await pageEnglish.Locator("xpath=../..").Locator("input:visible, textarea:visible")
                .CountAsync().ConfigureAwait(false) > 0) mask |= 8388608;
            if (await pageEnglish.Locator("xpath=../../..").Locator("input:visible, textarea:visible")
                .CountAsync().ConfigureAwait(false) > 0) mask |= 16777216;
        }
        if (await page.GetByRole(AriaRole.Button, new() { Name = "Edit Achievement", Exact = true })
            .CountAsync().ConfigureAwait(false) > 0) mask |= 33554432;
        if (await page.GetByText(apiName, new() { Exact = true })
            .Locator("xpath=..").Locator("button").CountAsync().ConfigureAwait(false) > 0) mask |= 67108864;
        SetSyncDiagnosticWriteState($"modern-editor-shape-{mask}");
        if (dialogs == 1)
        {
            int structure = 0;
            if (await dialog.GetByText("English", new() { Exact = true }).CountAsync().ConfigureAwait(false) > 0)
                structure |= 1;
            if (await dialog.GetByText(apiName, new() { Exact = true }).CountAsync().ConfigureAwait(false) > 0)
                structure |= 2;
            if (await dialog.GetByText("Display Name", new() { Exact = true }).CountAsync()
                .ConfigureAwait(false) > 0) structure |= 4;
            if (await dialog.GetByText("Description", new() { Exact = true }).CountAsync()
                .ConfigureAwait(false) > 0) structure |= 8;
            int textboxes = await dialog.GetByRole(AriaRole.Textbox).CountAsync().ConfigureAwait(false);
            if (textboxes == 1) structure |= 16;
            else if (textboxes > 1) structure |= 32;
            int editables = await dialog.Locator("[contenteditable='true']").CountAsync()
                .ConfigureAwait(false);
            if (editables == 1) structure |= 64;
            else if (editables > 1) structure |= 128;
            if (await dialog.Locator("iframe").CountAsync().ConfigureAwait(false) > 0) structure |= 256;
            int buttons = await dialog.GetByRole(AriaRole.Button).CountAsync().ConfigureAwait(false);
            if (buttons == 1) structure |= 512;
            else if (buttons == 2) structure |= 1024;
            else if (buttons > 2) structure |= 2048;
            if (await dialog.GetByRole(AriaRole.Link).CountAsync().ConfigureAwait(false) > 0)
                structure |= 4096;
            if (await dialog.GetByText("Edit", new() { Exact = true }).CountAsync()
                .ConfigureAwait(false) > 0) structure |= 8192;
            if (await dialog.GetByText("Save", new() { Exact = true }).CountAsync()
                .ConfigureAwait(false) > 0) structure |= 16384;
            SetSyncDiagnosticWriteState($"modern-editor-structure-{structure}");
            int actions = 0;
            string[] labels = ["Edit", "Close", "Cancel", "Save", "Done", "OK", "Continue",
                "Move", "Archive", "Delete", "Manage", "Edit Achievement"];
            for (int index = 0; index < labels.Length; index++)
                if (await dialog.GetByRole(AriaRole.Button, new() { Name = labels[index], Exact = true })
                    .CountAsync().ConfigureAwait(false) > 0) actions |= 1 << index;
            actions |= await dialog.EvaluateAsync<int>("""
                element => {
                    const text = (element.innerText || '').slice(0, 20000).toLowerCase();
                    let mask = 0;
                    if (text.includes('permission')) mask |= 1 << 12;
                    if (text.includes('read-only') || text.includes('read only')) mask |= 1 << 13;
                    if (text.includes('achievement')) mask |= 1 << 14;
                    if (text.includes('welcome') || text.includes("what's new")) mask |= 1 << 15;
                    return mask;
                }
                """).ConfigureAwait(false);
            int cardButtons = await page.GetByText(apiName, new() { Exact = true })
                .Locator("xpath=../..").GetByRole(AriaRole.Button).CountAsync().ConfigureAwait(false);
            if (cardButtons == 1) actions |= 1 << 16;
            else if (cardButtons > 1) actions |= 1 << 17;
            SetSyncDiagnosticWriteState($"modern-editor-actions-{actions}");
        }
        return null;
    }
}
