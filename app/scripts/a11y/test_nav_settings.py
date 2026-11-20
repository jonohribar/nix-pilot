#!/usr/bin/env python3
"""
Test: Click the Settings sidebar nav -> the Settings home shows.

Asserts via the semantic tree (role + accessible name), not pixels: after
clicking the Settings sidebar button, the Settings title and its category
nav rows (System, Network & internet, ...) must be present in the tree.
"""

import sys
import os
sys.path.insert(0, os.path.dirname(__file__))

from driver import atspi_session, find_sidebar_nav
import time


def test_nav_settings(app_binary: str) -> bool:
    """Click Settings and assert its home content appears."""
    with atspi_session(app_binary) as driver:
        # Wait for UI to settle
        time.sleep(2)

        settings_btn = find_sidebar_nav(driver, "Settings")
        if not settings_btn:
            print("FAIL: Could not find 'Settings' nav button")
            return False
        print(
            f"Found Settings button: role={settings_btn.get_role_name()}, "
            f"name='{settings_btn.get_name()}'"
        )

        if not driver.click(settings_btn):
            print("FAIL: Could not click Settings button")
            return False

        # After the click the Settings home must be the visible surface: its
        # title and its category nav rows (System, Network & internet, ...)
        # are present. The sidebar's own nav buttons would be there anyway,
        # so the discriminating signal is the Settings-only category rows plus
        # the profile header (@account) from the Settings home.
        time.sleep(1.5)
        names = set()
        for a in driver.find_by_role(driver.app, "button"):
            if a.get_name():
                names.add(a.get_name())
        for a in driver.find_by_role(driver.app, "label"):
            if a.get_name():
                names.add(a.get_name())

        required = {"Settings", "System", "Network & internet"}
        missing = required - names
        if missing:
            print(f"FAIL: Settings home content missing after click: {sorted(missing)}")
            return False

        print("SUCCESS: Settings navigation works correctly")
        return True


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("app_binary", help="Path to nix-pilot-app binary")
    args = parser.parse_args()

    sys.exit(0 if test_nav_settings(args.app_binary) else 1)