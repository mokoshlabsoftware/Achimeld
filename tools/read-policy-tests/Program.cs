using System.Text.Json;
using Achimeld.ReadWorker;

int cases = 0;
static JsonElement Json(string text) => JsonSerializer.Deserialize<JsonElement>(text);
ReadOnlyPolicy Started() { var policy = new ReadOnlyPolicy(); policy.Validate("start", Json("null"), false); return policy; }
void Reject(ReadOnlyPolicy policy, string operation, string args, bool development = false)
{
    try { policy.Validate(operation, Json(args), development); }
    catch (InvalidDataException) { cases++; return; }
    throw new Exception($"Unexpectedly accepted {operation}");
}
foreach (string operation in new[] { "applyOperation", "downloadArtwork", "compareIcons", "readDocument", "evaluate", "cancelRead", "unknown" })
    Reject(Started(), operation, "null", true);
foreach (string route in new[] { "http://partner.steamgames.com/apps/achievements/480", "https://partner.steamgames.com.evil/apps/achievements/480",
    "https://partner.steamgames.com/apps/publishing/480", "https://partner.steamgames.com/apps/achievements/0480",
    "https://partner.steamgames.com/apps/achievements/480?x=1", "https://partner.steamgames.com/apps/achievements/480/",
    "https://partner.steamgames.com/apps/achievements/0", "https://user@partner.steamgames.com/apps/achievements/480" })
    Reject(Started(), "navigate", JsonSerializer.Serialize(route));
Reject(new(), "networkSelfTest", "true");
Reject(new(), "navigate", "\"https://partner.steamgames.com/apps/achievements/480\"");
Reject(Started(), "start", "null");
Reject(Started(), "stop", "{}");
Reject(Started(), "readConfiguration", "{\"value\":480}");
var pinned = Started();
try { pinned.RequireSynchronizationApp(); throw new Exception("Unpinned synchronization accepted"); } catch (InvalidDataException) { cases++; }
pinned.Validate("navigate", Json("\"https://partner.steamgames.com/apps/achievements/480\""), false);
if (pinned.RequireSynchronizationApp() != 480) throw new Exception("Synchronization App ID pin lost"); cases++;
if (pinned.Validate("readConfiguration", Json("{\"value\":480}"), false).AppId != 480) throw new Exception("Pin lost"); cases++;
Reject(pinned, "navigate", "\"https://partner.steamgames.com/apps/achievements/481\"");
foreach (string argument in new[] { "{\"value\":481}", "{\"value\":0}", "{\"value\":480,\"extra\":1}", "{\"value\":480,\"value\":481}", "{\"value\":\"480\"}" })
    Reject(pinned, "readConfiguration", argument);
pinned.Validate("stop", Json("null"), false);
try { pinned.RequireSynchronizationApp(); throw new Exception("Stopped synchronization accepted"); } catch (InvalidDataException) { cases++; }
Reject(pinned, "start", "null");
var probe = new ReadOnlyPolicy(); probe.Validate("networkSelfTest", Json("true"), true); cases++;
Console.WriteLine($"PASS: {cases} receiving read-policy cases; no browser or Steam session used.");
