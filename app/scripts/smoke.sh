#!/usr/bin/env bash
# Headless smoke test of the store app window: run it under Xvfb and confirm
# that its titled window appears before the startup deadline.
set -euo pipefail
cd "$(dirname "$0")/.."
# Isolate data and GTK preferences so the test cannot write to or inherit
# settings from the real home (for example, a host dark-theme preference).
TEST_HOME="$(mktemp -d)"
export HOME="$TEST_HOME"
export XDG_DATA_HOME="$TEST_HOME/data"
export XDG_CONFIG_HOME="$TEST_HOME/config"
export GDK_BACKEND=x11
export NIX_PILOT_SKIP_SETUP=1
# A fresh session bus gives this process its own primary GtkApplication
# instance. Without it the new instance either hands off to an already-running
# nix-pilot and exits 0 before ever running `activate` (no window here), or has
# no bus to register on and never activates. A headless run cannot reach the
# host accessibility bus, so GTK logs a non-fatal "connection refused"; the
# window still appears and the check only asserts on the window.
mkdir -p "$XDG_DATA_HOME" "$XDG_CONFIG_HOME"
trap 'rm -rf "$TEST_HOME"' EXIT

# Bounded end to end: a stall is a failure (timeout rc 124), never a hang.
timeout 60 dbus-run-session -- xvfb-run -a bash -c '
	set -euo pipefail
	# Keep the app's own output for diagnosis: the smoke currently fails only
	# on the ubuntu CI runner, never locally, so a failure must reveal why.
	# A file (not the shell's stdout) keeps the app from holding the pipe open
	# and blocking xvfb-run/dbus-run-session teardown on slow shutdown.
	app_log="$TEST_HOME/app.log"
	./target/debug/nix-pilot-app >"$app_log" 2>&1 &
	app_pid=$!
	# The windowed app does not exit promptly on SIGTERM under this env, so
	# never `wait` on it here: kill it and return immediately, or the EXIT trap
	# (and this whole test) would block forever.
	cleanup() {
		kill "$app_pid" 2>/dev/null || true
	}
	trap cleanup EXIT

	for _ in {1..50}; do
		if ! kill -0 "$app_pid" 2>/dev/null; then
			wait "$app_pid" || true
			echo "smoke: app exited before presenting its window" >&2
			echo "--- app log ---" >&2
			cat "$app_log" >&2 || true
			exit 1
		fi
		if xwininfo -root -tree 2>/dev/null | grep -q "\"nix-pilot\""; then
			echo "smoke: app window appeared"
			exit 0
		fi
		sleep 0.1
	done

	echo "smoke: app window did not appear within 5 seconds" >&2
	echo "--- app log ---" >&2
	cat "$app_log" >&2 || true
	exit 1
'