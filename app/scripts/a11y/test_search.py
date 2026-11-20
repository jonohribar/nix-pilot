#!/usr/bin/env python3
"""
Test: Type in the search entry -> results change.
Asserts via the semantic tree (role + accessible name), not pixels.
"""

import sys
import os
sys.path.insert(0, os.path.dirname(__file__))

from driver import atspi_session, find_search_entry, find_listbox
import time


def test_search(app_binary: str) -> bool:
    """Test typing in search entry filters the results list."""
    with atspi_session(app_binary) as driver:
        # Wait for UI to settle
        time.sleep(2)

        # Find the search entry
        search_entry = find_search_entry(driver)
        if not search_entry:
            print("FAIL: Could not find search entry")
            print(driver.dump_tree())
            return False

        print(f"Found search entry: role={search_entry.get_role_name()}, name='{search_entry.get_name()}'")

        # Find the listbox (catalogue list)
        listbox = find_listbox(driver)
        if not listbox:
            print("FAIL: Could not find listbox")
            print(driver.dump_tree())
            return False

        # Get initial row count (browse mode shows ~60 entries)
        initial_rows = driver.find_by_role(listbox, "list item")
        initial_count = len(initial_rows)
        print(f"Initial list item count: {initial_count}")

        if initial_count == 0:
            print("FAIL: No items in initial list")
            return False

        # Type a search query that should match something
        query = "firefox"
        if not driver.set_text(search_entry, query):
            print(f"FAIL: Could not set search text to '{query}'")
            return False

        # Wait for search to complete and list to update
        time.sleep(1.5)

        # Get new row count
        filtered_rows = driver.find_by_role(listbox, "list item")
        filtered_count = len(filtered_rows)
        print(f"Filtered list item count (query='{query}'): {filtered_count}")

        # The list should have changed (either fewer results or different ones)
        if filtered_count == initial_count:
            print("FAIL: Search did not change the result count")
            return False

        # Verify at least one result contains the query text (case-insensitive)
        found_match = False
        for row in filtered_rows:
            row_text = driver.get_text(row).lower()
            if query.lower() in row_text:
                found_match = True
                print(f"  Found match: {row_text[:80]}")
                break

        if not found_match and filtered_count > 0:
            print(f"FAIL: No results matched query '{query}'")
            return False

        # Clear the search (press Escape or set empty text)
        if not driver.set_text(search_entry, ""):
            print("FAIL: Could not clear search text")
            return False

        time.sleep(1)

        # Verify we're back to browse mode
        cleared_rows = driver.find_by_role(listbox, "list item")
        cleared_count = len(cleared_rows)
        print(f"After clearing search, list item count: {cleared_count}")

        if cleared_count != initial_count:
            print("FAIL: Clearing search did not restore initial count")
            return False

        print("SUCCESS: Search filtering works correctly")
        return True


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("app_binary", help="Path to nix-pilot-app binary")
    args = parser.parse_args()

    success = test_search(args.app_binary)
    sys.exit(0 if success else 1)