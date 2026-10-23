using System.Buffers.Binary;
using Achimeld.Browser;
using Achimeld.Security;

int checks = 0;
checks += WorkerDiagnosticFixtures.Run();
checks += GameBannerFixtures.Run();
void Reject(Action action) { try { action(); } catch (Exception e) when (e is InvalidDataException or ArgumentException) { checks++; return; } throw new Exception("Unsafe input accepted."); }
foreach (string url in new[] { "https://cdn.steamstatic.com/steamcommunity/public/images/apps/480/a.png", "https://steamcommunity.com/public/images/apps/480/a.jpg", "https://partner.steamgames.com/public/images/a.png" }) { BrowserReadPolicy.RequireIcon(new Uri(url)); checks++; }
foreach (string url in new[] { "http://cdn.steamstatic.com/a.png", "https://steamstatic.com.evil.test/a.png", "https://sub.steamcommunity.com/a.png", "https://127.0.0.1/a.png", "https://user:secret@cdn.steamstatic.com/a.png", "https://cdn.steamstatic.com:8443/a.png", "https://cdn.steamstatic.com/a.png?token=secret", "https://cdn.steamstatic.com/a.png#secret", "https://login.steampowered.com/a.png", "https://checkout.steampowered.com/a.png", "https://help.steampowered.com/a.png" })
    Reject(() => BrowserReadPolicy.RequireIcon(new Uri(url)));
foreach (string url in new[] { "https://partner.steamgames.com/apps/deleteachievement/480", "https://partner.steamgames.com/images/uploadachievement", "https://steamcommunity.com/actions/delete.png", "https://cdn.steamstatic.com/a.json", "https://partner.steamgames.com/public/images/a%2fpng.png" })
    Reject(() => BrowserReadPolicy.RequireIcon(new Uri(url)));
SteamBrowserRequestPolicy policy = new();
void Request(bool expected, string method, string url, string type = "fetch", bool navigation = false)
{
    if (policy.Allows(method, new Uri(url), type, navigation) != expected) throw new Exception("Unexpected browser request decision.");
    checks++;
}
Request(false, "GET", "https://partner.steamgames.com/", "document", true);
policy.BeginAuthentication();
Request(false, "GET", "https://partner.steamgames.com/apps/getachievementcounts/480");
Request(true, "POST", "https://login.steampowered.com/jwt/finalizelogin");
Request(true, "POST", "https://steamcommunity.com/openid/login");
Request(true, "GET", "https://store.steampowered.com/login/?redir=partner", "document", true);
Request(false, "POST", "https://partner.steamgames.com/apps/setachievement/480");
Request(false, "GET", "https://partner.steamgames.com/apps/achievements/480", "document", true);
Reject(() => policy.ConfigurationUri(480, "fetchstats"));
policy.PinApp(480);
Request(true, "GET", "https://partner.steamgames.com/apps/getachievementcounts/480", "xhr");
Request(true, "GET", "https://partner.steamgames.com/apps/achievements/480", "document", true);
Request(false, "GET", "https://partner.steamgames.com/apps/achievements/481", "document", true);
Reject(() => policy.PinApp(481));
policy.EnterRead();
Request(true, "GET", "https://partner.steamgames.com/apps/getachievementcounts/480", "xhr");
foreach (string route in new[] { "https://partner.steamgames.com/apps/getachievementcounts/481", "https://partner.steamgames.com/apps/getachievementcounts/480?appid=481", "https://partner.steamgames.com/apps/getachievementcounts/480/" }) Request(false, "GET", route, "xhr");
Request(false, "POST", "https://partner.steamgames.com/apps/getachievementcounts/480", "xhr");
Request(false, "GET", "https://partner.steamgames.com/apps/getachievementcounts/480", "document", true);
if (policy.GameBannerUri(480).AbsoluteUri != "https://cdn.akamai.steamstatic.com/steam/apps/480/capsule_231x87_2x.jpg") throw new Exception("Game banner route changed."); checks++;
Reject(() => policy.GameBannerUri(0));
Reject(() => policy.GameBannerUri(481));
if (policy.ConfigurationUri(480, "fetchstats").AbsoluteUri != "https://partner.steamgames.com/apps/fetchstats/480") throw new Exception("Configuration route changed."); checks++;
Reject(() => policy.ConfigurationUri(481, "fetchstats"));
Reject(() => policy.ConfigurationUri(480, "deleteachievement"));
Reject(policy.BeginAuthentication);
foreach (string route in new[] { "https://partner.steamgames.com/apps/fetchstats/480", "https://partner.steamgames.com/apps/fetchachievements/480" }) Request(true, "GET", route);
foreach (string route in new[] { "https://partner.steamgames.com/apps/fetchstats/481", "https://partner.steamgames.com/apps/fetchstats/480?appid=481", "https://partner.steamgames.com/apps/deleteachievement/480", "https://partner.steamgames.com/images/uploadachievement", "https://login.steampowered.com/jwt/finalizelogin", "https://store.steampowered.com/login/", "http://partner.steamgames.com/apps/fetchstats/480" })
    foreach (string method in new[] { "GET", "POST", "PUT", "DELETE" }) Request(false, method, route);
Request(true, "GET", "https://partner.steamgames.com/public/javascript/a.js?v=1", "script");
Request(true, "GET", "https://partner.steamgames.com/public/javascript/applications/appmgmt/libraries~b28b7af69.js", "script");
Request(false, "GET", "https://partner.steamgames.com/public/javascript/applications/appmgmt/libraries%2Fb28b7af69.js", "script");
Request(false, "POST", "https://partner.steamgames.com/public/javascript/applications/appmgmt/libraries~b28b7af69.js", "script");
Request(false, "GET", "https://partner.steamgames.com/apps/deleteachievement/480~1", "script");
Request(true, "GET", "https://cdn.steamstatic.com/a.png", "image");
Request(false, "GET", "https://store.steampowered.com/public/a.js", "script");
Request(false, "GET", "https://partner.steamgames.com/public/a.js", "document", true);
policy.Reject();
Reject(policy.RequireHealthy);
Request(false, "GET", "https://partner.steamgames.com/apps/fetchstats/480");
var initial = new SupervisionStatus("nonce", new string('A',64), 42, "0.1.0", "development", true, true, "coverage", 42761, "browser", "profile", 42763);
SupervisionSession.ValidateContinuity(initial, initial with { Nonce = "another nonce" }); checks++;
foreach (SupervisionStatus changed in new[] { initial with { SupervisorPid=43 }, initial with { SessionId="different" }, initial with { PackageVersion="0.2.0" }, initial with { PackageMode="official" }, initial with { NetworkEnforced=false }, initial with { ProxyPort=4444 }, initial with { ProbePort=5555 }, initial with { BrowserExecutable="other" }, initial with { ProfileDirectory="other" }, initial with { MonitoringCoverage="other" } })
    Reject(() => SupervisionSession.ValidateContinuity(initial, changed));
var userSession = initial with { NetworkEnforced=false };
SupervisionSession.ValidateContinuity(userSession, userSession with { Nonce="fresh nonce" }); checks++;
Reject(() => SupervisionSession.ValidateContinuity(userSession, initial));
foreach (SupervisionStatus unhealthy in new[] { initial with { MonitoringActive=false }, initial with { FailureCode="lost" }, userSession with { MonitoringActive=false }, userSession with { FailureCode="lost" } }) {
    var previous = unhealthy.NetworkEnforced ? initial : userSession;
    try { SupervisionSession.ValidateContinuity(previous, unhealthy); throw new Exception("Unhealthy session accepted."); } catch (InvalidOperationException) { checks++; }
}
byte[] oversized = new byte[4]; BinaryPrimitives.WriteInt32LittleEndian(oversized, 65537);
try { await SecurityWire.ReadAsync<SupervisionStatus>(new MemoryStream(oversized), CancellationToken.None, 65536); throw new Exception("Oversized supervision frame accepted."); } catch (InvalidDataException) { checks++; }
string local = Path.Combine(Path.GetTempPath(), "Achimeld-profile-policy-fixture");
string expected = Path.Combine(local,"Achimeld","Browser","ChromeProfile");
if (DedicatedProfilePolicy.Resolve(null,local) != expected) throw new Exception("Dedicated profile selection changed."); checks++;
Reject(() => DedicatedProfilePolicy.Resolve(Path.Combine(local,"OtherProduct","Browser","ChromeProfile"),local));
Console.WriteLine($"{checks} migrated browser/security policy checks passed.");
await AchievementTextFixtures.RunAsync();
VignetteFixtures.Run();
ParticleFixtures.Run();
ExplosionFixtures.Run();
PerlinFixtures.Run();
MaskReferenceFixtures.Run();
SpatialEffectFixtures.Run();
RadialBlurFixtures.Run();
Text3dFixtures.Run();
Artifact3dFixtures.Run();
await SynchronizationFixtures.RunAsync();
if (args.Length != 0) await BrowserFixtures.RunAsync(args);
