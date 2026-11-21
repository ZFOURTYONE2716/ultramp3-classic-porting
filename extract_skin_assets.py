"""Decode the original UltraMP3 PCX skin assets to lossless PNG textures."""
from io import BytesIO
from pathlib import Path
from zipfile import ZipFile
from PIL import Image

ROOT = Path(__file__).resolve().parent
SOURCES = {
    "default": "12_0_S60.dta",
    "tiger": "08_0_Tiger.mp3skin",
    "technic": "09_0_Technic.mp3skin",
    "ferrari": "10_0_Ferrari.mp3skin",
}

for name, source in SOURCES.items():
    out = ROOT / "assets" / name
    out.mkdir(parents=True, exist_ok=True)
    with ZipFile(ROOT / "extracted" / source) as archive:
        def load(member, mode):
            with Image.open(BytesIO(archive.read(member))) as image:
                return image.convert(mode)

        load("skin.pcx", "RGB").save(out / "skin.png")
        sprites = load("spr.pcx", "RGBA")
        sprites.putalpha(load("spr_a.pcx", "L"))
        sprites.save(out / "sprites.png")
        (out / "skin.ini").write_bytes(archive.read("skin.ini"))
        sprite_definition = archive.read("spr.txt")
        definition = ROOT / "assets" / "spr.txt"
        if name == "default":
            definition.write_bytes(sprite_definition)
            font_mask = load("font.pcx", "L")
            font = Image.new("RGBA", font_mask.size, "white")
            font.putalpha(font_mask)
            font.save(ROOT / "assets" / "font.png")
        elif definition.read_bytes() != sprite_definition:
            raise ValueError(f"{source} has a different sprite definition")

