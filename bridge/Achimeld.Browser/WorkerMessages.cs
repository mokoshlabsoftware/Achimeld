using System.Text.Json;

namespace Achimeld.Browser;

public sealed record SteamWorkerRequest(Guid RequestId, string Operation, JsonElement Arguments);
public sealed record SteamWorkerResponse(Guid RequestId, bool Success, JsonElement Data, string? ErrorCode = null);
