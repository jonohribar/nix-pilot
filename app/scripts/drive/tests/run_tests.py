#!/usr/bin/env python3
"""
Run all np-drive unit tests.
"""

import sys
import os

# Add the np_drive package to path
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PKG_DIR = os.path.join(SCRIPT_DIR, "..")
sys.path.insert(0, PKG_DIR)

import test_snapshot
import test_repl
import test_driver
import test_session


def run_all():
    print("Running snapshot tests...")
    test_snapshot.run_all_tests()

    print("\nRunning REPL tests...")
    test_repl.run_all_tests()

    print("\nRunning driver tests...")
    test_driver.run_all_tests()

    print("\nRunning session tests...")
    test_session.run_all_tests()

    print("\n=== ALL TESTS PASSED ===")


if __name__ == "__main__":
    run_all()