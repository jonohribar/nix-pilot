#!/usr/bin/env bash
# Wireframe renderer: compile each Blueprint (.blp) to GTK .ui, then screenshot
# it under Xvfb with the shared style. Outputs <name>.png next to the sources.
#
# Notes:
#  - openbox runs inside Xvfb so GTK4's frame clock ticks (no compositor otherwise).
#  - GSK_RENDERER=cairo forces software rendering (GL has no GPU under Xvfb).
#
# Usage:  render.sh [surface*.blp ...]      (default: all *.blp in this dir)
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p build

BIN="$(cd .. && pwd)/target/debug/wireframe"
UI_DIR="$(pwd)/build"
CSS="$(pwd)/style.css"
OUT="$(pwd)"

targets=()
if [ "$#" -gt 0 ]; then
    for t in "$@"; do
        case "$t" in
            *.blp) targets+=("$t") ;;
            *)     targets+=("$t.blp") ;;
        esac
    done
else
    targets=(*.blp)
fi

for blp in "${targets[@]}"; do
    base="${blp##*/}"
    base="${base%.blp}"
    echo "== $base"
    blueprint-compiler compile "$blp" > "$UI_DIR/$base.ui"

    GSK_RENDERER=cairo GDK_BACKEND=x11 xvfb-run -a -s "-screen 0 1400x960x24" bash -c "
        openbox >/dev/null 2>&1 &
        sleep 1
        '$BIN' '$UI_DIR/$base.ui' '$CSS' &
        wp=\$!
        sleep 3
        xwd -root -silent | magick xwd:- -trim +repage '$OUT/$base.png'
        kill \$wp 2>/dev/null || true
    "
    echo "   wrote $OUT/$base.png ($(du -h "$OUT/$base.png" | cut -f1))"
done
