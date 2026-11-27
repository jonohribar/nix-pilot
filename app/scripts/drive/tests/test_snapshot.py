#!/usr/bin/env python3
"""
Unit tests for snapshot parsing and analysis.
"""

import sys
import os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from np_drive.snapshot import (
    Unit, Snapshot, parse_snapshot, load_snapshot,
    summarize, diff_snapshots, Summary, DiffResult,
    format_summary, format_diff,
)

FIXTURE_DIR = os.path.join(os.path.dirname(__file__))
FIXTURE_SNAPSHOT = os.path.join(FIXTURE_DIR, "fixture_snapshot.json")


def test_parse_snapshot():
    """Test parsing a JSON snapshot."""
    json_text = '''
    {
      "window": {"x": 0, "y": 0, "w": 800, "h": 600},
      "page": "settings",
      "units": [
        {
          "id": 0,
          "kind": "button",
          "role": "push button",
          "name": "Apply",
          "label": "Apply | tip:Apply changes",
          "x": 10, "y": 10, "w": 80, "h": 30,
          "visible": true, "sensitive": true, "focused": false
        }
      ]
    }
    '''
    snap = parse_snapshot(json_text)
    assert snap.window_x == 0
    assert snap.window_y == 0
    assert snap.window_w == 800
    assert snap.window_h == 600
    assert snap.page == "settings"
    assert len(snap.units) == 1
    u = snap.units[0]
    assert u.id == 0
    assert u.kind == "button"
    assert u.name == "Apply"
    assert u.center == (50, 25)
    print("test_parse_snapshot PASSED")


def test_load_snapshot():
    """Test loading snapshot from file."""
    snap = load_snapshot(FIXTURE_SNAPSHOT)
    assert snap.window_w == 1180
    assert snap.window_h == 800
    assert snap.page == "list"
    assert len(snap.units) == 14
    print("test_load_snapshot PASSED")


def test_find_by_id():
    """Test finding unit by ID."""
    snap = load_snapshot(FIXTURE_SNAPSHOT)
    u = snap.find_by_id(4)
    assert u is not None
    assert u.kind == "entry"
    assert u.name == "Searches packages and settings everywhere"

    # Non-existent ID
    assert snap.find_by_id(999) is None
    print("test_find_by_id PASSED")


def test_find_by_name():
    """Test finding units by exact name."""
    snap = load_snapshot(FIXTURE_SNAPSHOT)
    units = snap.find_by_name("Settings", exact=True)
    assert len(units) == 1
    assert units[0].id == 1

    units = snap.find_by_name("Install", exact=True)
    assert len(units) == 2  # Two Install buttons (ids 11, 12)
    print("test_find_by_name PASSED")


def test_find_by_label_substring():
    """Test finding units by label substring."""
    snap = load_snapshot(FIXTURE_SNAPSHOT)
    units = snap.find_by_label_substring("firefox")
    assert len(units) == 2  # firefox row + Install button with firefox tooltip
    ids = {u.id for u in units}
    assert 6 in ids  # firefox row
    assert 11 in ids  # Install button with firefox tooltip

    units = snap.find_by_label_substring("Install")
    assert len(units) == 2  # Two Install buttons
    print("test_find_by_label_substring PASSED")


def test_unit_properties():
    """Test Unit computed properties."""
    snap = load_snapshot(FIXTURE_SNAPSHOT)

    # Interactive units
    interactive = snap.interactive_units
    kinds = {u.kind for u in interactive}
    assert kinds == {"button", "switch", "entry", "row"}

    # Visible/hidden
    visible = snap.visible_units
    hidden = snap.hidden_units
    assert len(visible) == 13  # One hidden (id 11)
    assert len(hidden) == 1
    assert hidden[0].id == 11

    # Offscreen
    offscreen = snap.offscreen_units
    assert len(offscreen) == 1
    assert offscreen[0].id == 13

    # Focused
    focused = snap.focused_unit
    assert focused is not None
    assert focused.id == 4

    # Unnamed interactive
    unnamed = [u for u in interactive if u.is_unnamed]
    assert len(unnamed) == 1
    assert unnamed[0].id == 9

    print("test_unit_properties PASSED")


def test_click_point():
    """Test click point (center of bounds) calculation."""
    u = Unit(
        id=0, kind="button", role="push button", name="Test", label="Test",
        x=10, y=20, w=100, h=50,
        visible=True, sensitive=True, focused=False
    )
    assert u.center == (60, 45)
    assert u.rect == (10, 20, 100, 50)
    print("test_click_point PASSED")


def test_summarize():
    """Test snapshot summarization."""
    snap = load_snapshot(FIXTURE_SNAPSHOT)
    summary = summarize(snap)

    # Counts by kind
    assert "button" in summary.counts_by_kind
    assert "entry" in summary.counts_by_kind
    assert "row" in summary.counts_by_kind
    assert "switch" in summary.counts_by_kind

    # Unnamed interactive
    assert len(summary.unnamed_interactive) == 1
    assert summary.unnamed_interactive[0].id == 9

    # Duplicate names (two "Install" buttons)
    assert "Install" in summary.duplicate_names
    assert len(summary.duplicate_names["Install"]) == 2

    # Hidden units
    assert len(summary.hidden_units) == 1
    assert summary.hidden_units[0].id == 11

    # Offscreen units
    assert len(summary.offscreen_units) == 1
    assert summary.offscreen_units[0].id == 13

    # Focused unit
    assert summary.focused_unit_id == 4

    # Page
    assert summary.visible_page == "list"

    # Total visible text length
    assert summary.total_visible_text_length > 0

    print("test_summarize PASSED")


def test_diff_snapshots():
    """Test diffing two snapshots."""
    snap1 = load_snapshot(FIXTURE_SNAPSHOT)

    # Create a modified snapshot
    import json
    with open(FIXTURE_SNAPSHOT) as f:
        data = json.load(f)

    # Modify: change page, move a unit, change visibility
    data["page"] = "detail"
    data["units"][6]["x"] = 200  # Move firefox row
    data["units"][6]["y"] = 200
    data["units"][11]["visible"] = True  # Show hidden Install button
    data["units"][9]["name"] = "Now Named"  # Rename unnamed button

    snap2 = parse_snapshot(json.dumps(data))

    diff = diff_snapshots(snap1, snap2)

    # No units appeared/disappeared (same IDs)
    assert len(diff.appeared) == 0
    assert len(diff.disappeared) == 0

    # One moved (firefox row id 6)
    assert len(diff.moved) == 1
    assert diff.moved[0][0].id == 6

    # One visibility changed (Install button id 11)
    assert len(diff.changed_visible) == 1
    assert diff.changed_visible[0][0].id == 11

    # One name changed (id 9)
    assert len(diff.changed_name) == 1
    assert diff.changed_name[0][0].id == 9

    print("test_diff_snapshots PASSED")


def test_format_output():
    """Test formatting functions don't crash."""
    snap = load_snapshot(FIXTURE_SNAPSHOT)
    summary = summarize(snap)
    diff = diff_snapshots(snap, snap)

    summary_str = format_summary(summary)
    assert "Snapshot Summary" in summary_str
    assert "counts_by_kind" in summary_str.lower() or "Counts by kind" in summary_str

    diff_str = format_diff(diff)
    assert "Snapshot Diff" in diff_str

    print("test_format_output PASSED")


def test_edge_cases():
    """Test edge cases."""
    # Empty snapshot
    empty = Snapshot(0, 0, 100, 100, None, [])
    summary = summarize(empty)
    assert summary.counts_by_kind == {}
    assert summary.unnamed_interactive == []
    assert summary.duplicate_names == {}
    assert summary.hidden_units == []
    assert summary.offscreen_units == []
    assert summary.focused_unit_id is None
    assert summary.visible_page is None
    assert summary.total_visible_text_length == 0

    # Snapshot with null page
    json_text = '{"window":{"x":0,"y":0,"w":10,"h":10},"page":null,"units":[]}'
    snap = parse_snapshot(json_text)
    assert snap.page is None

    print("test_edge_cases PASSED")


def run_all_tests():
    """Run all tests."""
    test_parse_snapshot()
    test_load_snapshot()
    test_find_by_id()
    test_find_by_name()
    test_find_by_label_substring()
    test_unit_properties()
    test_click_point()
    test_summarize()
    test_diff_snapshots()
    test_format_output()
    test_edge_cases()
    print("\n=== ALL TESTS PASSED ===")


if __name__ == "__main__":
    run_all_tests()