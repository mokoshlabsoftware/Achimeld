using System.Buffers.Binary;

namespace Achimeld.Browser;

public static class BoundedRaster
{
    // Check dimensions before asking Chromium to decode untrusted raster bytes.
    // Uploaded/converted verification accepts PNG and JPEG, as the original adapter did.
    public static bool HasSafeDimensions(ReadOnlySpan<byte> bytes, uint maximum)
    {
        if (bytes.Length >= 24 && bytes[..8].SequenceEqual(new byte[] { 137, 80, 78, 71, 13, 10, 26, 10 }) && bytes.Slice(12, 4).SequenceEqual("IHDR"u8))
        {
            uint width = BinaryPrimitives.ReadUInt32BigEndian(bytes.Slice(16, 4)), height = BinaryPrimitives.ReadUInt32BigEndian(bytes.Slice(20, 4));
            return width > 0 && height > 0 && width <= maximum && height <= maximum;
        }
        if (bytes.Length < 4 || bytes[0] != 0xff || bytes[1] != 0xd8) return false;
        int offset = 2;
        while (offset + 4 <= bytes.Length)
        {
            if (bytes[offset++] != 0xff) return false;
            while (offset < bytes.Length && bytes[offset] == 0xff) offset++;
            if (offset >= bytes.Length) return false;
            byte marker = bytes[offset++];
            if (marker is 0xd8 or 0x01 || marker is >= 0xd0 and <= 0xd7) continue;
            if (marker is 0xda or 0xd9 || offset + 2 > bytes.Length) return false;
            int length = BinaryPrimitives.ReadUInt16BigEndian(bytes.Slice(offset, 2));
            if (length < 2 || offset + length > bytes.Length) return false;
            if (marker is >= 0xc0 and <= 0xcf && marker is not (0xc4 or 0xc8 or 0xcc))
            {
                if (length < 8) return false;
                uint height = BinaryPrimitives.ReadUInt16BigEndian(bytes.Slice(offset + 3, 2)), width = BinaryPrimitives.ReadUInt16BigEndian(bytes.Slice(offset + 5, 2));
                return width > 0 && height > 0 && width <= maximum && height <= maximum;
            }
            offset += length;
        }
        return false;
    }
}
