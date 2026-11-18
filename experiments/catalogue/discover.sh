#!/usr/bin/env bash
# Discovery probe: for each candidate package, fetch its outputs from the
# binary cache (no local build) and scan them for .desktop entries and
# AppStream metainfo. Answers the unproven half: "is output-derived desktop
# data discoverable for real GUI candidates, and how often?"
set -u

for pkg in "$@"; do
  echo "=== $pkg ==="
  t0=$(date +%s.%3N)
  outs=$(nix build --no-link --print-out-paths "nixpkgs#$pkg" 2>/dev/null) \
    || { echo "  build/fetch FAILED"; continue; }
  t1=$(date +%s.%3N)
  desks=0
  metas=0
  dlist=""
  mlist=""
  for o in $outs; do
    for d in $(find "$o" -name '*.desktop' 2>/dev/null); do
      desks=$((desks + 1))
      dlist="$dlist $d"
    done
    for m in $(find "$o" -path '*/share/metainfo/*' \( -name '*.xml' -o -name '*.metainfo' \) 2>/dev/null); do
      metas=$((metas + 1))
      mlist="$mlist $m"
    done
  done
  secs=$(node -e "console.log(((($t1)-($t0))/1000).toFixed(1))")
  echo "  fetch+scan seconds: $secs"
  echo "  .desktop entries: $desks"
  for f in $dlist; do echo "    $f"; done
  echo "  metainfo files: $metas"
  for f in $mlist; do echo "    $f"; done
done