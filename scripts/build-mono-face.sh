#!/usr/bin/env bash
# Build Almagest Mono — the cockpit's bespoke Iosevka build — from the
# checked-in plan (crates/magistr-cockpit/fonts/almagest-mono.toml) and
# install the three faces the renderers embed.
#
# The plan carries the design; this script carries the toolchain:
#   1. clone Iosevka at the pinned tag (shallow),
#   2. npm ci + build the unhinted TTFs (neither of our rasterizers
#      hints, and unhinted needs no ttfautohint),
#   3. pin the vertical metrics to the cell contract with fonttools —
#      hhea/OS2 ascent 1020, descent -300, lineGap 0 (Iosevka's
#      `leading` fixes the total at 1320; the split is pinned here so
#      fontdue and qs-text read the exact numbers the goldens were
#      measured on),
#   4. install Regular/Bold/Italic into crates/magistr-cockpit/fonts/.
#
# Needs: node >= 18, npm, python3 (a venv with fonttools is created).
# Takes minutes — this is a font compile, run it when the plan changes.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PLAN="$ROOT/crates/magistr-cockpit/fonts/almagest-mono.toml"
DEST="$ROOT/crates/magistr-cockpit/fonts"
IOSEVKA_TAG="${IOSEVKA_TAG:-v34.8.1}"
WORK="${IOSEVKA_WORK:-$(mktemp -d)}"

if [ ! -d "$WORK/iosevka" ]; then
  git clone --depth 1 --branch "$IOSEVKA_TAG" \
    https://github.com/be5invis/Iosevka.git "$WORK/iosevka"
fi
cd "$WORK/iosevka"
[ -d node_modules ] || npm ci
cp "$PLAN" private-build-plans.toml
npm run build -- ttf-unhinted::AlmagestMono

if [ ! -x "$WORK/fontenv/bin/python" ]; then
  python3 -m venv "$WORK/fontenv"
  "$WORK/fontenv/bin/pip" -q install fonttools
fi

for cut in Regular Bold Italic; do
  src="$WORK/iosevka/dist/AlmagestMono/TTF-Unhinted/AlmagestMono-$cut.ttf"
  "$WORK/fontenv/bin/python" - "$src" "$DEST/AlmagestMono-$cut.ttf" <<'EOF'
import sys
from fontTools.ttLib import TTFont
src, dest = sys.argv[1], sys.argv[2]
f = TTFont(src)
assert f["head"].unitsPerEm == 1000, "the cell contract assumes upem 1000"
hh, os2 = f["hhea"], f["OS/2"]
hh.ascent, hh.descent, hh.lineGap = 1020, -300, 0
os2.sTypoAscender, os2.sTypoDescender, os2.sTypoLineGap = 1020, -300, 0
os2.usWinAscent, os2.usWinDescent = 1020, 300
adv = f["hmtx"]["M"][0]
assert adv == 600, f"advance {adv} != 600/1000 em — the plan's width moved"
f.save(dest)
print(f"pinned {dest}: advance 600, hhea 1020/-300/0")
EOF
done

echo "Almagest Mono installed into $DEST"
