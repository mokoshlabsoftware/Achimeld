[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$root = Join-Path (Split-Path -Parent $PSScriptRoot) 'crates/achimeld-ui/assets/toolbar'
$count = 0
foreach ($file in Get-ChildItem -LiteralPath $root -Filter '*.png' -File -Recurse) {
    $source = [Drawing.Image]::FromFile($file.FullName)
    $target = $null
    $graphics = $null
    $attributes = $null
    $temporary = $file.FullName + '.resize.tmp'
    try {
        if ($source.Width -le 128 -and $source.Height -le 128) { continue }
        $ratio = [Math]::Min(128.0 / $source.Width, 128.0 / $source.Height)
        $width = [Math]::Max(1, [int][Math]::Floor($source.Width * $ratio))
        $height = [Math]::Max(1, [int][Math]::Floor($source.Height * $ratio))
        $target = [Drawing.Bitmap]::new($width, $height, [Drawing.Imaging.PixelFormat]::Format32bppPArgb)
        $graphics = [Drawing.Graphics]::FromImage($target)
        $graphics.CompositingMode = [Drawing.Drawing2D.CompositingMode]::SourceCopy
        $graphics.CompositingQuality = [Drawing.Drawing2D.CompositingQuality]::HighQuality
        $graphics.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
        $graphics.PixelOffsetMode = [Drawing.Drawing2D.PixelOffsetMode]::HighQuality
        $attributes = [Drawing.Imaging.ImageAttributes]::new()
        $attributes.SetWrapMode([Drawing.Drawing2D.WrapMode]::TileFlipXY)
        $graphics.DrawImage($source, [Drawing.Rectangle]::new(0, 0, $width, $height), 0, 0, $source.Width, $source.Height, [Drawing.GraphicsUnit]::Pixel, $attributes)
        $target.Save($temporary, [Drawing.Imaging.ImageFormat]::Png)
        $source.Dispose()
        Move-Item -LiteralPath $temporary -Destination $file.FullName -Force
        Write-Host "Toolbar icon resized: $($file.Name) -> ${width}x${height}"
        $count++
    } finally {
        if ($attributes) { $attributes.Dispose() }
        if ($graphics) { $graphics.Dispose() }
        if ($target) { $target.Dispose() }
        $source.Dispose()
        if (Test-Path -LiteralPath $temporary) { Remove-Item -LiteralPath $temporary }
    }
}
Write-Host "Toolbar icon watchdog: $count oversized PNG(s) resized."
