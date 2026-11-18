using Achimeld.Browser;
using Achimeld.Core;

internal static class ModernAchievementPageFixtures
{
    internal static int Run()
    {
        const string appInfo = "https://partner.steamgames.com/achievements/ajaxgetappinfo/4873800";
        const string dlc = "https://partner.steamgames.com/achievements/ajaxgetdlc/4873800";
        const string schema = "https://partner.steamgames.com/achievements/ajaxgetstatsschema/4873800";
        const string languages = "https://partner.steamgames.com/achievements/ajaxgetlanguagelist/4873800";
        const string options = "https://partner.steamgames.com/achievements/ajaxgetlanguageoptionslist/4873800";
        const string percentages = "https://partner.steamgames.com/um/Player/GetGlobalAchievementPercentages/?input_protobuf_encoded=CMi8qQI%3D";
        const string playerAchievements = "https://partner.steamgames.com/um/Player/GetGameAchievements/?input_protobuf_encoded=CMi8qQISB2VuZ2xpc2g%3D";
        SteamBrowserRequestPolicy policy = new();
        int checks = 0;
        CheckReviewedModernDraft();
        void Check(bool expected, string method, string url, string resource, bool navigation = false)
        {
            if (policy.Allows(method, new Uri(url), resource, navigation) != expected)
                throw new Exception("Modern achievements request policy changed.");
            checks++;
        }

        policy.BeginAuthentication();
        Check(false, "GET", appInfo, "xhr");
        Check(false, "GET", dlc, "xhr");
        Check(false, "GET", schema, "xhr");
        Check(false, "GET", percentages, "fetch");
        policy.PinApp(4873800);
        const string storeLookup = "https://partner.steamgames.com/um/StoreBrowse/GetItems/?input_protobuf_encoded=CAI%3D";
        Check(false, "GET", storeLookup, "fetch");
        if (!policy.DiscardsOptionalStoreBrowse("GET", new Uri(storeLookup), "fetch", false) ||
            policy.DiscardsOptionalStoreBrowse("POST", new Uri(storeLookup), "fetch", false) ||
            policy.DiscardsOptionalStoreBrowse("GET", new Uri(storeLookup), "xhr", false) ||
            policy.DiscardsOptionalStoreBrowse("GET", new Uri(storeLookup + "&extra=1"), "xhr", false) ||
            policy.DiscardsOptionalStoreBrowse("GET", new Uri(storeLookup.Replace("StoreBrowse", "Account")), "fetch", false))
            throw new Exception("Optional store lookup must be discarded locally only on the exact GET/fetch route.");
        checks += 5;
        for (int i = 0; i < 15; i++)
            if (!policy.DiscardsOptionalStoreBrowse("GET", new Uri(storeLookup), "fetch", false))
                throw new Exception("Bounded optional store lookup changed.");
        if (policy.DiscardsOptionalStoreBrowse("GET", new Uri(storeLookup), "fetch", false))
            throw new Exception("Optional store lookup exceeded its discard budget.");
        checks += 16;
        foreach (string route in new[] { appInfo, dlc, schema, languages, options })
        {
            Check(true, "GET", route, "xhr");
            Check(false, "GET", route, "fetch");
            Check(false, "GET", route, "document", true);
            Check(false, "POST", route, "xhr");
            Check(false, "GET", route + "?appid=4873801", "xhr");
            Check(false, "GET", route + "/", "xhr");
            Check(false, "GET", route.Replace("4873800", "4873801"), "xhr");
        }
        foreach (string route in new[] { percentages, playerAchievements })
        {
            Check(true, "GET", route, "fetch");
            Check(false, "GET", route, "xhr");
            Check(false, "GET", route, "document", true);
            Check(false, "POST", route, "fetch");
            Check(false, "GET", route + "&extra=1", "fetch");
            Check(false, "GET", route.Replace("CMi8qQI", "CMm8qQI"), "fetch");
            Check(false, "GET", route.Replace("/um/Player/", "/um/Account/"), "fetch");
        }
        policy.EnterRead();
        Check(true, "GET", appInfo, "xhr");
        Check(true, "GET", appInfo, "fetch");
        Check(false, "GET", appInfo + "?other=1", "fetch");
        Check(false, "GET", appInfo.Replace("4873800", "4873801"), "fetch");
        Check(true, "GET", dlc, "xhr");
        Check(true, "GET", schema, "xhr");
        Check(true, "GET", schema, "fetch");
        Check(false, "GET", schema + "?other=1", "fetch");
        Check(false, "GET", schema.Replace("4873800", "4873801"), "fetch");
        if (policy.ConfigurationUri(4873800, "ajaxgetstatsschema").AbsoluteUri != schema)
            throw new Exception("Pinned modern schema route changed.");
        checks++;
        if (policy.ConfigurationUri(4873800, "ajaxgetappinfo").AbsoluteUri != appInfo)
            throw new Exception("Pinned modern app-info route changed.");
        checks++;
        SteamworksSchemaOutline shape = SteamworksSchemaShape.Describe("{\"success\":true,\"data\":{\"achievements\":[{\"api_name\":\"PRIVATE_NAME\"}]}}");
        if (shape.Mask != ((1u << 0) | (1u << 2) | (1u << 3) | (1u << 11) | (1u << 13)) ||
            shape.Kind != "missing" || shape.LargestArray != 1 || shape.ObjectCount != 3 ||
            SteamworksSchemaShape.Describe("{\"success\":true,\"data\":{\"achievements\":[{\"api_name\":\"ANOTHER_NAME\"}]}}") != shape)
            throw new Exception("Schema diagnostic must expose fixed structure only.");
        checks++;
        SteamworksSchemaOutline containers = SteamworksSchemaShape.Describe("{\"schema\":{\"stats\":{},\"achievements\":[]}}");
        if (containers.Kind != "object" || containers.ItemCount != 2 ||
            containers.ContainerMask != 3 || containers.ContainerObjects != 1 || containers.ContainerArrays != 1)
            throw new Exception("Schema container outline changed.");
        checks++;
        Check(true, "GET", percentages, "fetch");
        string definitionsCsv = "api_name,setid,permission,spoiler,archived,progress_stat_name,progress_stat_min,progress_stat_max\r\nACH_ONE,,Client,false,false,,,\r\n";
        string localizationCsv = "\uFEFFapi_name,field,english\r\nACH_ONE,name,First\r\nACH_ONE,description,Description\r\n";
        string oneAchievement = System.Text.Encoding.UTF8.GetString(
            SteamworksModernLocalizationImport.BuildOneAchievement(localizationCsv, "ACH_ONE", "name", "New Name"));
        if (oneAchievement != "api_name,field,english\r\nACH_ONE,name,New Name\r\n")
            throw new Exception("Localization import must include only the reviewed changed row.");
        IReadOnlySet<string> knownNames = SteamworksModernLocalizationImport.KnownApiNames(localizationCsv);
        if (knownNames.Count != 1 || !knownNames.Contains("ACH_ONE"))
            throw new Exception("Localization export API identities were not parsed safely.");
        checks++;
        SteamworksCsvExportData export = new(definitionsCsv, localizationCsv,
            new(1, 2, 255, 7, 17));
        Dictionary<string, ModernAchievementIconPair> icons = new(StringComparer.Ordinal)
        {
            ["ACH_ONE"] = new("https://shared.akamai.steamstatic.com/community_assets/images/apps/4873800/achieved.png",
                "https://shared.akamai.steamstatic.com/community_assets/images/apps/4873800/unachieved.png"),
        };
        string json = ModernSteamworksConfiguration.AchievementJson(export, icons);
        _ = ModernSteamworksConfiguration.AchievementJson(export with
            { DefinitionsCsv = definitionsCsv.Replace(",false,false,,,", ",false,False,,,") }, icons);
        _ = ModernSteamworksConfiguration.AchievementJson(export with
            { DefinitionsCsv = definitionsCsv.Replace("ACH_ONE,,Client", "ACH_ONE,0,Client") }, icons);
        string assignedJson = ModernSteamworksConfiguration.AchievementJson(export with
            { DefinitionsCsv = definitionsCsv.Replace("ACH_ONE,,Client", "ACH_ONE,17,Client") }, icons);
        ManagedPageDocument assigned = ModernSteamworksConfiguration.RemoveSyntheticAchievementIds(
            LiveSteamworksConfiguration.Normalize(new SteamAppId(4873800), "[]", assignedJson));
        if (!assigned.Html.Contains("data-set-id=\"17\"", StringComparison.Ordinal))
            throw new Exception("Steam achievement set assignment was not retained.");
        ManagedPageDocument normalized = ModernSteamworksConfiguration.RemoveSyntheticAchievementIds(
            LiveSteamworksConfiguration.Normalize(new SteamAppId(4873800), "[]", json));
        if (!normalized.Html.Contains("data-api-name=\"ACH_ONE\"", StringComparison.Ordinal) ||
            normalized.Html.Contains("data-steam-id=\"0_0\"", StringComparison.Ordinal))
            throw new Exception("Modern CSV snapshot adapter omitted an achievement or leaked a synthetic ID.");
        checks++;
        string zeroDefaults = definitionsCsv.Replace("false,,,", "false,,0,0");
        _ = ModernSteamworksConfiguration.AchievementJson(export with { DefinitionsCsv = zeroDefaults }, icons);
        checks++;
        bool orphanRangeRejected = false;
        try { ModernSteamworksConfiguration.AchievementJson(export with
            { DefinitionsCsv = definitionsCsv.Replace("false,,,", "false,,0,1") }, icons); }
        catch (UnsupportedPortalStructureException) { orphanRangeRejected = true; }
        if (!orphanRangeRejected) throw new Exception("A nonzero progress range without a stat must be rejected.");
        checks++;
        SteamBrowserRequestPolicy reviewedPage = new();
        reviewedPage.BeginAuthentication();
        reviewedPage.PinApp(4873800);
        reviewedPage.EnterRead();
        reviewedPage.BeginReviewedDraft(4873800, PortalEntityKind.Achievement);
        if (!reviewedPage.DiscardsOptionalStoreBrowse("GET", new Uri(storeLookup), "fetch", false))
            throw new Exception("The modern editor's optional store lookup must stay local during a reviewed draft.");
        checks++;
        Uri modernSave = new("https://partner.steamgames.com/achievements/ajaxcreateorupdateachievement/4873800");
        int writeShape = SteamworksModernWriteShape.Read("POST", modernSave, "xhr",
            "application/x-www-form-urlencoded", "api_name=ACH_ONE&english=First+Name&setid=17",
            4873800, "ACH_ONE", "First Name");
        if ((writeShape & 1) == 0 || (writeShape & 32) == 0 || (writeShape & 64) == 0 ||
            (writeShape & 256) == 0 || (writeShape & 512) == 0 ||
            SteamworksModernWriteShape.Read("POST", new Uri(modernSave + "?other=1"), "xhr",
                "application/json", "{}", 4873800, "ACH_ONE", "First Name") != 0)
            throw new Exception("Modern achievement write diagnostics did not remain pinned and bounded.");
        checks++;
        int structure = SteamworksModernWriteShape.ReadStructure(null,
            "{\"api_name\":\"ACH_ONE\",\"set_id\":17,\"progressstat\":null}");
        if ((structure & 1) == 0 || (structure & 2) == 0 || (structure & 4) == 0 ||
            (structure & 8) == 0 || (structure & 128) == 0 || (structure & 1024) == 0 ||
            SteamworksModernWriteShape.ReadStructure(null, new string('x', 65_537)) != 0)
            throw new Exception("Modern write structure diagnostics exceeded their fixed shape.");
        checks++;
        string modernBody = "apiName=ACHIEVEMENT_ONE&English=First+Name&progressStat=";
        const string modernType = "multipart/form-data; boundary=fixture";
        if (SteamworksModernWriteShape.ReadGuardConditions(modernType, modernBody,
                "ACHIEVEMENT_ONE") != 3 ||
            SteamworksModernWriteShape.ReadGuardConditions("multipart/form-data", modernBody,
                "ACHIEVEMENT_ONE") != 2 ||
            SteamworksModernWriteShape.ReadGuardConditions(modernType,
                modernBody + "&other=ACHIEVEMENT_TWO", "ACHIEVEMENT_ONE") != 1 ||
            SteamworksModernWriteShape.ReadIdentityShape(modernBody, "ACHIEVEMENT_ONE") != 265 ||
            SteamworksModernWriteShape.ReadIdentityShape(
                modernBody + "&other=ACHIEVEMENT_TWO", "ACHIEVEMENT_ONE") != 298)
            throw new Exception("Modern save guard diagnostics did not distinguish header and identity checks.");
        checks++;
        string multipart = "--fixture\r\nContent-Disposition: form-data; name=\"api_name\"\r\n\r\nACHIEVEMENT_ONE\r\n" +
            "--fixture\r\nContent-Disposition: form-data; name=\"progressstat\"\r\n\r\nACHIEVEMENT_TWO\r\n--fixture--\r\n";
        if (SteamworksModernWriteShape.ReadIdentityLocations(modernType, multipart,
                "ACHIEVEMENT_ONE", "First Name") != (uint)(1 | (8 << 8) | (2 << 16)) ||
            SteamworksModernWriteShape.ReadIdentityLocations(null, multipart,
                "ACHIEVEMENT_ONE", "First Name") != 0 ||
            SteamworksModernWriteShape.ReadMultipartSequence(modernType, multipart,
                "ACHIEVEMENT_ONE", "First Name") != (ulong)(8 | ((3 | 16) << 6)) ||
            SteamworksModernWriteShape.ReadMultipartSequence(modernType, multipart,
                "ACHIEVEMENT_ONE", "First Name",
                new HashSet<string>(["ACHIEVEMENT_ONE", "ACHIEVEMENT_TWO"], StringComparer.Ordinal)) !=
                ((ulong)(8 | ((3 | 16) << 6)) | (1UL << 60)))
            throw new Exception("Modern multipart identity locations were not categorized safely.");
        checks++;
        reviewedPage.ArmReviewedModernAchievementPost(4873800, "ACHIEVEMENT_ONE", "First Name");
        if (reviewedPage.Allows("POST", modernSave, "xhr", false,
            modernBody.Replace("First+Name", "Other+Name"), modernType) ||
            !reviewedPage.Allows("POST", modernSave, "xhr", false, modernBody, modernType) ||
            reviewedPage.Allows("POST", modernSave, "xhr", false, modernBody, modernType))
            throw new Exception("Modern achievement write scope allowed a wrong, absent, or repeated save.");
        reviewedPage.EndReviewedDraft();
        checks++;
        string ImportPart(string name, string value) =>
            $"--fixture\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n";
        string importBody = ImportPart("progressstat", "") +
            ImportPart("nonce", "English") + ImportPart("language", "") +
            ImportPart("name", "ACHIEVEMENT_ONE") +
            ImportPart("progressstat_min", "") + ImportPart("progressstat_max", "") +
            ImportPart("progressstat_display", "") +
            ImportPart("displayname", "ACHIEVEMENT_INTERNAL First Name") + "--fixture--\r\n";
        IReadOnlySet<string> knownImport = new HashSet<string>(["ACHIEVEMENT_ONE"],
            StringComparer.Ordinal);
        if (!SteamworksModernWriteShape.AllowsReviewedLocalizationImport("POST", modernSave,
                "xhr", modernType, importBody, 4873800, "ACHIEVEMENT_ONE", "First Name",
                knownImport) ||
            SteamworksModernWriteShape.AllowsReviewedLocalizationImport("POST", modernSave,
                "xhr", modernType, importBody, 4873800, "ACHIEVEMENT_ONE", "First Name",
                new HashSet<string>(["ACHIEVEMENT_ONE", "ACHIEVEMENT_INTERNAL"],
                    StringComparer.Ordinal)) ||
            SteamworksModernWriteShape.AllowsReviewedLocalizationImport("POST", modernSave,
                "xhr", modernType, importBody.Replace("First Name", "Wrong Name"),
                4873800, "ACHIEVEMENT_ONE", "First Name", knownImport))
            throw new Exception("Reviewed localization import body scope changed.");
        checks++;
        reviewedPage.BeginReviewedDraft(4873800, PortalEntityKind.Achievement);
        reviewedPage.ArmReviewedModernAchievementPost(4873800, "ACHIEVEMENT_ONE",
            "First Name", knownImport, localizationImport: true);
        if (!reviewedPage.Allows("POST", modernSave, "xhr", false, importBody, modernType) ||
            reviewedPage.Allows("POST", modernSave, "xhr", false, importBody, modernType))
            throw new Exception("Modern import write scope was not consumed exactly once.");
        reviewedPage.EndReviewedDraft();
        checks++;
        bool archivedRejected = false;
        try { ModernSteamworksConfiguration.AchievementJson(export with
            { DefinitionsCsv = definitionsCsv.Replace(",false,false,,,", ",false,true,,,") }, icons); }
        catch (UnsupportedPortalStructureException) { archivedRejected = true; }
        if (!archivedRejected) throw new Exception("Archived achievements must not be silently flattened.");
        checks++;
        policy.Reject();
        Check(false, "GET", appInfo, "xhr");
        Check(false, "GET", dlc, "xhr");
        Check(false, "GET", schema, "xhr");
        Check(false, "GET", percentages, "fetch");
        return checks;
    }

    private static void CheckReviewedModernDraft()
    {
        var records = ModernAchievementDraft.ReadRecords("""
            {"success":1,"schema":{"stats":{"9":{"type":"ACHIEVEMENTS","bits":{"3":{"name":"ACH_ONE","groupid":"17","display":{"desc":{"token":"ACH_ONE_DESC"}}}}}}}}
            """);
        var record = records["ACH_ONE"];
        if (record.Identity != "9_3" || record.SetId != "17" || record.DescriptionToken != "ACH_ONE_DESC")
            throw new Exception("The draft identity did not match the pinned schema record.");
        var array = ModernAchievementDraft.ReadRecords("""
            {"schema":{"stats":[null,{"type":"ACHIEVEMENTS","bits":[null,{"name":"ACH_ONE","groupid":0}]}]}}
            """);
        if (array["ACH_ONE"] is not { Identity: "1_1", SetId: null })
            throw new Exception("Array positions must retain their Steam stat and bit identities.");
        var definition = ModernAchievementDraft.Definition("ACH_ONE", record, new Dictionary<string, string>
        {
            ["hidden"] = "false", ["setBy"] = "GameClient", ["progressStatApiName"] = "SCORE", ["unlockValue"] = "145",
        });
        var move = new Dictionary<string, string> { ["appid"] = "480", ["groupid"] = "0", ["names"] = "[\"ACH_ONE\"]" };
        const string contentType = "multipart/form-data; boundary=fixture";
        static string Body(Dictionary<string, string> fields) => string.Concat(fields.Select(field =>
            $"--fixture\r\nContent-Disposition: form-data; name=\"{field.Key}\"\r\n\r\n{field.Value}\r\n")) + "--fixture--\r\n";
        var policy = new SteamBrowserRequestPolicy();
        policy.PinApp(480);
        policy.EnterRead();
        foreach (bool moving in new[] { false, true })
        {
            policy.BeginReviewedDraft(480, PortalEntityKind.Achievement);
            var fields = moving ? move : definition;
            Uri uri = policy.ArmReviewedModernForm(480, moving, fields);
            string body = Body(fields);
            if (policy.Allows("POST", uri, "fetch", true, body, contentType) ||
                policy.Allows("POST", new Uri(uri.AbsoluteUri.Replace("/480", "/481")), "fetch", false, body, contentType) ||
                policy.Allows("POST", uri, "fetch", false, body.Replace("ACH_ONE", "ACH_TWO"), contentType) ||
                policy.Allows("POST", uri, "fetch", false, body.Replace(moving ? "\r\n0\r\n" : "\r\n145\r\n", "\r\n999\r\n"), contentType) ||
                !policy.Allows("POST", uri, "fetch", false, body, contentType) ||
                policy.Allows("POST", uri, "fetch", false, body, contentType))
                throw new Exception("The modern draft guard must allow only the exact reviewed form once.");
            policy.EndReviewedDraft();
        }
        policy.BeginReviewedDraft(480, PortalEntityKind.Icon);
        byte[] image = [0, 255, 123, 10];
        const string fileName = "480_9_3_a.png";
        var icon = new Dictionary<string, string> { ["appID"] = "480", ["statID"] = "9", ["bit"] = "3", ["requestType"] = "achievement" };
        Uri iconUri = policy.ArmReviewedModernIcon(480, icon, fileName, image);
        byte[] partHeader = System.Text.Encoding.ASCII.GetBytes(Body(icon).Replace("--fixture--\r\n",
            $"--fixture\r\nContent-Disposition: form-data; name=\"image\"; filename=\"{fileName}\"\r\nContent-Type: image/png\r\n\r\n"));
        byte[] payload = [.. partHeader, .. image, .. "\r\n--fixture--\r\n"u8];
        byte[] changed = (byte[])payload.Clone();
        changed[partHeader.Length + 1] ^= 1;
        if (policy.Allows("POST", iconUri, "fetch", false, null, contentType, changed) ||
            !policy.Allows("POST", iconUri, "fetch", false, null, contentType, payload) ||
            policy.Allows("POST", iconUri, "fetch", false, null, contentType, payload))
            throw new Exception("Only the exact reviewed modern icon bytes may be sent once.");
        policy.EndReviewedDraft();
    }
}
