#!/usr/bin/env python3
"""Render branding/icon.png from theme colors.

Canonical source for the app icon. Colors mirror `src/bevy/theme.rs`;
edit here, then run `scripts/gen_app_icons.sh` to fan out platform sizes.

    python3 -m venv /tmp/kvint-icon-venv
    /tmp/kvint-icon-venv/bin/pip install Pillow
    /tmp/kvint-icon-venv/bin/python scripts/gen_branding_icon.py
"""
from pathlib import Path
from PIL import Image, ImageDraw

# Theme palette (src/bevy/theme.rs)
BOARD_LIGHT = (0xF0, 0xD9, 0xB5)
BOARD_DARK  = (0xB5, 0x88, 0x63)
STONE_P1    = (0x1B, 0x1A, 0x18)
STONE_P2    = (0x7E, 0x2A, 0x26)
FRAME       = (0x5A, 0x39, 0x22)

SIZE      = 1024
ROWS      = 7
FRAME_W   = 56
CORNER_R  = 200

# (row, col, color)
STONES = [
    (1, 1, STONE_P1),
    (2, 2, STONE_P1),
    (3, 1, STONE_P2),
    (3, 2, STONE_P2),
    (3, 3, STONE_P2),
    (3, 4, STONE_P2),
    (3, 5, STONE_P2),
    (4, 4, STONE_P1),
    (5, 5, STONE_P1),
]


def main() -> None:
    # Opaque black corners: App Store Connect rejects any alpha, and iOS masks the corners anyway.
    img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 255))

    ImageDraw.Draw(img).rounded_rectangle(
        (0, 0, SIZE - 1, SIZE - 1), radius=CORNER_R, fill=FRAME
    )

    # Checkerboard, masked to a slightly smaller rounded rect for the inset.
    inner = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    bd = ImageDraw.Draw(inner)
    cell = (SIZE - 2 * FRAME_W) / ROWS
    for r in range(ROWS):
        for c in range(ROWS):
            color = BOARD_LIGHT if (r + c) % 2 == 0 else BOARD_DARK
            x0 = FRAME_W + c * cell
            y0 = FRAME_W + r * cell
            bd.rectangle((x0, y0, x0 + cell + 1, y0 + cell + 1), fill=color)

    mask = Image.new("L", (SIZE, SIZE), 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        (FRAME_W, FRAME_W, SIZE - 1 - FRAME_W, SIZE - 1 - FRAME_W),
        radius=max(0, CORNER_R - FRAME_W),
        fill=255,
    )
    img.paste(inner, (0, 0), mask)

    # Matte stones: hard-edged flat disk, no shadow.
    body_r = cell * 0.40

    bd2 = ImageDraw.Draw(img)
    for (r, c, color) in STONES:
        cx = FRAME_W + (c + 0.5) * cell
        cy = FRAME_W + (r + 0.5) * cell
        bd2.ellipse(
            (cx - body_r, cy - body_r, cx + body_r, cy + body_r),
            fill=color,
        )

    # App Store Connect rejects PNGs with an alpha channel, even a fully opaque one.
    out = Path(__file__).resolve().parent.parent / "branding" / "icon.png"
    img.convert("RGB").save(out)
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
