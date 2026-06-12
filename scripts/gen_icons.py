"""Generate Tauri app icons — brass crosshair on dark background"""
from PIL import Image, ImageDraw, ImageFont
import os

def make_icon(size, path):
    img = Image.new("RGBA", (size, size), (26, 28, 32, 255))
    draw = ImageDraw.Draw(img)
    cx, cy = size // 2, size // 2
    r = size * 0.3
    # crosshair circle
    draw.ellipse([cx - r, cy - r, cx + r, cy + r], outline=(201, 165, 95, 255), width=max(2, size // 16))
    # crosshair lines
    lw = max(2, size // 20)
    gap = r * 0.35
    draw.line([cx - r - 2, cy, cx - gap, cy], fill=(201, 165, 95, 255), width=lw)
    draw.line([cx + gap, cy, cx + r + 2, cy], fill=(201, 165, 95, 255), width=lw)
    draw.line([cx, cy - r - 2, cx, cy - gap], fill=(201, 165, 95, 255), width=lw)
    draw.line([cx, cy + gap, cx, cy + r + 2], fill=(201, 165, 95, 255), width=lw)
    # center dot
    dot_r = max(1, size // 40)
    draw.ellipse([cx - dot_r, cy - dot_r, cx + dot_r, cy + dot_r], fill=(201, 165, 95, 255))
    img.save(path)

base = r"C:\Users\Administrator\Documents\txt与gdb互转\src-tauri\icons"
os.makedirs(base, exist_ok=True)
make_icon(32, os.path.join(base, "32x32.png"))
make_icon(128, os.path.join(base, "128x128.png"))
make_icon(256, os.path.join(base, "128x128@2x.png"))
# .ico (use 32x32 as base)
img32 = Image.open(os.path.join(base, "32x32.png"))
img32.save(os.path.join(base, "icon.ico"), format="ICO", sizes=[(32, 32)])
# .icns — simply save a copy, macOS will use the PNG fallback
img256 = Image.open(os.path.join(base, "128x128@2x.png"))
img256.save(os.path.join(base, "icon.icns"), format="PNG")
print("Icons generated OK")
