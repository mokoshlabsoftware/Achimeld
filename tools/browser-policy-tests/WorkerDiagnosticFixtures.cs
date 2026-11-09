using Achimeld.Browser;
using Achimeld.ReadWorker;

internal static class WorkerDiagnosticFixtures
{
    internal static int Run()
    {
        const string secret = "https://secret.invalid/?token=private password=hidden";
        var detail = WorkerFailure.Details("operation", new InvalidDataException(secret), 12,
            BrowserRequestDiagnostic.Describe("POST", "https://api.steampowered.com/IAuthenticationService/BeginAuthSessionViaQR/v1?secret=hidden", false, "fetch"));
        var requestDetail = detail.GetProperty("request");
        if (requestDetail.GetProperty("host").GetString() != "api" || requestDetail.GetProperty("path").GetString() != "authentication-api" ||
            requestDetail.GetProperty("method").GetString() != "POST" || detail.GetProperty("elapsedMs").GetInt64() != 12 ||
            detail.GetRawText().Contains("hidden", StringComparison.Ordinal))
            throw new Exception("Production failure details lost wire casing or exposed request data.");
        var sync = WorkerFailure.Details("operation", new TimeoutException(secret), 35000, null,
            new SyncOperationDiagnostic("write-precheck", 30000, "fetchachievements", "none", "none"));
        if (sync.GetProperty("sync").GetProperty("endpoint").GetString() != "fetchachievements" ||
            sync.GetProperty("sync").GetProperty("phaseElapsedMs").GetInt64() != 30000 ||
            sync.GetRawText().Contains(secret, StringComparison.Ordinal))
            throw new Exception("Synchronization diagnostics lost closed phase information or exposed exception text.");
        (Exception Error, string Code)[] cases = [
            (new InvalidDataException("The synchronization snapshot has inconsistent identities, ownership or fingerprints."), "steam.sync.snapshot-invalid"),
            (new InvalidDataException("Reviewed synchronization command rejected."), "steam.sync.command-invalid"),
            (new InvalidDataException("A pending API rename conflicts with Steam. Reconcile or revert the rename before synchronizing."), "steam.sync.rename-conflict"),
            (new InvalidDataException("Project and Steam API names differ only by case; align their spelling before synchronizing."), "steam.sync.name-case-conflict"),
            (new InvalidDataException("Steam changed after review; prepare a new synchronization plan."), "steam.sync.review-changed"),
            (new InvalidDataException("Steam changed during synchronization; prepare a fresh plan."), "steam.sync.remote-changed"),
            (new InvalidDataException("Steam draft read-back did not match the approved change."), "steam.sync.readback-mismatch"),
            (new InvalidOperationException("Steamworks did not confirm the draft save."), "steam.sync.save-unconfirmed"),
            (new InvalidOperationException("Steamworks did not confirm the draft deletion."), "steam.sync.delete-unconfirmed"),
            (new InvalidOperationException("Steamworks rejected the achievement icon upload."), "steam.sync.icon-post-rejected"),
            (new InvalidOperationException("Steamworks did not save the achievement icon."), "steam.sync.icon-not-saved"),
            (new InvalidOperationException("Steamworks saved icon artwork that does not match the approved upload."), "steam.sync.icon-verify-failed"),
            (new InvalidOperationException("Steamworks still serves the pre-upload achievement icon after the upload."), "steam.sync.icon-stale"),
            (new InvalidOperationException("Steamworks saved an icon with unexpected image dimensions or format."), "steam.sync.icon-format"),
            (new InvalidOperationException("Steamworks saved icon pixels close to but outside the approved threshold."), "steam.sync.icon-pixels-near"),
            (new InvalidOperationException("Steamworks saved icon pixels substantially different from the approved upload."), "steam.sync.icon-pixels-different"),
            (new InvalidDataException("Steam did not preserve the renamed record identity."), "steam.sync.rename-identity"),
            (new InvalidDataException("A localization row is missing."), "steam.sync.localization-missing"),
            (new InvalidDataException("Select the progress stat before its dependent achievement."), "steam.sync.progress-selection"),
            (new InvalidDataException("An icon upload requires its achievement definition."), "steam.sync.icon-selection"),
            (new InvalidDataException("An unrelated Steam value changed during the draft save."), "steam.sync.unrelated-change"),
            (new InvalidDataException("Steam artwork changed after review; prepare a fresh plan."), "steam.sync.artwork-changed"),
            (new InvalidDataException("Imported artwork exceeds 8 MiB."), "steam.sync.artwork-size"),
            (new InvalidDataException("Import the progress stat first."), "steam.sync.progress-import"),
            (new OperationCanceledException(secret), "security.operation.interrupted"),
            (new System.TimeoutException(secret), "steam.operation.timeout"),
            (new Achimeld.Core.IncompleteRemoteSnapshotException(), "steam.snapshot.incomplete"),
            (new FileNotFoundException(secret), "steam.runtime.missing"),
            (new DirectoryNotFoundException(secret), "steam.runtime.missing"),
            (new UnauthorizedAccessException(secret), "steam.runtime.access-denied"),
            (new System.Text.Json.JsonException(secret), "steam.project.invalid"),
            (new PortalSafetyException(secret, secret), "steam.portal.rejected"),
            (new UnsupportedPortalStructureException(secret), "steam.portal.unsupported"),
            (new Microsoft.Playwright.PlaywrightException("net::ERR_CONNECTION_RESET " + secret), "steam.network.connection"),
            (new Microsoft.Playwright.PlaywrightException("net::ERR_FAILED " + secret), "steam.network.failed"),
            (new Microsoft.Playwright.PlaywrightException("net::ERR_TIMED_OUT " + secret), "steam.network.timeout"),
            (new Microsoft.Playwright.PlaywrightException("net::ERR_NETWORK_CHANGED " + secret), "steam.network.changed"),
            (new Microsoft.Playwright.PlaywrightException("net::ERR_INTERNET_DISCONNECTED " + secret), "steam.network.disconnected"),
            (new Microsoft.Playwright.PlaywrightException("net::ERR_CONNECTION_CLOSED " + secret), "steam.network.connection"),
            (new Microsoft.Playwright.PlaywrightException("net::ERR_CONNECTION_REFUSED " + secret), "steam.network.connection"),
            (new Microsoft.Playwright.PlaywrightException("net::ERR_HTTP_RESPONSE_CODE_FAILURE " + secret), "steam.network.http"),
            (new Microsoft.Playwright.PlaywrightException("net::ERR_ABORTED " + secret), "steam.navigation.interrupted"),
            (new Microsoft.Playwright.PlaywrightException("Navigation to " + secret + " is interrupted by another navigation"), "steam.navigation.interrupted"),
            (new Microsoft.Playwright.PlaywrightException("Target page, context or browser has been closed " + secret), "steam.browser.closed"),
        ];
        foreach (var test in cases)
        {
            if (WorkerFailure.Details("operation", test.Error, 12, null).GetRawText().Contains(secret, StringComparison.Ordinal))
                throw new Exception("Failure details exposed exception contents.");
            string code = WorkerFailure.Code(test.Error is InvalidOperationException ? "syncStep" : "prepareSync", test.Error);
            if (code != test.Code)
                throw new Exception($"Worker diagnostic classification changed: expected {test.Code}, got {code}.");
            if (WorkerFailure.Kind(test.Error).Contains(secret, StringComparison.Ordinal))
                throw new Exception("Worker diagnostic kind exposed raw error data.");
        }
        foreach (BrowserRequestRejection rejection in Enum.GetValues<BrowserRequestRejection>())
        {
            if (rejection != BrowserRequestRejection.None && !WorkerFailure.Code("authenticate", new Exception(secret), rejection).StartsWith("steam.request.", StringComparison.Ordinal))
                throw new Exception("Request rejection did not map to declared static diagnostics.");
            int exitCode = WorkerFailure.ExitCode(rejection);
            if (rejection == BrowserRequestRejection.None ? exitCode != 125 : exitCode is < 201 or > 216)
                throw new Exception("Monitor exit code escaped the declared diagnostic vocabulary.");
        }
        (BrowserRequestRejection Reason, string Code, int Exit)[] requestCases = [
            (BrowserRequestRejection.OtherPage, "steam.request.other-page", 201),
            (BrowserRequestRejection.Ping, "steam.request.ping", 212),
            (BrowserRequestRejection.Media, "steam.request.media", 213),
            (BrowserRequestRejection.Manifest, "steam.request.manifest", 214),
            (BrowserRequestRejection.EventSource, "steam.request.eventsource", 215),
            (BrowserRequestRejection.UnavailableFrame, "steam.request.frame-unavailable", 216),
        ];
        foreach (var test in requestCases)
            if (WorkerFailure.Code("navigate", new Exception(secret), test.Reason) != test.Code ||
                WorkerFailure.ExitCode(test.Reason) != test.Exit)
                throw new Exception("Request rejection lost its specific safe diagnostic.");
        if (WorkerFailure.Code("start", new Exception(secret)) != "steam.browser.start.failed" ||
            WorkerFailure.Code("prepareSync", new Exception(secret)) != "steam.sync.failed" ||
            WorkerFailure.Code("authenticate", new Exception(secret)) != "steam.navigation.failed" ||
            WorkerFailure.Code("navigate", new Exception(secret)) != "steam.navigation.failed" ||
            WorkerFailure.Code("readConfiguration", new Exception(secret)) != "steam.read.failed")
            throw new Exception("Worker fallback diagnostic classification changed.");
        int reads = 0;
        List<TimeSpan> waits = [];
        bool propagated = IconUploadVerification.WaitForSavedArtworkAsync(
            () => Task.FromResult(++reads == 3),
            duration => { waits.Add(duration); return Task.CompletedTask; }).GetAwaiter().GetResult();
        if (!propagated || reads != 3 || waits.Count != 2 || waits[0] != TimeSpan.FromMilliseconds(500) || waits[1] != TimeSpan.FromSeconds(1))
            throw new Exception("Icon verification did not retry only the read-back.");
        reads = 0;
        waits.Clear();
        bool exhausted = IconUploadVerification.WaitForSavedArtworkAsync(
            () => Task.FromResult(++reads > 5),
            duration => { waits.Add(duration); return Task.CompletedTask; }).GetAwaiter().GetResult();
        if (exhausted || reads != 5 || waits.Count != 4 || waits[^1] != TimeSpan.FromSeconds(4))
            throw new Exception("Icon verification retry budget changed.");
        string oldHash = new('a', 64), newHash = new('b', 64);
        if (!IconUploadVerification.IsFresh(oldHash, newHash) ||
            !IconUploadVerification.IsFresh(null, newHash) ||
            IconUploadVerification.IsFresh(oldHash, oldHash) ||
            IconUploadVerification.IsFresh(oldHash.ToUpperInvariant(), oldHash) ||
            IconUploadVerification.IsFresh(oldHash, null) ||
            IconUploadVerification.IsFresh(oldHash, "invalid"))
            throw new Exception("Remote icon hash freshness check changed.");
        return cases.Length + 6;
    }
}
