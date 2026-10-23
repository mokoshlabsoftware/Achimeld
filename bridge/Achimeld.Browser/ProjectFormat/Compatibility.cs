using Achimeld.Core;

namespace Achimeld.Persistence;

// Pure schema mapping only. Storage remains in the supervised Rust host.
internal static class RebrandCompatibility
{
    internal static string NormalizeProjectFormat(string value) =>
        value == "com.achiever.project" ? AchimeldProject.FormatIdentifier : value;
}

internal sealed class ProjectDataException(string message) : IOException(message);
