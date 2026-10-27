"""Small tile textures for the station shell. Original patterns, not a third-party kit."""

import os

from PIL import Image, ImageDraw, ImageFilter

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
OUT = os.path.join(ROOT, "apps", "web", "public", "textures")


def steel():
    image = Image.new("RGB", (512, 512), (42, 46, 51))
    draw = ImageDraw.Draw(image)
    seam = (59, 64, 70)
    rivet = (126, 132, 140)
    for step in range(0, 513, 128):
        draw.line((step, 0, step, 512), fill=seam, width=4)
        draw.line((0, step, 512, step), fill=seam, width=4)
        for other in range(0, 513, 128):
            draw.ellipse((step - 5, other - 5, step + 5, other + 5), fill=rivet)
            draw.ellipse((step - 2, other - 2, step + 2, other + 2), fill=(70, 74, 80))
    rough = Image.new("L", (512, 512), 150)
    rdraw = ImageDraw.Draw(rough)
    for step in range(0, 513, 128):
        rdraw.line((step, 0, step, 512), fill=210, width=6)
        rdraw.line((0, step, 512, step), fill=210, width=6)
    image.save(os.path.join(OUT, "steel-albedo.png"))
    rough.save(os.path.join(OUT, "steel-roughness.png"))


def grate():
    image = Image.new("RGB", (256, 256), (22, 26, 30))
    draw = ImageDraw.Draw(image)
    bar = (78, 84, 90)
    for step in range(0, 257, 18):
        draw.line((step, 0, step, 256), fill=bar, width=3)
        draw.line((0, step, 256, step), fill=bar, width=3)
    image.save(os.path.join(OUT, "grate-albedo.png"))


def grass():
    image = Image.new("RGB", (512, 512), (78, 128, 62))
    pixels = image.load()
    for y in range(512):
        for x in range(512):
            n = ((x * 17 + y * 31) ^ (x * y)) & 255
            pixels[x, y] = (58 + n // 8, 112 + (n % 40), 48 + (n % 24))
    image.save(os.path.join(OUT, "grass-albedo.png"))


def cloud():
    image = Image.new("RGBA", (768, 384), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    puffs = [
        (180, 190, 150, 70),
        (280, 170, 120, 56),
        (360, 200, 170, 74),
        (470, 175, 130, 60),
        (540, 205, 110, 48),
        (240, 210, 90, 40),
    ]
    for x, y, rx, ry in puffs:
        draw.ellipse((x - rx, y - ry, x + rx, y + ry), fill=(255, 246, 238, 150))
    image = image.filter(ImageFilter.GaussianBlur(radius=8))
    image.save(os.path.join(OUT, "cloud.png"))


if __name__ == "__main__":
    os.makedirs(OUT, exist_ok=True)
    steel()
    grate()
    grass()
    cloud()
    print(OUT)
