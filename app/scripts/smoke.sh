#!/usr/bin/env bash
# Headless smoke test of the store app window: run it under Xvfb and confirm
# that its titled window appears before the startup deadline.
#
# DISPLAY is chosen and exported here and this script starts its own Xvfb,
# rather than delegating to `xvfb-run`: xvfb-run did not export DISPLAY to
# the app on the CI runner, which made GTK's init fail before any window.
# Managing the X server directly makes the smoke deterministic wherever it
# runs (the AT-SPI integration driver already does the same).
set -euo pipefail
cd "$(dirname "$0")/.."
# Isolate data and GTK preferences so the test cannot write to or inherit
# settings from the real home (for example, a host dark-theme preference).
TEST_HOME="$(mktemp -d)"
export TEST_HOME
export HOME="$TEST_HOME"
export XDG_DATA_HOME="$TEST_HOME/data"
export XDG_CONFIG_HOME="$TEST_HOME/config"
export GDK_BACKEND=x11
export NIX_PILOT_SKIP_SETUP=1
export DISPLAY=":${NIX_PILOT_SMOKE_DISPLAY:-$(( 100 + RANDOM % 2000 ))}"
mkdir -p "$XDG_DATA_HOME" "$XDG_CONFIG_HOME"

# Start our own X server on the chosen display. It must live OUTSIDE the
# dbus-run-session below: a server inside the session never exits on its own,
# so dbus-run-session would wait for it forever during teardown.
Xvfb "$DISPLAY" -screen 0 1280x800x24 -ac +extension GLX +extension RANDR +extension RENDER \
	>"$TEST_HOME/xvfb.log" 2>&1 &
XVFB_PID=$!
cleanup() {
	kill "$XVFB_PID" 2>/dev/null || true
	rm -rf "$TEST_HOME"
}
trap cleanup EXIT
# Wait for the X socket so the app never races Xvfb startup.
xnum="${DISPLAY#:}"
for _ in {1..50}; do
	[ -S "/tmp/.X11-unix/X$xnum" ] && break
	sleep 0.1
done

# The app loop lives in a generated script so this outer shell never needs to
# nest-shell-quote the polling logic. It runs under a fresh session bus (its
# own primary GtkApplication instance) with Xvfb still on DISPLAY.
cat >"$TEST_HOME/inner.sh" <<'INNER'
#!/usr/bin/env bash
set -euo pipefail
pass() {
	# Record the verdict outside the session so a lingering teardown can never
	# turn a pass into a timeout; the outer shell reads this file.
	echo "0" >"$TEST_HOME/result"
}
fail() {
	echo "1" >"$TEST_HOME/result"
}
# Keep the app's own output in a file, off this shell's stdout, so its slow
# shutdown cannot hold the pipe and block teardown; the file preserves it for
# diagnosis on failure.
app_log="$TEST_HOME/app.log"
app_pid=
# The windowed app ignores SIGTERM and keeps the session bus alive, which
# would block dbus-run-session's teardown; kill -9 it and never `wait` here.
cleanup() {
	[ -n "$app_pid" ] && kill -9 "$app_pid" 2>/dev/null || true
}
trap cleanup EXIT
./target/debug/nix-pilot-app >"$app_log" 2>&1 &
app_pid=$!

for _ in {1..50}; do
	if ! kill -0 "$app_pid" 2>/dev/null; then
		wait "$app_pid" 2>/dev/null || true
		echo "smoke: app exited before presenting its window" >&2
		echo "--- app log ---" >&2
		cat "$app_log" >&2 || true
		fail
		exit 0
	fi
	if xwininfo -root -tree 2>/dev/null | grep -q '"nix-pilot"'; then
		echo "smoke: app window appeared"
		pass
		exit 0
	fi
	sleep 0.1
done

echo "smoke: app window did not appear within 5 seconds" >&2
echo "--- app log ---" >&2
cat "$app_log" >&2 || true
fail
exit 0
INNER

# Bounded end to end: a stall is a failure, never a hang. The result file
# records the verdict, so even if this session's teardown lingers and the
# timeout must SIGKILL it, success is not lost.
timeout --kill-after=5 60 dbus-run-session -- bash "$TEST_HOME/inner.sh" || true
if [ -f "$TEST_HOME/result" ]; then
	exit "$(cat "$TEST_HOME/result")"
fi
echo "smoke: runner timed out without a result" >&2
exit 1