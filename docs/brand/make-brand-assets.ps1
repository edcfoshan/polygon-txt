# 品牌资产再生成脚本（UTF-8 BOM；PowerShell 5.1 直接 -File 运行）
# 输入：src-tauri/icons/GIS土地测绘软件图标设计.jpeg（豆包原始稿，带水印、全出血方角）
# 产出：docs/brand/app-icon-source.png（去水印+RGBA 圆角）、src/assets/brand-mark.png（单色黑透明）、
#       docs/brand/social-preview-1280x640.png（渐变青底+左图形+右产品名自动字号）
# 图标全套另需：npx tauri icon docs/brand/app-icon-source.png，然后删除 bundle.icon 未引用的多余产物
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$srcPath = Join-Path $repo 'src-tauri\icons\GIS土地测绘软件图标设计.jpeg'
$outSource = Join-Path $repo 'docs\brand\app-icon-source.png'
$outMark = Join-Path $repo 'src\assets\brand-mark.png'
$outSocial = Join-Path $repo 'docs\brand\social-preview-1280x640.png'

$srcBmp = New-Object System.Drawing.Bitmap($srcPath)
$W = $srcBmp.Width; $H = $srcBmp.Height
Write-Output "source: ${W}x${H}"

$clean = New-Object System.Drawing.Bitmap($W, $H, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
$g = [System.Drawing.Graphics]::FromImage($clean)
$g.DrawImage($srcBmp, 0, 0, $W, $H)
$g.Dispose()
$srcBmp.Dispose()

# 1) 去右下角水印：矩形区内逐列在上下两条纯底行之间垂直插值
$x0 = [int](0.78 * $W); $y0 = [int](0.92 * $H)
for ($x = $x0; $x -lt $W; $x++) {
  $top = $clean.GetPixel($x, $y0 - 4)
  $bot = $clean.GetPixel($x, $H - 1)
  $span = ($H - 1) - $y0
  for ($y = $y0; $y -lt $H; $y++) {
    $t = ($y - $y0) / $span
    $c = [System.Drawing.Color]::FromArgb(255,
      [int]($top.R + ($bot.R - $top.R) * $t),
      [int]($top.G + ($bot.G - $top.G) * $t),
      [int]($top.B + ($bot.B - $top.B) * $t))
    $clean.SetPixel($x, $y, $c)
  }
}
$c1 = $clean.GetPixel(2, 2)
$c2 = $clean.GetPixel($W - 3, $H - 3)
Write-Output "field gradient: $($c1.R),$($c1.G),$($c1.B) -> $($c2.R),$($c2.G),$($c2.B)"

# 2) 圆角矩形几何沿用旧源图比例：透明边距 21/1254、圆角 213/1254
$inset = [int]((21.0 / 1254.0) * $W)
$r = [int]((213.0 / 1254.0) * $W)
$mask = New-Object System.Drawing.Bitmap($W, $H, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
$gm = [System.Drawing.Graphics]::FromImage($mask)
$gm.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$path = New-Object System.Drawing.Drawing2D.GraphicsPath
$d = [float](2 * $r)
$x1 = $W - $inset - $d
$y1 = $H - $inset - $d
$path.AddArc($inset, $inset, $d, $d, 180, 90)
$path.AddArc($x1, $inset, $d, $d, 270, 90)
$path.AddArc($x1, $y1, $d, $d, 0, 90)
$path.AddArc($inset, $y1, $d, $d, 90, 90)
$path.CloseFigure()
$gm.FillPath([System.Drawing.Brushes]::White, $path)
$gm.Dispose()

$rect = New-Object System.Drawing.Rectangle(0, 0, $W, $H)
$lc = $clean.LockBits($rect, [System.Drawing.Imaging.ImageLockMode]::ReadOnly, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
$lm = $mask.LockBits($rect, [System.Drawing.Imaging.ImageLockMode]::ReadOnly, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
$bc = New-Object byte[] ($W * $H * 4); $bm = New-Object byte[] ($W * $H * 4)
[System.Runtime.InteropServices.Marshal]::Copy($lc.Scan0, $bc, 0, $bc.Length)
[System.Runtime.InteropServices.Marshal]::Copy($lm.Scan0, $bm, 0, $bm.Length)
$clean.UnlockBits($lc); $mask.UnlockBits($lm); $mask.Dispose()
for ($i = 0; $i -lt $bc.Length; $i += 4) { $bc[$i + 3] = $bm[$i + 3] }
$final = New-Object System.Drawing.Bitmap($W, $H, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
$lf = $final.LockBits($rect, [System.Drawing.Imaging.ImageLockMode]::WriteOnly, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
[System.Runtime.InteropServices.Marshal]::Copy($bc, 0, $lf.Scan0, $bc.Length)
$final.UnlockBits($lf)
$final.Save($outSource, [System.Drawing.Imaging.ImageFormat]::Png)
Write-Output "saved $outSource"

# 3) 亮度 smoothstep 取 alpha：派生单色黑版（brand-mark）与米白版（social 用）
$markB = New-Object System.Drawing.Bitmap($W, $H, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
$whiteB = New-Object System.Drawing.Bitmap($W, $H, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
$lmk = $markB.LockBits($rect, [System.Drawing.Imaging.ImageLockMode]::WriteOnly, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
$lwh = $whiteB.LockBits($rect, [System.Drawing.Imaging.ImageLockMode]::WriteOnly, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
$bmk = New-Object byte[] ($W * $H * 4); $bwh = New-Object byte[] ($W * $H * 4)
$minX = $W; $minY = $H; $maxX = -1; $maxY = -1
for ($y = 0; $y -lt $H; $y++) {
  $row = $y * $W * 4
  for ($x = 0; $x -lt $W; $x++) {
    $i = $row + $x * 4
    $L = (0.2126 * $bc[$i + 2] + 0.7152 * $bc[$i + 1] + 0.0722 * $bc[$i]) / 255.0
    $t = ($L - 0.55) / (0.78 - 0.55)
    if ($t -lt 0) { $t = 0 } elseif ($t -gt 1) { $t = 1 }
    $a = [byte][int](255 * $t * $t * (3 - 2 * $t))
    if ($a -gt 16) {
      if ($x -lt $minX) { $minX = $x }; if ($x -gt $maxX) { $maxX = $x }
      if ($y -lt $minY) { $minY = $y }; if ($y -gt $maxY) { $maxY = $y }
    }
    $bmk[$i] = 0; $bmk[$i + 1] = 0; $bmk[$i + 2] = 0; $bmk[$i + 3] = $a
    $bwh[$i] = 248; $bwh[$i + 1] = 247; $bwh[$i + 2] = 242; $bwh[$i + 3] = $a
  }
}
[System.Runtime.InteropServices.Marshal]::Copy($bmk, 0, $lmk.Scan0, $bmk.Length)
[System.Runtime.InteropServices.Marshal]::Copy($bwh, 0, $lwh.Scan0, $bwh.Length)
$markB.UnlockBits($lmk); $whiteB.UnlockBits($lwh)
$bw = $maxX - $minX + 1; $bh = $maxY - $minY + 1
Write-Output "glyph bbox: ${minX},${minY} ${bw}x${bh}"
$cropRect = New-Object System.Drawing.Rectangle($minX, $minY, $bw, $bh)
$markCrop = $markB.Clone($cropRect, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
$whiteCrop = $whiteB.Clone($cropRect, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
$markB.Dispose(); $whiteB.Dispose(); $clean.Dispose(); $final.Dispose()

# 4) brand-mark：bbox 加 3% padding 成正方画布，缩到 512
$side = [Math]::Max($bw, $bh)
$pad = [int](0.03 * $side)
$canvasSide = $side + 2 * $pad
$mark512 = New-Object System.Drawing.Bitmap(512, 512, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
$gmk = [System.Drawing.Graphics]::FromImage($mark512)
$gmk.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
$dest = [int](512 * $side / $canvasSide)
$off = [int]((512 - $dest) / 2)
$gmk.DrawImage($markCrop, $off, $off, $dest, $dest)
$gmk.Dispose()
$mark512.Save($outMark, [System.Drawing.Imaging.ImageFormat]::Png)
Write-Output "saved $outMark"

# 5) social preview：对角渐变底 + 左图形 + 右产品名（字号自动收缩到放得下）
$social = New-Object System.Drawing.Bitmap(1280, 640, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
$gs = [System.Drawing.Graphics]::FromImage($social)
$gs.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$gs.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAliasGridFit
$brush = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
  (New-Object System.Drawing.Point(0, 0)), (New-Object System.Drawing.Point(1280, 640)), $c1, $c2)
$gs.FillRectangle($brush, 0, 0, 1280, 640)
$scale = [Math]::Min(440 / $bw, 440 / $bh)
$dw = [int]($bw * $scale); $dh = [int]($bh * $scale)
$gx = 100; $gy = [int]((640 - $dh) / 2)
$gs.DrawImage($whiteCrop, $gx, $gy, $dw, $dh)
$fam = New-Object System.Drawing.FontFamily('Microsoft YaHei')
$title = '极思G界址点互转工具'
$sub = 'SHP / GDB ↔ 界址点 TXT 双向转换'
$tx = $gx + $dw + 76
$budget = 1280 - $tx - 70
$titleSize = 64
$titleFont = $null
$tw = 0
while ($titleSize -ge 30) {
  $titleFont = New-Object System.Drawing.Font($fam, [float]$titleSize, [System.Drawing.FontStyle]::Bold)
  $tw = $gs.MeasureString($title, $titleFont).Width
  if ($tw -le $budget) { break }
  $titleFont.Dispose()
  $titleSize -= 2
}
$subFont = New-Object System.Drawing.Font($fam, [float][int]($titleSize * 0.48), [System.Drawing.FontStyle]::Regular)
$th = $gs.MeasureString($title, $titleFont).Height
$sh = $gs.MeasureString($sub, $subFont).Height
$y0 = [int]((640 - $th - 18 - $sh) / 2)
$gs.DrawString($title, $titleFont, [System.Drawing.Brushes]::White, $tx, $y0)
$subBrush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(225, 255, 255, 255))
$gs.DrawString($sub, $subFont, $subBrush, $tx + 2, $y0 + $th + 18)
$gs.Dispose()
$social.Save($outSocial, [System.Drawing.Imaging.ImageFormat]::Png)
Write-Output ("title size " + $titleSize + " width " + [int]$tw + " budget " + $budget)
Write-Output "saved $outSocial"
Write-Output 'DONE'
