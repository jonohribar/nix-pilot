#!/usr/bin/env bash
# Phase 0 scoped measurement runner.
#
# Evaluates the metadata exporter over the first N top-level Nixpkgs attrs and
# reports cost/completeness. Scoped by design: the full 27k-set run belongs on
# real infra (see README). Node is used for JSON analysis (no python here).
set -euo pipefail

N="${1:-300}"
ROOT="$(cd "$(dirname "$0")" && pwd)"
OUT="$ROOT/results/scoped-${N}.json"
mkdir -p "$ROOT/results"

PKG='(builtins.getFlake "nixpkgs").legacyPackages.x86_64-linux'
NAMES="(builtins.getFlake \"nixpkgs\").lib.take ${N} (builtins.attrNames ${PKG})"
EXPR="(import $ROOT/export.nix) { pkgs = ${PKG}; names = ${NAMES}; }"

start=$(date +%s.%3N)
nix eval --impure --json --expr "$EXPR" > "$ROOT/results/records.json" 2> "$ROOT/results/eval.err" \
  || { echo "EVAL_FAILED"; cat "$ROOT/results/eval.err"; exit 1; }
end=$(date +%s.%3N)

node "$ROOT/analyze.js" "$start" "$end" | tee "$OUT"
echo
echo "raw records: $ROOT/results/records.json"
echo "eval stderr: $ROOT/results/eval.err"