"""Draws the app icon: seeds placed at the golden angle (Vogel's phyllotaxis model).

    python3 scripts/icon.py && pnpm tauri icon src-tauri/icons/app-icon.svg
"""

import math
from pathlib import Path

SIZE = 1024
INSET = 100  # macOS icons sit inside a 824px rounded square
RADIUS = 186
SEEDS = 210
GOLDEN = math.pi * (3 - math.sqrt(5))


def main() -> None:
    c = SIZE / 2
    spread = 23.0
    dots = []
    for i in range(1, SEEDS + 1):
        r = spread * math.sqrt(i)
        a = i * GOLDEN
        size = 3.2 + 9.5 * (i / SEEDS) ** 0.8
        opacity = 0.55 + 0.45 * (i / SEEDS)
        dots.append(
            f'<circle cx="{c + r * math.cos(a):.1f}" cy="{c + r * math.sin(a):.1f}" r="{size:.1f}" fill-opacity="{opacity:.2f}"/>'
        )
    side = SIZE - 2 * INSET
    svg = f"""<svg xmlns="http://www.w3.org/2000/svg" width="{SIZE}" height="{SIZE}" viewBox="0 0 {SIZE} {SIZE}">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#3a6e55"/>
      <stop offset="1" stop-color="#244a38"/>
    </linearGradient>
    <clipPath id="tile"><rect x="{INSET}" y="{INSET}" width="{side}" height="{side}" rx="{RADIUS}"/></clipPath>
  </defs>
  <rect x="{INSET}" y="{INSET}" width="{side}" height="{side}" rx="{RADIUS}" fill="url(#bg)"/>
  <g clip-path="url(#tile)" fill="#f3e7cc">
    {chr(10).join("    " + d for d in dots)}
  </g>
</svg>
"""
    out = Path(__file__).resolve().parent.parent / "src-tauri" / "icons" / "app-icon.svg"
    out.write_text(svg)
    print(out)


if __name__ == "__main__":
    main()
