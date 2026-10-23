using System.Globalization;
using System.Text.Json;

namespace Achimeld.ReadWorker;

internal enum ReadOperation { Start, Authenticate, AuthenticationState, Navigate, ReadConfiguration, Stop, NetworkSelfTest }
internal readonly record struct ReadAction(ReadOperation Operation, uint AppId = 0, bool Hold = false)
{
    internal string Route => $"https://partner.steamgames.com/apps/achievements/{AppId.ToString(CultureInfo.InvariantCulture)}";
}

// SECURITY(SEC-READ-001): Receiving-boundary policy, independent of the Rust API.
// Legacy read/lifecycle dispatch. Reviewed synchronization is independently
// received by SynchronizationSession; raw mutation/download operations stay absent.
internal sealed class ReadOnlyPolicy
{
    private bool started, stopped;
    private uint? approvedAppId;
    internal uint RequireSynchronizationApp()
    {
        if (!started || stopped || approvedAppId is not uint id) throw Rejected();
        return id;
    }
    internal ReadAction Validate(string operation, JsonElement arguments, bool development)
    {
        if (stopped) throw Rejected();
        ReadAction action = operation switch
        {
            "start" when !started && arguments.ValueKind == JsonValueKind.Null => new(ReadOperation.Start),
            "stop" when arguments.ValueKind == JsonValueKind.Null => new(ReadOperation.Stop),
            "authenticate" when started && arguments.ValueKind == JsonValueKind.Null => new(ReadOperation.Authenticate),
            "authenticationState" when started && arguments.ValueKind == JsonValueKind.Null => new(ReadOperation.AuthenticationState),
            "navigate" when started => new(ReadOperation.Navigate, NavigationApp(arguments)),
            "readConfiguration" when started => new(ReadOperation.ReadConfiguration, ConfigurationApp(arguments)),
            "networkSelfTest" when !started && development && arguments.ValueKind is JsonValueKind.True or JsonValueKind.False =>
                new(ReadOperation.NetworkSelfTest, Hold: arguments.GetBoolean()),
            _ => throw Rejected(),
        };
        if (action.Operation == ReadOperation.ReadConfiguration && approvedAppId != action.AppId) throw Rejected();
        if (action.Operation == ReadOperation.Navigate)
        {
            if (approvedAppId is uint pinned && pinned != action.AppId) throw Rejected();
            approvedAppId = action.AppId;
        }
        if (action.Operation is ReadOperation.Start or ReadOperation.NetworkSelfTest) started = true;
        if (action.Operation == ReadOperation.Stop) stopped = true;
        return action;
    }
    private static uint NavigationApp(JsonElement argument)
    {
        const string prefix = "https://partner.steamgames.com/apps/achievements/";
        if (argument.ValueKind != JsonValueKind.String) throw Rejected();
        string route = argument.GetString()!;
        if (!route.StartsWith(prefix, StringComparison.Ordinal)) throw Rejected();
        string suffix = route[prefix.Length..];
        if (!uint.TryParse(suffix, NumberStyles.None, CultureInfo.InvariantCulture, out uint id) || id == 0
            || suffix != id.ToString(CultureInfo.InvariantCulture)) throw Rejected();
        return id;
    }
    private static uint ConfigurationApp(JsonElement argument)
    {
        if (argument.ValueKind != JsonValueKind.Object) throw Rejected();
        JsonProperty[] properties = argument.EnumerateObject().Take(2).ToArray();
        if (properties.Length != 1 || properties[0].Name != "value" || properties[0].Value.ValueKind != JsonValueKind.Number
            || !properties[0].Value.TryGetUInt32(out uint id) || id == 0)
            throw Rejected();
        return id;
    }
    private static InvalidDataException Rejected() => new("Read session command rejected.");
}
