"""Build the Orbit transport atlas from the registered UltraMP3 sprites."""
from pathlib import Path
from PIL import Image

ASSETS = Path(__file__).resolve().parent / "assets"
with Image.open(ASSETS / "default-hd" / "sprites.png") as source:
    atlas = source.convert("RGBA").resize((2048, 1024), Image.Resampling.LANCZOS)

pixels = atlas.load()
for y in range(atlas.height):
    for x in range(atlas.width):
        r, g, b, a = pixels[x, y]
        if not a:
            continue
        # Original atlas positions are in a 256x128 logical grid.
        pressed = 61 * 8 <= x < 159 * 8 and 15 * 8 <= y < 28 * 8
        clock = y >= 63 * 8
        peak = (255, 210, 130) if pressed or clock else (145, 235, 255)
        base = (3, 12, 24)
        intensity = max(r, g, b) / 255
        color = tuple(round(low + (high - low) * intensity) for low, high in zip(base, peak))
        pixels[x, y] = (*color, a)

atlas.save(ASSETS / "orbit" / "sprites.png")

