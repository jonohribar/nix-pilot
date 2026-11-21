#!/usr/bin/env bash
# Headless smoke test of the store app window: run it under Xvfb and confirm
# that its titled window appears before the startup deadline.
set -euo pipefail
cd "$(dirname "$0")/.."
# Isolate data and GTK preferences so the test cannot write to or inherit
# settings from the real home (for example, a host dark-theme preference).
TEST_HOME="$(mktemp -d)"
export XDG_DATA_HOME="$TEST_HOME/data"
export XDG_CONFIG_HOME="$TEST_HOME/config"
export GDK_BACKEND=x11
# A headless run cannot reach the host accessibility bus, so GTK logs a
# non-fatal "connection refused" from its accessibility thread; the window
# still appears and the check only asserts on the window.
mkdir -p "$XDG_DATA_HOME" "$XDG_CONFIG_HOME"
trap 'rm -rf "$TEST_HOME"' EXIT

xvfb-run -a bash -c '
	set -euo pipefail
	./target/debug/nix-pilot-app &
	app_pid=$!
	cleanup() {
		kill "$app_pid" 2>/dev/null || true
		wait "$app_pid" 2>/dev/null || true
	}
	trap cleanup EXIT

	for _ in {1..50}; do
		if ! kill -0 "$app_pid" 2>/dev/null; then
			wait "$app_pid" || true
			echo "smoke: app exited before presenting its window" >&2
			exit 1
		fi
		if xwininfo -root -tree 2>/dev/null | grep -q "\"nix-pilot\""; then
			echo "smoke: app window appeared"
			exit 0
		fi
		sleep 0.1
	done

	echo "smoke: app window did not appear within 5 seconds" >&2
	exit 1
'