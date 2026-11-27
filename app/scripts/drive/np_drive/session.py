#!/usr/bin/env python3
"""
Session runner for np-drive.

Launches the app under an isolated session: dbus-run-session + dedicated Xvfb
display + at-spi-bus-launcher + the app, with proper environment variables.
Designed to degrade gracefully (clear errors, not hangs) if the live session
fails — every wait is bounded, processes are killed on close.
"""

import os
import sys
import time
import subprocess
import tempfile
import shutil
import signal
from pathlib import Path
from typing import Optional, Dict, List
from contextlib import contextmanager


DEFAULT_TIMEOUT = 30  # seconds for any single wait
MAX_SESSION_TIME = 60  # total session lifetime cap


class SessionError(Exception):
    """Raised when session setup or operation fails."""
    pass


class Session:
    """
    Manages an isolated test session for the nix-pilot app.

    Components:
    - Xvfb virtual display
    - dbus-run-session (session bus)
    - at-spi-bus-launcher (accessibility bus)
    - The nix-pilot app process

    All processes are tracked and killed on cleanup.
    """

    def __init__(
        self,
        app_binary: str,
        catalogue_path: Optional[str] = None,
        config_dir: Optional[str] = None,
        xvfb_display: Optional[str] = None,
        dump_file: Optional[str] = None,
        extra_env: Optional[Dict[str, str]] = None,
    ):
        self.app_binary = app_binary
        self.catalogue_path = catalogue_path
        self.config_dir = config_dir
        self.xvfb_display = xvfb_display or f":{100 + os.getpid() % 5000}"
        self.dump_file = dump_file
        self.extra_env = extra_env or {}

        # Process tracking
        self.processes: List[subprocess.Popen] = []
        self.xvfb_proc: Optional[subprocess.Popen] = None
        self.dbus_proc: Optional[subprocess.Popen] = None  # the dbus-run-session wrapper
        self.launcher_proc: Optional[subprocess.Popen] = None
        self.app_proc: Optional[subprocess.Popen] = None

        # Temp directories
        self.test_home = tempfile.mkdtemp(prefix="np-drive-")
        self.runtime_dir = os.path.join(self.test_home, "runtime")
        self.data_dir = os.path.join(self.test_home, "data")
        self.config_dir_path = os.path.join(self.test_home, "config")

        # State
        self._started = False
        self._start_time = 0.0

    def _run_cmd(self, cmd: List[str], env: Optional[Dict[str, str]] = None) -> subprocess.Popen:
        """Run a command and track the process."""
        proc_env = os.environ.copy()
        if env:
            proc_env.update(env)
        proc = subprocess.Popen(
            cmd,
            env=proc_env,
            start_new_session=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        self.processes.append(proc)
        return proc

    def _wait_for_file(self, path: str, timeout: float = DEFAULT_TIMEOUT) -> bool:
        """Wait for a file to exist and be non-empty."""
        start = time.time()
        while time.time() - start < timeout:
            try:
                if os.path.exists(path) and os.path.getsize(path) > 0:
                    return True
            except OSError:
                pass
            time.sleep(0.1)
        return False

    def _wait_for_process(self, proc: subprocess.Popen, timeout: float) -> bool:
        """Wait for a process to exit (return True if exited, False if timeout)."""
        try:
            proc.wait(timeout=timeout)
            return True
        except subprocess.TimeoutExpired:
            return False

    def start(self) -> None:
        """Start the full isolated session."""
        if self._started:
            return

        self._start_time = time.time()

        # Create directories
        os.makedirs(self.runtime_dir, mode=0o700, exist_ok=True)
        os.makedirs(self.data_dir, exist_ok=True)
        os.makedirs(self.config_dir_path, exist_ok=True)

        # Build environment for all child processes
        base_env = os.environ.copy()
        base_env.update({
            "DISPLAY": self.xvfb_display,
            "GDK_BACKEND": "x11",
            "GSK_RENDERER": "cairo",
            "GTK_MODULES": "",
            "ATSPI_DBUS_IMPLEMENTATION": "dbus-daemon",
            "XDG_DATA_HOME": self.data_dir,
            "XDG_CONFIG_HOME": self.config_dir_path,
            "XDG_RUNTIME_DIR": self.runtime_dir,
            "NIX_PILOT_UI_DUMP": "1",
        })

        if self.dump_file:
            base_env["NIX_PILOT_UI_DUMP_FILE"] = self.dump_file

        if self.catalogue_path:
            base_env["NIX_PILOT_CATALOGUE"] = self.catalogue_path

        if self.config_dir:
            base_env["NIX_PILOT_CONFIG_DIR"] = self.config_dir

        # NIX_PILOT_SKIP_SETUP=1 avoids first-run setup; tests drive the store shell
        base_env["NIX_PILOT_SKIP_SETUP"] = "1"

        base_env.update(self.extra_env)

        # Start Xvfb
        self.xvfb_proc = self._run_cmd([
            "Xvfb", self.xvfb_display,
            "-screen", "0", "1280x800x24",
            "-ac", "+extension", "GLX", "+extension", "RANDR", "+extension", "RENDER"
        ], env=base_env)

        # Give Xvfb time to start
        time.sleep(1)

        # Start everything inside a single dbus-run-session
        # The dbus-run-session will run a shell that starts launcher + app
        session_script = f"""#!/bin/bash
set -e
export DISPLAY={self.xvfb_display}
export GDK_BACKEND=x11
export GSK_RENDERER=cairo
export GTK_MODULES=
export ATSPI_DBUS_IMPLEMENTATION=dbus-daemon
export XDG_DATA_HOME={self.data_dir}
export XDG_CONFIG_HOME={self.config_dir_path}
export XDG_RUNTIME_DIR={self.runtime_dir}
export NIX_PILOT_UI_DUMP=1
{f'export NIX_PILOT_UI_DUMP_FILE={self.dump_file}' if self.dump_file else ''}
{f'export NIX_PILOT_CATALOGUE={self.catalogue_path}' if self.catalogue_path else ''}
{f'export NIX_PILOT_CONFIG_DIR={self.config_dir}' if self.config_dir else ''}
export NIX_PILOT_SKIP_SETUP=1

# Start at-spi-bus-launcher
at-spi-bus-launcher --launch-immediately &
LAUNCHER_PID=$!
sleep 2

# Start the app
{self.app_binary} &
APP_PID=$!

# Wait for either process to exit
wait $APP_PID
EXIT_CODE=$?
kill $LAUNCHER_PID 2>/dev/null
exit $EXIT_CODE
"""

        script_path = os.path.join(self.test_home, "session.sh")
        with open(script_path, "w") as f:
            f.write(session_script)
        os.chmod(script_path, 0o755)

        # Launch dbus-run-session with the script
        self.dbus_proc = self._run_cmd([
            "dbus-run-session", "--", "bash", script_path
        ], env=base_env)

        # The dbus_proc is the session wrapper; the app and launcher are its children
        # We can't easily get their PIDs from here, but we track the wrapper

        self._started = True

    def wait_for_snapshot(self, timeout: float = DEFAULT_TIMEOUT) -> bool:
        """Wait for the snapshot file to appear and be non-empty."""
        if not self.dump_file:
            return False
        return self._wait_for_file(self.dump_file, timeout)

    def wait_for_app_ready(self, timeout: float = DEFAULT_TIMEOUT) -> bool:
        """Wait for the app to be responsive (best effort via snapshot)."""
        return self.wait_for_snapshot(timeout)

    def is_alive(self) -> bool:
        """Check if the session wrapper is still running."""
        if not self.dbus_proc:
            return False
        return self.dbus_proc.poll() is None

    def elapsed(self) -> float:
        """Return elapsed time since session start."""
        return time.time() - self._start_time

    def close(self) -> None:
        """Clean up all processes and temp directories."""
        # Kill in reverse order: dbus session (which kills children), then Xvfb
        for proc in [self.dbus_proc, self.xvfb_proc]:
            if proc and proc.poll() is None:
                try:
                    # Send SIGTERM to the process group
                    os.killpg(os.getpgid(proc.pid), signal.SIGTERM)
                    proc.wait(timeout=2)
                except (ProcessLookupError, subprocess.TimeoutExpired):
                    try:
                        os.killpg(os.getpgid(proc.pid), signal.SIGKILL)
                        proc.wait(timeout=1)
                    except (ProcessLookupError, subprocess.TimeoutExpired):
                        pass

        # Clean up temp directory
        shutil.rmtree(self.test_home, ignore_errors=True)

    def __enter__(self) -> "Session":
        self.start()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        self.close()


@contextmanager
def isolated_session(
    app_binary: str,
    catalogue_path: Optional[str] = None,
    config_dir: Optional[str] = None,
    xvfb_display: Optional[str] = None,
    dump_file: Optional[str] = None,
    extra_env: Optional[Dict[str, str]] = None,
    timeout: float = DEFAULT_TIMEOUT,
) -> "Session":
    """
    Context manager for an isolated test session.

    Usage:
        with isolated_session(app_binary, catalogue_path="/path/to/catalogue-index.json") as session:
            session.wait_for_snapshot()
            # read snapshot, drive app, etc.
    """
    session = Session(
        app_binary=app_binary,
        catalogue_path=catalogue_path,
        config_dir=config_dir,
        xvfb_display=xvfb_display,
        dump_file=dump_file,
        extra_env=extra_env,
    )
    try:
        session.start()
        # Wait briefly for session to stabilize
        time.sleep(2)
        yield session
    finally:
        session.close()


def find_catalogue_index(repo_root: Optional[str] = None) -> Optional[str]:
    """Find the catalogue index file (fixture or generated)."""
    if repo_root is None:
        repo_root = os.environ.get("NIX_PILOT_REPO_ROOT", "/tmp/np-drive-wt")

    # Try committed fixture first
    fixture = os.path.join(repo_root, "app", "testdata", "catalogue-index.json")
    if os.path.exists(fixture):
        return fixture

    # Try generated index
    generated = os.path.join(repo_root, "experiments", "catalogue", "results", "catalogue-index.json")
    if os.path.exists(generated):
        return generated

    return None


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description="Test session runner")
    parser.add_argument("app_binary", help="Path to nix-pilot-app binary")
    parser.add_argument("--catalogue", help="Path to catalogue-index.json")
    parser.add_argument("--config-dir", help="Path to config directory")
    parser.add_argument("--dump-file", help="Path for UI dump file")
    parser.add_argument("--timeout", type=float, default=30, help="Wait timeout")
    args = parser.parse_args()

    catalogue = args.catalogue or find_catalogue_index()
    dump_file = args.dump_file or "/tmp/nix-pilot-ui.json"

    print(f"Starting session with app: {args.app_binary}")
    print(f"Catalogue: {catalogue}")
    print(f"Dump file: {dump_file}")

    with isolated_session(
        app_binary=args.app_binary,
        catalogue_path=catalogue,
        config_dir=args.config_dir,
        dump_file=dump_file,
        timeout=args.timeout,
    ) as session:
        print("Session started, waiting for snapshot...")
        if session.wait_for_snapshot(args.timeout):
            print(f"Snapshot written to {dump_file}")
            with open(dump_file, "r") as f:
                print(f.read()[:500])
        else:
            print("ERROR: Snapshot did not appear within timeout")
            sys.exit(1)
    print("Session closed successfully")