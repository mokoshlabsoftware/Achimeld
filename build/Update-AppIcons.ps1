[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$root = Split-Path -Parent $PSScriptRoot
$assets = Join-Path $root 'crates/achimeld-app/assets'
# achimeld.png is the unchanged supplied artwork. Only format/size conversion
# belongs here: do not recolor, remove its background, or redraw the mark.
$source = [Drawing.Image]::FromFile((Join-Path $assets 'achimeld.png'))
$frames = [Collections.Generic.List[byte[]]]::new()
$sizes = @(16, 24, 32, 48, 64, 128, 256)
try {
    if ($source.Width -ne 256 -or $source.Height -ne 256) { throw 'Expected the supplied 256-by-256 application icon.' }
    foreach ($size in $sizes) {
        $bitmap = [Drawing.Bitmap]::new($size, $size, [Drawing.Imaging.PixelFormat]::Format32bppArgb)
        $graphics = [Drawing.Graphics]::FromImage($bitmap)
        $attributes = [Drawing.Imaging.ImageAttributes]::new()
        $stream = [IO.MemoryStream]::new()
        try {
            $graphics.CompositingMode = [Drawing.Drawing2D.CompositingMode]::SourceCopy
            $graphics.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
            $graphics.PixelOffsetMode = [Drawing.Drawing2D.PixelOffsetMode]::HighQuality
            $attributes.SetWrapMode([Drawing.Drawing2D.WrapMode]::TileFlipXY)
            $graphics.DrawImage($source, [Drawing.Rectangle]::new(0, 0, $size, $size), 0, 0,
                $source.Width, $source.Height, [Drawing.GraphicsUnit]::Pixel, $attributes)
            if ($size -eq 256) {
                $frames.Add([IO.File]::ReadAllBytes((Join-Path $assets 'achimeld.png')))
            } else {
                $bitmap.Save($stream, [Drawing.Imaging.ImageFormat]::Png)
                $frames.Add($stream.ToArray())
            }
            if ($size -eq 128) {
                $rgba = [byte[]]::new($size * $size * 4)
                for ($y = 0; $y -lt $size; $y++) {
                    for ($x = 0; $x -lt $size; $x++) {
                        $pixel = $bitmap.GetPixel($x, $y)
                        $offset = ($y * $size + $x) * 4
                        $rgba[$offset] = $pixel.R; $rgba[$offset + 1] = $pixel.G
                        $rgba[$offset + 2] = $pixel.B; $rgba[$offset + 3] = $pixel.A
                    }
                }
                [IO.File]::WriteAllBytes((Join-Path $root 'crates/achimeld-ui/assets/achimeld-logo.rgba'), $rgba)
            }
        } finally {
            $stream.Dispose(); $attributes.Dispose(); $graphics.Dispose(); $bitmap.Dispose()
        }
    }
    $output = [IO.MemoryStream]::new()
    $writer = [IO.BinaryWriter]::new($output)
    try {
        $writer.Write([uint16]0); $writer.Write([uint16]1); $writer.Write([uint16]$sizes.Count)
        $offset = 6 + 16 * $sizes.Count
        for ($i = 0; $i -lt $sizes.Count; $i++) {
            $dimension = [byte]($sizes[$i] % 256)
            $writer.Write($dimension); $writer.Write($dimension)
            $writer.Write([byte]0); $writer.Write([byte]0)
            $writer.Write([uint16]1); $writer.Write([uint16]32)
            $writer.Write([uint32]$frames[$i].Length); $writer.Write([uint32]$offset)
            $offset += $frames[$i].Length
        }
        foreach ($frame in $frames) { $writer.Write($frame) }
        $writer.Flush()
        [IO.File]::WriteAllBytes((Join-Path $assets 'achimeld.ico'), $output.ToArray())
    } finally { $writer.Dispose(); $output.Dispose() }
} finally { $source.Dispose() }
Write-Output 'Converted the application artwork to a seven-size Windows ICO and the title/About RGBA asset.'
