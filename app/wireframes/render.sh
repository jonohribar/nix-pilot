#!/usr/bin/env bash
# Wireframe renderer: compile every page-*.blp to GTK .ui, then run the
# navigable prototype under Xvfb for each state and screenshot it.
# Outputs proto-<state>.png next to the sources.
#
# Notes:
#  - openbox runs inside Xvfb so GTK4's frame clock ticks (no compositor).
#  - GSK_RENDERER=cairo forces software rendering (GL has no GPU under Xvfb).
#
# Usage: render.sh            (all states)   |   render.sh detail search
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p build

BIN="$(cd .. && pwd)/target/debug/proto"
CSS="$(pwd)/style.css"
OUT="$(pwd)"

# compile every page blueprint
for blp in page-*.blp; do
    base="${blp%.blp}"
    blueprint-compiler compile "$blp" > "build/$base.ui"
done

declare -A STATES=(
    [home]=home
    [search]="search --search docker"
    [detail]="detail --detail GIMP"
    [settings]=settings
    [updates]=updates
)

targets=()
if [ "$#" -gt 0 ]; then targets=("$@"); else targets=("${!STATES[@]}"); fi

for st in "${targets[@]}"; do
    name="${STATES[$st]%% *}"
    extra=""
    [[ "${STATES[$st]}" != "$name" ]] && extra="${STATES[$st]#* }"
    echo "== $st"
    GSK_RENDERER=cairo GDK_BACKEND=x11 xvfb-run -a -s "-screen 0 1400x960x24" bash -c "
        openbox >/dev/null 2>&1 &
        sleep 1
        WIREFRAME_BUILD='$OUT/build' WIREFRAME_CSS='$CSS' '$BIN' --page $name $extra &
        wp=\$!
        sleep 3
        xwd -root -silent | magick xwd:- -trim +repage '$OUT/proto-$st.png'
        kill \$wp 2>/dev/null || true
    "
    echo "   wrote $OUT/proto-$st.png ($(du -h "$OUT/proto-$st.png" | cut -f1))"
done
