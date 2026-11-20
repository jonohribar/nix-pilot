#!/usr/bin/env python3
"""
AT-SPI driver for nix-pilot integration tests.

This module provides a high-level API to drive the nix-pilot GTK4 application
through its AT-SPI accessibility tree. It launches the app under Xvfb +
dbus-run-session (which starts the session bus and at-spi-bus-launcher),
waits for the AT-SPI registry, and exposes helpers to navigate, click,
and assert on semantic properties (role + accessible name).
"""

import os
import sys
import time
import subprocess
import signal
from contextlib import contextmanager
from typing import Optional, List, Iterator

# Import AT-SPI bindings
import gi
gi.require_version("Atspi", "2.0")
from gi.repository import Atspi, GLib

# Disable dogtail's accessibility check (it requires GSettings schemas)
try:
    from dogtail.config import config
    config.checkForA11y = False
except ImportError:
    pass


class AtspiDriver:
    """High-level driver for nix-pilot via AT-SPI."""

    def __init__(self, app_binary: str, xvfb_display: Optional[str] = None):
        self.app_binary = app_binary
        # A unique display per run avoids stale Xvfb locks from previous or
        # orphaned sessions (a fixed ":99" fails after a timed-out run).
        self.xvfb_display = xvfb_display or (":%d" % (100 + os.getpid() % 5000))
        self.processes: List[subprocess.Popen] = []
        self.xvfb_proc: Optional[subprocess.Popen] = None
        self.session_proc: Optional[subprocess.Popen] = None  # dbus-run-session (wrapper)
        self.launcher: Optional[subprocess.Popen] = None  # at-spi-bus-launcher
        self.app_proc: Optional[subprocess.Popen] = None
        self.desktop: Optional[Atspi.Accessible] = None
        self.app: Optional[Atspi.Accessible] = None

    def _run_cmd(self, cmd: List[str], env: Optional[dict] = None) -> subprocess.Popen:
        """Run a command and track the process."""
        proc_env = os.environ.copy()
        if env:
            proc_env.update(env)
        proc = subprocess.Popen(cmd, env=proc_env, start_new_session=True)
        self.processes.append(proc)
        return proc

    def start_xvfb(self) -> None:
        """Start Xvfb virtual display."""
        # Clean up any stale lock
        lock_file = f"/tmp/.X{self.xvfb_display.split(':')[-1]}-lock"
        try:
            os.unlink(lock_file)
        except FileNotFoundError:
            pass

        self.xvfb_proc = self._run_cmd([
            "Xvfb", self.xvfb_display,
            "-screen", "0", "1280x800x24",
            "-ac", "+extension", "GLX", "+extension", "RANDR", "+extension", "RENDER"
        ])
        # Give Xvfb time to start
        time.sleep(1)
        os.environ["DISPLAY"] = self.xvfb_display

    def start_session_and_app(self, extra_env: Optional[dict] = None) -> None:
        """Start the a11y launcher and the app.

        The caller wraps the whole test in ONE `dbus-run-session` (see
        run-test.sh): the Python Atspi client below, the launcher, and the app
        must all share the same session bus, otherwise the app registers on a
        different bus than the one Atspi.get_desktop() reads and the test
        never finds it. Inside that single session we start the launcher
        first (so org.a11y.Bus owns a live registry before the app's GTK4
        init), then the app.
        """
        env = os.environ.copy()
        # Strip inherited bus addresses so children never reach the host's
        # session (or a stale one) — only the wrapping dbus-run-session's bus
        # must be used.
        for k in ("DBUS_SESSION_BUS_ADDRESS", "AT_SPI_BUS_ADDRESS"):
            env.pop(k, None)
        env.update({
            "DISPLAY": self.xvfb_display,
            "GDK_BACKEND": "x11",
            "GSK_RENDERER": "cairo",  # software rendering, no Vulkan/GPU needed
            "GTK_MODULES": "",  # GTK4 has built-in AT-SPI, no atk-bridge needed
            "XDG_DATA_HOME": "/tmp/nix-pilot-test-data",
            # Isolate the run-time dir: at-spi creates its bus socket under
            # $XDG_RUNTIME_DIR/at-spi/bus_<id>, and the shared /run/user/1000
            # namespace collects stale sockets from killed runs that let the
            # launcher reconnect to a dead bus ("unit failed").
            "XDG_RUNTIME_DIR": f"/tmp/nix-pilot-test-runtime-{os.getpid()}",
        })
        os.makedirs(env["XDG_RUNTIME_DIR"], exist_ok=True)
        if extra_env:
            env.update(extra_env)
        os.makedirs(env["XDG_DATA_HOME"], exist_ok=True)

        # Launcher first, then the app, in this same (wrapping) session bus.
        self.launcher = self._run_cmd(
            ["at-spi-bus-launcher", "--launch-immediately"], env=env)
        time.sleep(2)
        self.app_proc = self._run_cmd([self.app_binary], env=env)
        self._wait_for_app(timeout=40)

    def _wait_for_app(self, timeout: float = 30) -> None:
        """Wait for the nix-pilot app to appear in the accessibility tree.
        Matches by name containment so suffix/versioned app ids still hit."""
        Atspi.init()
        start = time.time()
        while time.time() - start < timeout:
            try:
                desktop = Atspi.get_desktop(0)
                for i in range(desktop.get_child_count()):
                    child = desktop.get_child_at_index(i)
                    if child.get_name() and "nix-pilot" in child.get_name():
                        self.desktop = desktop
                        self.app = child
                        return
            except Exception:
                pass
            time.sleep(0.2)
        raise RuntimeError(f"nix-pilot-app did not appear in AT-SPI tree within {timeout}s")

    def find_by_role_and_name(self, parent: Atspi.Accessible, role_name: str,
                               accessible_name: str, recursive: bool = True) -> Optional[Atspi.Accessible]:
        """Find a child by role name and accessible name."""
        for i in range(parent.get_child_count()):
            child = parent.get_child_at_index(i)
            if child.get_role_name() == role_name and child.get_name() == accessible_name:
                return child
            if recursive:
                found = self.find_by_role_and_name(child, role_name, accessible_name, True)
                if found:
                    return found
        return None

    def find_by_role(self, parent: Atspi.Accessible, role_name: str,
                     recursive: bool = True) -> List[Atspi.Accessible]:
        """Find all children matching a role name."""
        results = []
        for i in range(parent.get_child_count()):
            child = parent.get_child_at_index(i)
            if child.get_role_name() == role_name:
                results.append(child)
            if recursive:
                results.extend(self.find_by_role(child, role_name, True))
        return results

    def find_by_name(self, parent: Atspi.Accessible, accessible_name: str,
                     recursive: bool = True) -> List[Atspi.Accessible]:
        """Find all children matching an accessible name."""
        results = []
        for i in range(parent.get_child_count()):
            child = parent.get_child_at_index(i)
            if child.get_name() == accessible_name:
                results.append(child)
            if recursive:
                results.extend(self.find_by_name(child, accessible_name, True))
        return results

    def click(self, accessible: Atspi.Accessible) -> bool:
        """Perform a click action on an accessible."""
        # PyGObject exposes action names via get_action_name(i), not a
        # get_action(i) object. Try the common invoke/activate actions.
        for i in range(accessible.get_n_actions()):
            try:
                name = accessible.get_action_name(i) or ""
            except Exception:
                name = ""
            if name in ("click", "press", "activate", ""):
                accessible.do_action(i)
                return True
        # Some button-like nodes expose only a "activate" action; if nothing
        # matched above, activate the first action as a last resort.
        if accessible.get_n_actions() > 0:
            accessible.do_action(0)
            return True
        return False

    def _node_text(self, accessible: Atspi.Accessible) -> str:
        """The human-readable text of a node."""
        # GTK labels expose their text via the accessible name and role
        # "label"; that is the reliable source here (the PyGObject Text
        # interface get_text is mangled). Prefer a label descendant's name.
        if accessible.get_role_name() == "label" or accessible.get_role_name() == "heading":
            return accessible.get_name() or ""
        return ""

    def get_text(self, accessible: Atspi.Accessible) -> str:
        """Get the text content of an accessible, or its first non-empty
        descendant label (list items expose their text on a child label)."""
        own = self._node_text(accessible)
        if own:
            return own
        for child in _iter_children(accessible):
            t = self.get_text(child)
            if t:
                return t
        return ""

    def set_text(self, accessible: Atspi.Accessible, text: str) -> bool:
        """Set text on an editable text accessible."""
        editable = accessible.get_editable_text()
        if editable:
            editable.set_text_contents(text)
            return True
        return False

    def wait_for_window(self, title: str, timeout: float = 5) -> Optional[Atspi.Accessible]:
        """Wait for a window with the given title to appear."""
        start = time.time()
        while time.time() - start < timeout:
            for i in range(self.app.get_child_count()):
                child = self.app.get_child_at_index(i)
                if child.get_role_name() == "frame" and child.get_name() == title:
                    return child
            time.sleep(0.1)
        return None

    def dump_tree(self, accessible: Optional[Atspi.Accessible] = None, indent: int = 0) -> str:
        """Dump the accessibility tree for debugging."""
        if accessible is None:
            accessible = self.app
        lines = []
        prefix = "  " * indent
        lines.append(f"{prefix}{accessible.get_role_name()}: '{accessible.get_name()}'")
        for i in range(accessible.get_child_count()):
            child = accessible.get_child_at_index(i)
            lines.append(self.dump_tree(child, indent + 1))
        return "\n".join(lines)

    def cleanup(self) -> None:
        """Clean up all processes."""
        for proc in reversed(self.processes):
            if proc.poll() is None:
                try:
                    proc.terminate()
                    proc.wait(timeout=2)
                except subprocess.TimeoutExpired:
                    proc.kill()
                    proc.wait()


@contextmanager
def atspi_session(app_binary: str, xvfb_display: str = ":99"):
    """Context manager for a full AT-SPI test session."""
    driver = AtspiDriver(app_binary, xvfb_display)
    try:
        driver.start_xvfb()
        driver.start_session_and_app()
        yield driver
    finally:
        driver.cleanup()


def _iter_children(accessible: Atspi.Accessible) -> Iterator[Atspi.Accessible]:
    """Iterate over direct children of an accessible."""
    for i in range(accessible.get_child_count()):
        yield accessible.get_child_at_index(i)


def get_sidebar(driver: AtspiDriver) -> Optional[Atspi.Accessible]:
    """Get the sidebar container (the panel containing nav buttons)."""
    # Structure (GTK4/AT-SPI): frame -> (panel) -> panel (sidebar) -> buttons.
    # GTK4 reports nav buttons as role "button".
    frame = driver.app  # the app is the frame
    for child in _iter_children(frame):
        if child.get_role_name() == "panel":
            for grandchild in _iter_children(child):
                if grandchild.get_role_name() == "panel":
                    for btn in _iter_children(grandchild):
                        if btn.get_role_name() == "button" and btn.get_name() in ("Apps", "Settings", "Updates", "Your system"):
                            return grandchild
            # Also allow a single nested panel under a grouping (tree shape
            # seen in the live dump: frame -> panel -> panel { buttons }).
            for group in _iter_children(child):
                if group.get_role_name() == "panel":
                    for btn in _iter_children(group):
                        if btn.get_role_name() == "button" and btn.get_name() in ("Apps", "Settings", "Updates", "Your system"):
                            return group
    return None


def get_main_area(driver: AtspiDriver) -> Optional[Atspi.Accessible]:
    """Get the main content area (the panel containing the stack pages)."""
    frame = driver.app
    panels = list(_iter_children(frame))
    if len(panels) >= 2:
        # Second panel is usually the main area
        return panels[1]
    return None


# Convenience functions for common test patterns
def find_sidebar_nav(driver: AtspiDriver, label: str) -> Optional[Atspi.Accessible]:
    """Find a sidebar navigation button by its label."""
    sidebar = get_sidebar(driver)
    if sidebar:
        for child in _iter_children(sidebar):
            if child.get_role_name() == "button" and child.get_name() == label:
                return child
    # Fallback to recursive search (slower). GTK4 reports buttons as:
    # "button" (AT-SPI role button).
    return driver.find_by_role_and_name(driver.app, "button", label)


def _first_or_none(items: List[Atspi.Accessible]) -> Optional[Atspi.Accessible]:
    """First element of a list, or None."""
    return items[0] if items else None


def find_search_entry(driver: AtspiDriver) -> Optional[Atspi.Accessible]:
    """Find the store-local search entry (the one scoped to the catalogue).

    The app now has two search entries: the app-wide one at the top of the
    sidebar (fyq) and the store-local one inside the Apps tab. The store one
    is the entry whose ancestor page also holds the "Hide non-free" filter
    check box, so we locate that check box and take the first entry inside
    its page subtree.
    """
    check = driver.find_by_role_and_name(driver.app, "check box", "Hide non-free")
    scope = None
    if check is not None:
        # Walk up to a container that holds an entry (the store page box).
        parent = check
        while parent is not None and scope is None:
            parent = parent.get_parent()
            if parent is None:
                break
            entries = driver.find_by_role(parent, "entry")
            if entries:
                scope = parent
    if scope is None:
        scope = driver.app
    return _first_or_none(driver.find_by_role(scope, "entry"))


def find_listbox(driver: AtspiDriver) -> Optional[Atspi.Accessible]:
    """Find the main listbox (catalogue list)."""
    # The visible list is what the search/browse actually rebuilds. In the
    # tree, gtk::ListBox surfaces as a role "list" node; pick the first one
    # that has list-item children (the store), not the queued-changes list
    # (which shows "Nothing ready..." with no items on first browse).
    lists = driver.find_by_role(driver.app, "list")
    for lst in lists:
        if _first_or_none(list(_iter_children(lst))) is not None:
            return lst
    return _first_or_none(lists)


def find_stack_page(driver: AtspiDriver, page_name: str) -> Optional[Atspi.Accessible]:
    """Find a stack page by name (e.g., 'list', 'detail', 'settings', 'updates')."""
    return driver.find_by_name(driver.app, page_name)


if __name__ == "__main__":
    # Simple test: launch the app and dump the tree
    import argparse
    parser = argparse.ArgumentParser(description="AT-SPI driver test")
    parser.add_argument("app_binary", help="Path to nix-pilot-app binary")
    parser.add_argument("--dump", action="store_true", help="Dump accessibility tree")
    args = parser.parse_args()

    with atspi_session(args.app_binary) as driver:
        print("App started, waiting 2s for UI to settle...")
        time.sleep(2)
        if args.dump:
            print(driver.dump_tree())
        print("Test passed: app launched and accessible via AT-SPI")