using System.Globalization;

namespace Achimeld.Core;

/// <summary>Up to 64 editable color points; the initial twelve lie on the edges.</summary>
public sealed record IconGradientMap
{
    public IReadOnlyList<IconColorPoint> Points { get; init; } = Enumerable.Range(0, 12).Select(i =>
    {
        (double x, double y) = AnchorPosition(i);
        int shade = (int)Math.Round(y * 255, MidpointRounding.AwayFromZero);
        return new IconColorPoint { X = x, Y = y, ColorHex = $"#FF{shade:X2}{shade:X2}{shade:X2}" };
    }).ToArray();

    public static (double X, double Y) AnchorPosition(int i) => i switch
    {
        >= 0 and <= 3 => (i / 3d, 0),
        >= 4 and <= 7 => ((i - 4) / 3d, 1),
        >= 8 and <= 9 => (0, (i - 7) / 3d),
        >= 10 and <= 11 => (1, (i - 9) / 3d),
        _ => (.5, .5)
    };

    public bool Valid()
    {
        if (Points is not { Count: <= 64 }) return false;
        for (int i = 0; i < Points.Count; i++)
        {
            IconColorPoint p = Points[i];
            if (p is null || !p.Valid()) return false;
            for (int j = 0; j < i; j++)
                if (Math.Pow(p.X - Points[j].X, 2) + Math.Pow(p.Y - Points[j].Y, 2) < 1e-8) return false;
        }
        return true;
    }

    public string Fingerprint() => Points.Count == 0 ? "empty" : string.Join(";", Points.Select(p => p.Fingerprint()));
}

public sealed record IconColorPoint
{
    public double X { get; init; } = .5;
    public double Y { get; init; } = .5;
    public string ColorHex { get; init; } = "#FFFFFFFF";
    public double Power { get; init; } = 1;
    public double RangeX { get; init; } = .35;
    public double RangeY { get; init; } = .35;

    public bool Valid() => In(X, 0, 1) && In(Y, 0, 1) && In(Power, .05, 8) && In(RangeX, .02, 2) &&
        In(RangeY, .02, 2) && ColorHex is { Length: 7 or 9 } && ColorHex[0] == '#' &&
        ColorHex.AsSpan(1).IndexOfAnyExcept("0123456789abcdefABCDEF") < 0;

    private static bool In(double v, double min, double max) => double.IsFinite(v) && v >= min && v <= max;
    public string Fingerprint() => ColorHex + "," + string.Join(",", new[] { X, Y, Power, RangeX, RangeY }
        .Select(v => v.ToString("G17", CultureInfo.InvariantCulture)));
}
