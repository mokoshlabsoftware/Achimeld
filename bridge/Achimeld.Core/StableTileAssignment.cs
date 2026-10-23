namespace Achimeld.Core;

public static class StableTileAssignment
{
    public static int GetNext(IEnumerable<int> existingAssignments)
    {
        ArgumentNullException.ThrowIfNull(existingAssignments);
        HashSet<int> used = [];
        foreach (int assignment in existingAssignments)
        {
            ArgumentOutOfRangeException.ThrowIfNegative(assignment);
            if (!used.Add(assignment))
            {
                throw new ArgumentException(
                    $"Tile assignment {assignment} is duplicated.",
                    nameof(existingAssignments));
            }
        }

        for (int candidate = 0; candidate < int.MaxValue; candidate++)
        {
            if (!used.Contains(candidate))
            {
                return candidate;
            }
        }

        if (!used.Contains(int.MaxValue))
        {
            return int.MaxValue;
        }

        throw new InvalidOperationException("No nonnegative tile assignment remains available.");
    }

}
