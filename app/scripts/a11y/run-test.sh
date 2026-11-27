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
APP_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
REPO_DIR="$(cd "$APP_DIR/.." && pwd)"
cd "$APP_DIR"
TEST="$1"
APP_BIN="$2"
# Let the Python driver own the one Xvfb process; choosing a display here
# avoids the runner starting a second, unused server.
export NIX_PILOT_A11Y_DISPLAY=":${3:-$(shuf -i 100-5000 -n 1)}"
# The nav/search tests drive the store shell, not first-run setup; and the
# harness must never write app files into a real configuration found via HOME.
export NIX_PILOT_SKIP_SETUP=1
# Point the app at a real catalogue index. CI may set NIX_PILOT_CATALOGUE;
# otherwise use the index in this repository checkout when available.
if [ -z "${NIX_PILOT_CATALOGUE:-}" ]; then
  cand="$REPO_DIR/experiments/catalogue/results/catalogue-index.json"
  if [ -f "$cand" ]; then
    export NIX_PILOT_CATALOGUE="$cand"
  else
    # CI and fresh checkouts have no generated index; fall back to the small
    # committed fixture so store/search tests remain functional.
    cand="$APP_DIR/testdata/catalogue-index.json"
    if [ -f "$cand" ]; then
      export NIX_PILOT_CATALOGUE="$cand"
    else
      echo "warning: catalogue index not found; catalogue-dependent tests may fail" >&2
    fi
  fi
fi
# Locate the session.conf shipped by the nix dbus package so the harness
# works on non-NixOS hosts (e.g. ubuntu-latest) that lack /etc/dbus-1/session.conf.
DBUS_BIN=$(command -v dbus-run-session)
DBUS_SESSION_CONF="${DBUS_BIN%/bin/dbus-run-session}/share/dbus-1/session.conf"
# The test driver starts launcher + app itself inside this single session.
dbus-run-session --config-file="$DBUS_SESSION_CONF" -- python3 "$TEST" "$APP_BIN"
