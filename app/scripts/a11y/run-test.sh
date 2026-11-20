#!/usr/bin/env bash
# Run one AT-SPI integration test against the real app, headless.
#
# Critical: the Python Atspi client, at-spi-bus-launcher, and the app must
# ALL share ONE fresh session bus. If the test driver and the app run on
# different buses, the app registers somewhere the probe never reads and the
# test fails with "did not appear in AT-SPI tree". So everything lives inside
# a single `dbus-run-session`; the driver launches launcher+app as children
# of that same session.
#
# Usage (from app/):
#   nix-shell test-atspi.nix --run './scripts/a11y/run-test.sh scripts/a11y/test_nav_settings.py target/debug/nix-pilot-app'
set -euo pipefail
TEST="$1"
APP_BIN="$2"
DISPLAY_NUM=":${3:-$(shuf -i 100-5000 -n 1)}"

Xvfb "$DISPLAY_NUM" -screen 0 1280x800x24 -ac -nolisten tcp >/dev/null 2>&1 &
XV=$!
trap 'kill $XV 2>/dev/null || true' EXIT
sleep 1

export DISPLAY="$DISPLAY_NUM"
# Point the app at a real catalogue index. CI picks NIX_PILOT_CATALOGUE; a
# dev worktree may not contain experiments/ (git-sparse), so fall back to the
# main checkout path when it exists, otherwise the store list stays empty and
# catalogue-dependent tests fail.
if [ -z "${NIX_PILOT_CATALOGUE:-}" ]; then
  for cand in \
    "$PWD/experiments/catalogue/results/catalogue-index.json" \
    "$OLDPWD/experiments/catalogue/results/catalogue-index.json" \
    "/home/jonoh/Projects/nix-pilot/experiments/catalogue/results/catalogue-index.json"; do
    if [ -f "$cand" ]; then
      export NIX_PILOT_CATALOGUE="$cand"
      break
    fi
  done
fi
# The test driver starts launcher + app itself inside this single session.
exec dbus-run-session -- python3 "$TEST" "$APP_BIN"
