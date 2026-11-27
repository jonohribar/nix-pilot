#!/usr/bin/env python3
"""
Unit tests for REPL commands and assertions.
"""

import sys
import os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from np_drive.snapshot import load_snapshot, diff_snapshots, parse_snapshot
from np_drive.repl import Repl, ReplState

FIXTURE_DIR = os.path.join(os.path.dirname(__file__))
FIXTURE_SNAPSHOT = os.path.join(FIXTURE_DIR, "fixture_snapshot.json")


def make_repl():
    """Create a Repl with the fixture snapshot loaded."""
    state = ReplState(dump_file=FIXTURE_SNAPSHOT)
    repl = Repl(state)
    # Force load the fixture
    repl._refresh_snapshot()
    return repl


def test_assert_unit_exists():
    """Test 'assert unit:<name>' for visible unit."""
    repl = make_repl()
    assert repl.cmd_assert(["unit:Packages"]) == True
    assert repl.cmd_assert(["unit:Settings"]) == True
    assert repl.cmd_assert(["unit:Apply changes"]) == True
    print("test_assert_unit_exists PASSED")


def test_assert_unit_not_visible():
    """Test 'assert unit:<name>' fails for non-existent unit."""
    repl = make_repl()
    # Non-existent unit
    assert repl.cmd_assert(["unit:NonExistent"]) == False
    print("test_assert_unit_not_visible PASSED")


def test_assert_hidden():
    """Test 'assert hidden:<name>'."""
    repl = make_repl()
    # Install button id 11 is hidden
    assert repl.cmd_assert(["hidden:Install"]) == True
    # Packages is visible
    assert repl.cmd_assert(["hidden:Packages"]) == False
    print("test_assert_hidden PASSED")


def test_assert_text():
    """Test 'assert text:<substring>'."""
    repl = make_repl()
    assert repl.cmd_assert(["text:firefox"]) == True
    assert repl.cmd_assert(["text:Firefox"]) == True  # Case insensitive
    assert repl.cmd_assert(["text:Web browser"]) == True
    assert repl.cmd_assert(["text:NonExistentText"]) == False
    print("test_assert_text PASSED")


def test_assert_page():
    """Test 'assert page:<id>'."""
    repl = make_repl()
    assert repl.cmd_assert(["page:list"]) == True
    assert repl.cmd_assert(["page:settings"]) == False
    assert repl.cmd_assert(["page:detail"]) == False
    print("test_assert_page PASSED")


def test_assert_units_count():
    """Test 'assert units == N'."""
    repl = make_repl()
    assert repl.cmd_assert(["units == 14"]) == True
    assert repl.cmd_assert(["units == 10"]) == False
    print("test_assert_units_count PASSED")


def test_assert_none_unnamed():
    """Test 'assert none-unnamed'."""
    repl = make_repl()
    # Fixture has one unnamed interactive unit (id 9)
    assert repl.cmd_assert(["none-unnamed"]) == False
    print("test_assert_none_unnamed PASSED")


def test_changed_diff():
    """Test 'changed' command diffs against previous snapshot."""
    repl = make_repl()

    # No previous snapshot yet
    assert repl.cmd_changed([]) == True  # Just prints message

    # Create a modified snapshot as "previous"
    import json
    with open(FIXTURE_SNAPSHOT) as f:
        data = json.load(f)

    # Modify
    data["page"] = "detail"
    data["units"][6]["x"] = 200
    data["units"][6]["y"] = 200
    data["units"][11]["visible"] = True
    data["units"][9]["name"] = "Now Named"

    prev_snap = parse_snapshot(json.dumps(data))
    repl.state.previous_snapshot = prev_snap
    repl.state.current_snapshot = load_snapshot(FIXTURE_SNAPSHOT)

    # Should not fail, just print diff
    assert repl.cmd_changed([]) == True
    print("test_changed_diff PASSED")


def test_find_unit_by_id_name_label():
    """Test _find_unit helper."""
    repl = make_repl()

    # By ID
    u = repl._find_unit("4")
    assert u is not None
    assert u.id == 4

    # By exact name
    u = repl._find_unit("Settings")
    assert u is not None
    assert u.id == 1

    # By label substring - use more specific substring to avoid ambiguity
    u = repl._find_unit("Mozilla Firefox")
    assert u is not None
    assert u.id == 6

    # Not found
    u = repl._find_unit("NonExistent")
    assert u is None

    print("test_find_unit_by_id_name_label PASSED")


def test_scenario_runner():
    """Test running a simple scenario file."""
    # Create a temp scenario file
    scenario_content = """# Test scenario
state
assert unit:Packages
assert page:list
assert units == 14
# This should fail
assert unit:NonExistent
"""
    import tempfile
    with tempfile.NamedTemporaryFile(mode="w", suffix=".drv", delete=False) as f:
        f.write(scenario_content)
        scenario_path = f.name

    try:
        state = ReplState(dump_file=FIXTURE_SNAPSHOT)
        repl = Repl(state)
        repl._refresh_snapshot()

        # Run scenario - should stop at the failing assert
        result = repl.run_scenario(scenario_path)
        assert result == 1  # Non-zero exit on assert failure
    finally:
        os.unlink(scenario_path)

    print("test_scenario_runner PASSED")


def run_all_tests():
    test_assert_unit_exists()
    test_assert_unit_not_visible()
    test_assert_hidden()
    test_assert_text()
    test_assert_page()
    test_assert_units_count()
    test_assert_none_unnamed()
    test_changed_diff()
    test_find_unit_by_id_name_label()
    test_scenario_runner()
    print("\n=== ALL REPL TESTS PASSED ===")


if __name__ == "__main__":
    run_all_tests()