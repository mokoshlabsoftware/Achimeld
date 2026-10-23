namespace Achimeld.Core;

public readonly record struct AtlasGridMetrics(
    int Columns,
    int Rows,
    int TileWidth,
    int TileHeight,
    int RemainderWidth,
    int RemainderHeight,
    int Capacity,
    bool IsSquare);

public static class AtlasGridGeometry
{
    public static AtlasGridMetrics? TryMeasure(int pixelWidth, int pixelHeight, int columns, int rows)
    {
        if (pixelWidth <= 0 || pixelHeight <= 0 || columns <= 0 || rows <= 0)
        {
            return null;
        }

        int tileWidth = pixelWidth / columns;
        int tileHeight = pixelHeight / rows;
        if (tileWidth <= 0 || tileHeight <= 0)
        {
            return null;
        }

        int capacity;
        try
        {
            capacity = checked(columns * rows);
        }
        catch (OverflowException)
        {
            return null;
        }

        return new AtlasGridMetrics(
            columns,
            rows,
            tileWidth,
            tileHeight,
            pixelWidth % columns,
            pixelHeight % rows,
            capacity,
            tileWidth == tileHeight);
    }

    public static bool TileFits(int tileIndex, int columns, int rows)
    {
        if (tileIndex < 0 || columns <= 0 || rows <= 0)
        {
            return false;
        }

        try
        {
            return tileIndex < checked(columns * rows);
        }
        catch (OverflowException)
        {
            return false;
        }
    }
}
