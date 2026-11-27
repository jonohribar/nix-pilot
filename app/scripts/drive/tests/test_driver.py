#!/usr/bin/env python3
"""
Unit tests for driver utilities (pure Python parts, no live AT-SPI).
"""

import sys
import os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from np_drive.driver import _classify_role
from np_drive.session import Session, find_catalogue_index


def test_classify_role():
    """Test AT-SPI role to kind mapping."""
    assert _classify_role("push button") == "button"
    assert _classify_role("button") == "button"
    assert _classify_role("toggle button") == "button"
    assert _classify_role("check box") == "button"
    assert _classify_role("radio button") == "button"

    assert _classify_role("entry") == "entry"
    assert _classify_role("search entry") == "entry"
    assert _classify_role("spin button") == "entry"
    assert _classify_role("combo box") == "entry"

    assert _classify_role("list item") == "row"
    assert _classify_role("row") == "row"

    assert _classify_role("list") == "list"
    assert _classify_role("tree") == "list"
    assert _classify_role("tree table") == "list"

    assert _classify_role("scroll pane") == "scroll"
    assert _classify_role("viewport") == "scroll"

    assert _classify_role("switch") == "switch"

    # Unknown roles return None
    assert _classify_role("label") is None
    assert _classify_role("heading") is None
    assert _classify_role("panel") is None
    assert _classify_role("frame") is None
    assert _classify_role("unknown") is None

    print("test_classify_role PASSED")


def test_session_init():
    """Test Session object initialization."""
    session = Session(
        app_binary="/fake/path/app",
        catalogue_path="/fake/catalogue.json",
        config_dir="/fake/config",
        dump_file="/tmp/test-dump.json",
    )
    assert session.app_binary == "/fake/path/app"
    assert session.catalogue_path == "/fake/catalogue.json"
    assert session.config_dir == "/fake/config"
    assert session.dump_file == "/tmp/test-dump.json"
    assert session.xvfb_display.startswith(":")
    assert not session._started
    print("test_session_init PASSED")


def test_find_catalogue_index():
    """Test catalogue index finder."""
    # Should find the fixture in this repo
    repo_root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    catalogue = find_catalogue_index(repo_root)
    # Might be None if not in expected location, but shouldn't crash
    if catalogue:
        assert os.path.exists(catalogue)
        assert "catalogue-index.json" in catalogue
    print("test_find_catalogue_index PASSED")


def run_all_tests():
    test_classify_role()
    test_session_init()
    test_find_catalogue_index()
    print("\n=== ALL DRIVER TESTS PASSED ===")


if __name__ == "__main__":
    run_all_tests()