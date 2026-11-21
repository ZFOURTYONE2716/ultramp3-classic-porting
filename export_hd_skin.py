"""Register the ImageGen remaster and export the three production textures."""
from pathlib import Path
from PIL import Image

ASSETS = Path(__file__).resolve().parent / "assets"
SOURCE = ASSETS / "hd-source"
OUT = ASSETS / "default-hd"
SIDE = 3840
SPRITE_SCALE = SIDE // 256

# Crop only the generated controls that retained the original symbol.
# Previous and Next still need the vertical bars from the original masks.
GEN_BUTTONS = {11: (507, 605), 12: (611, 700), 13: (714, 807),
               14: (808, 910), 16: (1000, 1095)}


def export_skin():
    with Image.open(SOURCE / "skin-imagegen.png") as image:
        size = (round(SIDE * 176 / 208), SIDE)
        image.convert("RGB").resize(size, Image.Resampling.LANCZOS).save(OUT / "skin.png")


def export_sprites():
    with Image.open(ASSETS / "default" / "sprites.png") as original:
        atlas = original.resize((SIDE, SIDE // 2), Image.Resampling.BICUBIC)
        original_alpha = original.getchannel("A")
    with Image.open(SOURCE / "transport-imagegen.png") as generated:
        for pressed, (top, bottom) in enumerate(((0, 110), (110, 210))):
            for sprite, (left, right) in GEN_BUTTONS.items():
                cell = generated.crop((left, top, right, bottom))
                bounds = cell.getchannel("A").point(lambda alpha: 255 if alpha >= 32 else 0).getbbox()
                if bounds is None:
                    raise ValueError(f"missing generated button {sprite}")
                cell = cell.crop(bounds)
                x, y = 61 + 14 * (sprite - 10), 1 + pressed * 14
                destination = original_alpha.crop((x, y, x + 13, y + 13)).getbbox()
                if destination is None:
                    raise ValueError(f"missing original button {sprite}")
                x0, y0, x1, y1 = destination
                size = ((x1 - x0) * SPRITE_SCALE, (y1 - y0) * SPRITE_SCALE)
                tile = tuple(v * SPRITE_SCALE for v in (x, y, x + 13, y + 13))
                atlas.paste((0, 0, 0, 0), tile)
                atlas.paste(cell.resize(size, Image.Resampling.LANCZOS),
                            ((x + x0) * SPRITE_SCALE, (y + y0) * SPRITE_SCALE))
    atlas.save(OUT / "sprites.png")


def export_font():
    with Image.open(ASSETS / "font.png") as original:
        scale = SIDE // original.width
        atlas = Image.new("RGBA", (SIDE, original.height * scale))
        for row in range(10):
            for column in range(16):
                x, y = column * 5, row * 6
                cell = original.crop((x, y, x + 5, y + 6))
                # Resize each glyph separately so a neighboring cell cannot bleed.
                cell = cell.resize((5 * scale, 6 * scale), Image.Resampling.BICUBIC)
                atlas.paste(cell, (x * scale, y * scale))
        atlas.save(OUT / "font.png")


def main():
    OUT.mkdir(exist_ok=True)
    export_skin()
    export_sprites()
    export_font()


if __name__ == "__main__":
    main()

