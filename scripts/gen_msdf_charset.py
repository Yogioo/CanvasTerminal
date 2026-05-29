"""Generate the expanded MSDF charset.txt for CanvasTerminal.

Combines:
- ASCII printable 0x20-0x7E (95 code points)
- All unique non-ASCII characters from src/**/*.rs (623 CJK + symbols)
- Additional common CJK punctuation and symbols

Output format compatible with msdf-atlas-gen:
  [0x20, 0x7E] 0x4E2D 0x6587 ...

Usage:
  python scripts/gen_msdf_charset.py    # (works from any cwd)
  # Then regenerate atlas:
  msdf-atlas-gen.exe -font "C:/Windows/Fonts/NotoSansSC-VF.ttf" -type mtsdf -format png ^
    -json assets/fonts/msdf/atlas.json -imageout assets/fonts/msdf/atlas.png ^
    -size 48 -pxrange 4 -charset assets/fonts/msdf/charset.txt -fontname "NotoSansSC-VF"
"""
import os
import unicodedata
from pathlib import Path

# Derive project root from this script's location: scripts/../ = project root
PROJECT = Path(__file__).resolve().parent.parent

# ── 1. ASCII printable ──
ascii_range = [0x20, 0x7E]  # 95 code points

# ── 2. Collect non-ASCII from source ──
src_chars: set[str] = set()
for root, dirs, files in os.walk(PROJECT / "src"):
    for f in files:
        if not f.endswith(".rs"):
            continue
        path = os.path.join(root, f)
        with open(path, "r", encoding="utf-8") as fh:
            for line in fh:
                for c in line:
                    if ord(c) > 0x7F:
                        src_chars.add(c)

print(f"Source non-ASCII chars: {len(src_chars)}")

# ── 3. Additional punctuation/symbols not in source ──
extra_symbols = set(
    "？；『』【】《》、…—·￥←✔○■□▲▼★☆"
)
# Filter to only include punctuation/symbol categories that Noto Sans SC might have
for c in list(extra_symbols):
    cat = unicodedata.category(c)
    if cat.startswith("P") or cat.startswith("S"):
        pass  # Keep punctuation and symbols
    else:
        extra_symbols.discard(c)
        print(f"Dropping extra char {c!r} U+{ord(c):04X} (cat={cat})")

# ── 4. GB2312 Level 1 Hanzi (常用汉字) ──
# GB2312 Qu (区) 16-55, Wei (位) 1-94 for Level 1 (~3755 characters)
# Encoding: bytes([0xA0+qu, 0xA0+wei]).decode('gb2312')
gb2312_l1: set[str] = set()
for qu in range(16, 56):
    for wei in range(1, 95):
        try:
            c = bytes([0xA0 + qu, 0xA0 + wei]).decode("gb2312")
            gb2312_l1.add(c)
        except UnicodeDecodeError:
            pass
print(f"GB2312 Level 1 (16-55): {len(gb2312_l1)} chars")

# ── 5. Merge everything ──
all_chars: set[str] = set()
all_chars.update(src_chars)
all_chars.update(extra_symbols)
all_chars.update(gb2312_l1)

# Exclude non-BMP (emoji) - not in Noto Sans SC
all_chars = {c for c in all_chars if ord(c) <= 0xFFFF}
non_bmp = {c for c in (src_chars | extra_symbols) if ord(c) > 0xFFFF}
if non_bmp:
    print(f"Excluded non-BMP (emoji): {''.join(sorted(non_bmp, key=ord))}")

# Exclude ASCII (already covered by range)
all_chars = {c for c in all_chars if ord(c) > 0x7E}

print(f"Non-ASCII chars for charset: {len(all_chars)}")

# Sort by Unicode code point
sorted_chars = sorted(all_chars, key=ord)

# ── 6. Write charset.txt (hex format for msdf-atlas-gen) ──
hex_codes = [f"0x{ord(c):04X}" for c in sorted_chars]
output_path = PROJECT / "assets" / "fonts" / "msdf" / "charset.txt"

# Use a readable line length (~80 chars per line)
line = f"[0x{ascii_range[0]:02X}, 0x{ascii_range[1]:02X}]"
parts = []
for h in hex_codes:
    candidate = f"{line} {h}"
    if len(candidate) > 100:  # wrap
        parts.append(line)
        line = h
    else:
        line = candidate
parts.append(line)

with open(output_path, "w", encoding="utf-8") as f:
    f.write("\n".join(parts))
    f.write("\n")

total_code_points = (ascii_range[1] - ascii_range[0] + 1) + len(sorted_chars)
print(f"Charset written to {output_path}")
print(f"  ASCII range: {ascii_range[0]:02X}-{ascii_range[1]:02X} ({ascii_range[1] - ascii_range[0] + 1} code points)")
print(f"  Non-ASCII: {len(sorted_chars)} chars")
print(f"  Total code points: {total_code_points}")

# ── 7. Verify font coverage ──
try:
    from fontTools.ttLib import TTFont
    font_path = "C:/Windows/Fonts/NotoSansSC-VF.ttf"
    font = TTFont(font_path)
    cmap = font.getBestCmap()
    missing = [c for c in sorted_chars if ord(c) not in cmap]
    print(f"  Not in Noto Sans SC: {len(missing)} chars")
    for c in missing[:10]:
        print(f"    U+{ord(c):04X} {c}")
    print()
    if missing:
        print("These chars will be skipped by msdf-atlas-gen (tofu fallback).")
except ImportError:
    print("  (fonttools not available, skipping coverage check)")
