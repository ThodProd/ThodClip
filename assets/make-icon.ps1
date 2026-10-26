Add-Type -AssemblyName System.Drawing
$out = Join-Path $PSScriptRoot 'thodclip.ico'
$sizes = @(16,24,32,48,64,128,256)
$images = @()
foreach ($s in $sizes) {
    $bmp = New-Object System.Drawing.Bitmap($s, $s, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.Clear([System.Drawing.Color]::Transparent)
    $rect = New-Object System.Drawing.Rectangle(0, 0, $s, $s)
    $brush = New-Object System.Drawing.Drawing2D.LinearGradientBrush($rect, [System.Drawing.Color]::FromArgb(255, 79, 70, 229), [System.Drawing.Color]::FromArgb(255, 6, 182, 212), 45.0)
    $r = [Math]::Max(3, [int]($s * 0.22))
    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $d = $r * 2
    $path.AddArc(0, 0, $d, $d, 180, 90); $path.AddArc($s - $d - 1, 0, $d, $d, 270, 90)
    $path.AddArc($s - $d - 1, $s - $d - 1, $d, $d, 0, 90); $path.AddArc(0, $s - $d - 1, $d, $d, 90, 90)
    $path.CloseFigure()
    $g.FillPath($brush, $path)

    $body = New-Object System.Drawing.RectangleF(($s * 0.27), ($s * 0.24), ($s * 0.46), ($s * 0.55))
    $white = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 255, 255, 255))
    $g.FillRectangle($white, $body)

    $clip = New-Object System.Drawing.RectangleF(($s * 0.38), ($s * 0.17), ($s * 0.24), ($s * 0.12))
    $clipBrush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 224, 231, 255))
    $g.FillRectangle($clipBrush, $clip)

    $line = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb(255, 79, 70, 229), [Math]::Max(1.0, $s * 0.035))
    $line.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
    $line.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
    foreach ($y in @(0.43, 0.55, 0.67)) {
        $g.DrawLine($line, $s * 0.35, $s * $y, $s * 0.65, $s * $y)
    }
    $dot = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 6, 182, 212))
    $g.FillEllipse($dot, $s * 0.60, $s * 0.60, $s * 0.20, $s * 0.20)

    $g.Dispose(); $brush.Dispose(); $path.Dispose(); $white.Dispose(); $clipBrush.Dispose(); $line.Dispose(); $dot.Dispose()
    $ms = New-Object System.IO.MemoryStream
    $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    $images += , @($s, $ms.ToArray())
    $bmp.Dispose(); $ms.Dispose()
}

$stream = New-Object System.IO.MemoryStream
$writer = New-Object System.IO.BinaryWriter($stream)
$writer.Write([UInt16]0); $writer.Write([UInt16]1); $writer.Write([UInt16]$images.Count)
$offset = 6 + 16 * $images.Count
foreach ($img in $images) {
    $size = [int]$img[0]; $bytes = [byte[]]$img[1]
    $writer.Write([byte]($(if ($size -ge 256) { 0 } else { $size })))
    $writer.Write([byte]($(if ($size -ge 256) { 0 } else { $size })))
    $writer.Write([byte]0); $writer.Write([byte]0); $writer.Write([UInt16]1); $writer.Write([UInt16]32)
    $writer.Write([UInt32]$bytes.Length); $writer.Write([UInt32]$offset)
    $offset += $bytes.Length
}
foreach ($img in $images) { $writer.Write([byte[]]$img[1]) }
$writer.Flush()
[System.IO.File]::WriteAllBytes($out, $stream.ToArray())
$writer.Dispose(); $stream.Dispose()
Write-Output $out
