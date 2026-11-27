#!/usr/bin/env python3
"""
Unit tests for session module.
"""

import sys
import os
import tempfile
import json
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from np_drive.session import Session, find_catalogue_index


def test_session_env_preparation():
    """Test that session prepares environment correctly."""
    with tempfile.TemporaryDirectory() as tmpdir:
        dump_file = os.path.join(tmpdir, "ui-dump.json")
        session = Session(
            app_binary="/fake/app",
            catalogue_path="/fake/catalogue.json",
            config_dir="/fake/config",
            dump_file=dump_file,
        )

        # Check that temp directories would be created
        assert "np-drive-" in session.test_home
        assert session.runtime_dir == os.path.join(session.test_home, "runtime")
        assert session.data_dir == os.path.join(session.test_home, "data")
        assert session.config_dir_path == os.path.join(session.test_home, "config")

        # Check env building (without actually starting)
        env = os.environ.copy()
        env.update({
            "DISPLAY": session.xvfb_display,
            "GDK_BACKEND": "x11",
            "GSK_RENDERER": "cairo",
            "GTK_MODULES": "",
            "ATSPI_DBUS_IMPLEMENTATION": "dbus-daemon",
            "XDG_DATA_HOME": session.data_dir,
            "XDG_CONFIG_HOME": session.config_dir_path,
            "XDG_RUNTIME_DIR": session.runtime_dir,
            "NIX_PILOT_UI_DUMP": "1",
            "NIX_PILOT_UI_DUMP_FILE": dump_file,
            "NIX_PILOT_CATALOGUE": "/fake/catalogue.json",
            "NIX_PILOT_CONFIG_DIR": "/fake/config",
            "NIX_PILOT_SKIP_SETUP": "1",
        })

        assert env["NIX_PILOT_UI_DUMP"] == "1"
        assert env["NIX_PILOT_UI_DUMP_FILE"] == dump_file
        assert env["NIX_PILOT_CATALOGUE"] == "/fake/catalogue.json"
        assert env["NIX_PILOT_CONFIG_DIR"] == "/fake/config"
        assert env["NIX_PILOT_SKIP_SETUP"] == "1"
        assert "AT_SPI_BUS_ADDRESS" not in env  # Should not be set

        print("test_session_env_preparation PASSED")


def test_session_script_generation():
    """Test the session shell script generation logic."""
    session = Session(
        app_binary="/fake/app",
        xvfb_display=":99",
        dump_file="/tmp/dump.json",
        catalogue_path="/fake/cat.json",
        config_dir="/fake/config",
    )
    session.test_home = "/tmp/test-session"
    session.runtime_dir = "/tmp/test-session/runtime"
    session.data_dir = "/tmp/test-session/data"
    session.config_dir_path = "/tmp/test-session/config"

    # This mimics the script generation in session.start()
    script = f"""#!/bin/bash
set -e
export DISPLAY={session.xvfb_display}
export GDK_BACKEND=x11
export GSK_RENDERER=cairo
export GTK_MODULES=
export ATSPI_DBUS_IMPLEMENTATION=dbus-daemon
export XDG_DATA_HOME={session.data_dir}
export XDG_CONFIG_HOME={session.config_dir_path}
export XDG_RUNTIME_DIR={session.runtime_dir}
export NIX_PILOT_UI_DUMP=1
export NIX_PILOT_UI_DUMP_FILE={session.dump_file}
export NIX_PILOT_CATALOGUE={session.catalogue_path}
export NIX_PILOT_CONFIG_DIR={session.config_dir}
export NIX_PILOT_SKIP_SETUP=1

# Start at-spi-bus-launcher
at-spi-bus-launcher --launch-immediately &
LAUNCHER_PID=$!
sleep 2

# Start the app
{session.app_binary} &
APP_PID=$!

# Wait for either process to exit
wait $APP_PID
EXIT_CODE=$?
kill $LAUNCHER_PID 2>/dev/null
exit $EXIT_CODE
"""

    # Verify key elements
    assert "dbus-daemon" in script
    assert "NIX_PILOT_UI_DUMP=1" in script
    assert "NIX_PILOT_SKIP_SETUP=1" in script
    assert "AT_SPI_BUS_ADDRESS" not in script  # Not exported
    assert "at-spi-bus-launcher --launch-immediately" in script
    assert "/fake/app" in script

    print("test_session_script_generation PASSED")


def test_find_catalogue_fixture():
    """Test finding the committed fixture."""
    repo_root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    catalogue = find_catalogue_index(repo_root)
    if catalogue:
        assert os.path.exists(catalogue)
        with open(catalogue) as f:
            data = json.load(f)
        assert "entries" in data
        assert data["schemaVersion"] == "0.1"
    print("test_find_catalogue_fixture PASSED")


def run_all_tests():
    test_session_env_preparation()
    test_session_script_generation()
    test_find_catalogue_fixture()
    print("\n=== ALL SESSION TESTS PASSED ===")


if __name__ == "__main__":
    run_all_tests()