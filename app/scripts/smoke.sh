#!/usr/bin/env bash
# Headless smoke test of the store app window: run it under Xvfb and stop it
# after ~5 s. Exits 0 whether or not the window was killed by the timer.
set -euo pipefail
cd "$(dirname "$0")/.."
# isolate the app's user-data dir (desired-state file) so a headless smoke
# run never writes into the real home
XDG_DATA_HOME="$(mktemp -d)"
export XDG_DATA_HOME
trap 'rm -rf "$XDG_DATA_HOME"' EXIT
xvfb-run -a bash -c './target/debug/nix-pilot-app & p=$!; sleep 5; kill $p 2>/dev/null; wait $p 2>/dev/null; true'