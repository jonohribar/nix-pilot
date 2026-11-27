#!/usr/bin/env python3
"""
AT-SPI driver for np-drive.

Absorbs the existing app/scripts/a11y/driver.py helpers (AtspiDriver:
click, find_by_role, find_by_role_and_name, wait_for_window, atspi_session,
find_sidebar_nav, find_search_entry) keeping that API intact so
test_nav_settings.py and test_search.py still work.

Adds:
- Full AT-SPI state dump mapped onto the same Unit shape
- xdotool click fallback at bounds
- Screenshot via ImageMagick `import -window root`
"""

import os
import sys
import time
import subprocess
import shutil
import tempfile
from contextlib import contextmanager
from typing import Optional, List, Iterator, Dict, Any
from dataclasses import dataclass

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

# Import our snapshot types
from .snapshot import Unit, Snapshot, parse_snapshot


@dataclass
class AtspiNode:
    """Wrapper around an AT-SPI accessible with convenience methods."""
    accessible: Atspi.Accessible

    def get_role_name(self) -> str:
        return self.accessible.get_role_name()

    def get_name(self) -> str:
        return self.accessible.get_name() or ""

    def get_attributes(self) -> Dict[str, str]:
        """Get AT-SPI attributes as a dict."""
        attrs = {}
        try:
            attr_set = self.accessible.get_attributes()
            if attr_set:
                for i in range(attr_set.get_n_attributes()):
                    name = attr_set.get_attribute_name(i)
                    value = attr_set.get_attribute_value(i)
                    attrs[name] = value
        except Exception:
            pass
        return attrs

    def get_bounds(self) -> tuple[int, int, int, int]:
        """Get (x, y, w, h) in screen coordinates."""
        try:
            # Get component interface for geometry
            component = self.accessible.query_component()
            if component:
                x, y = component.get_position(Atspi.CoordType.SCREEN)
                w, h = component.get_size()
                return (x, y, w, h)
        except Exception:
            pass
        return (0, 0, 0, 0)

    def get_state_set(self) -> set:
        """Get the state set as a set of state names."""
        states = set()
        try:
            state_set = self.accessible.get_state_set()
            if state_set:
                for state in [
                    Atspi.StateType.VISIBLE,
                    Atspi.StateType.SENSITIVE,
                    Atspi.StateType.FOCUSED,
                    Atspi.StateType.SELECTED,
                    Atspi.StateType.ENABLED,
                    Atspi.StateType.SHOWING,
                ]:
                    if state_set.contains(state):
                        states.add(state.value_nick)
        except Exception:
            pass
        return states

    def get_text(self) -> str:
        """Get text content via Text interface if available."""
        try:
            text_iface = self.accessible.query_text()
            if text_iface:
                return text_iface.get_text(0, -1) or ""
        except Exception:
            pass
        return ""

    def get_children(self) -> List["AtspiNode"]:
        """Get direct children as AtspiNode wrappers."""
        children = []
        for i in range(self.accessible.get_child_count()):
            child = self.accessible.get_child_at_index(i)
            if child:
                children.append(AtspiNode(child))
        return children

    def do_action(self, action_index: int) -> bool:
        """Perform an action by index."""
        try:
            self.accessible.do_action(action_index)
            return True
        except Exception:
            return False

    def get_action_names(self) -> List[str]:
        """Get list of action names."""
        names = []
        for i in range(self.accessible.get_n_actions()):
            try:
                name = self.accessible.get_action_name(i) or ""
                names.append(name)
            except Exception:
                names.append("")
        return names

    def click(self) -> bool:
        """Perform a click action (prefers 'click', 'press', 'activate')."""
        for i, name in enumerate(self.get_action_names()):
            if name in ("click", "press", "activate", ""):
                return self.do_action(i)
        if self.accessible.get_n_actions() > 0:
            return self.do_action(0)
        return False


class AtspiDriver:
    """High-level driver for nix-pilot via AT-SPI (compatible with a11y/driver.py)."""

    def __init__(self, app_binary: str, xvfb_display: Optional[str] = None):
        self.app_binary = app_binary
        self.xvfb_display = (
            xvfb_display
            or os.environ.get("NIX_PILOT_A11Y_DISPLAY")
            or f":{100 + os.getpid() % 5000}"
        )
        self.processes: List[subprocess.Popen] = []
        self.xvfb_proc: Optional[subprocess.Popen] = None
        self.session_proc: Optional[subprocess.Popen] = None
        self.launcher: Optional[subprocess.Popen] = None
        self.app_proc: Optional[subprocess.Popen] = None
        self.desktop: Optional[Atspi.Accessible] = None
        self.app: Optional[Atspi.Accessible] = None
        self.test_home = tempfile.mkdtemp(prefix="nix-pilot-a11y-")

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
        self.xvfb_proc = self._run_cmd([
            "Xvfb", self.xvfb_display,
            "-screen", "0", "1280x800x24",
            "-ac", "+extension", "GLX", "+extension", "RANDR", "+extension", "RENDER"
        ])
        time.sleep(1)
        os.environ["DISPLAY"] = self.xvfb_display

    def start_session_and_app(self, extra_env: Optional[dict] = None) -> None:
        """Start the a11y launcher and the app inside the wrapping dbus-run-session."""
        env = os.environ.copy()
        env.pop("AT_SPI_BUS_ADDRESS", None)
        env.update({
            "DISPLAY": self.xvfb_display,
            "GDK_BACKEND": "x11",
            "GSK_RENDERER": "cairo",
            "GTK_MODULES": "",
            "ATSPI_DBUS_IMPLEMENTATION": "dbus-daemon",
            "XDG_DATA_HOME": os.path.join(self.test_home, "data"),
            "XDG_CONFIG_HOME": os.path.join(self.test_home, "config"),
            "XDG_RUNTIME_DIR": os.path.join(self.test_home, "runtime"),
        })
        if extra_env:
            env.update(extra_env)
        os.makedirs(env["XDG_RUNTIME_DIR"], mode=0o700, exist_ok=True)
        os.makedirs(env["XDG_DATA_HOME"], exist_ok=True)
        os.makedirs(env["XDG_CONFIG_HOME"], exist_ok=True)
        if not extra_env or "XDG_RUNTIME_DIR" not in extra_env:
            os.chmod(env["XDG_RUNTIME_DIR"], 0o700)

        self.launcher = self._run_cmd(
            ["at-spi-bus-launcher", "--launch-immediately"], env=env)
        time.sleep(2)
        self.app_proc = self._run_cmd([self.app_binary], env=env)
        self._wait_for_app(timeout=40)

    def _wait_for_app(self, timeout: float = 30) -> None:
        """Wait for the nix-pilot app to appear in the accessibility tree."""
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

    # --- Core query methods (kept compatible with a11y/driver.py) ---

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
        for i in range(accessible.get_n_actions()):
            try:
                name = accessible.get_action_name(i) or ""
            except Exception:
                name = ""
            if name in ("click", "press", "activate", ""):
                accessible.do_action(i)
                return True
        if accessible.get_n_actions() > 0:
            accessible.do_action(0)
            return True
        return False

    def _node_text(self, accessible: Atspi.Accessible) -> str:
        """The human-readable text of a node."""
        if accessible.get_role_name() == "label" or accessible.get_role_name() == "heading":
            return accessible.get_name() or ""
        return ""

    def get_text(self, accessible: Atspi.Accessible) -> str:
        """Get the text content of an accessible, or its first non-empty descendant label."""
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
        shutil.rmtree(self.test_home, ignore_errors=True)


@contextmanager
def atspi_session(app_binary: str, xvfb_display: Optional[str] = None):
    """Context manager for a full AT-SPI test session (compatible with a11y/driver.py)."""
    driver = AtspiDriver(app_binary, xvfb_display)
    try:
        driver.start_xvfb()
        driver.start_session_and_app()
        yield driver
    finally:
        driver.cleanup()


# --- Convenience functions (kept compatible with a11y/driver.py) ---

def _iter_children(accessible: Atspi.Accessible) -> Iterator[Atspi.Accessible]:
    """Iterate over direct children of an accessible."""
    for i in range(accessible.get_child_count()):
        yield accessible.get_child_at_index(i)


def _first_or_none(items: List[Atspi.Accessible]) -> Optional[Atspi.Accessible]:
    """First element of a list, or None."""
    return items[0] if items else None


def get_sidebar(driver: AtspiDriver) -> Optional[Atspi.Accessible]:
    """Get the sidebar container (the panel containing nav buttons)."""
    frame = driver.app
    for child in _iter_children(frame):
        if child.get_role_name() == "panel":
            for grandchild in _iter_children(child):
                if grandchild.get_role_name() == "panel":
                    for btn in _iter_children(grandchild):
                        if btn.get_role_name() == "button" and btn.get_name() in ("Packages", "Settings", "Updates", "Your system"):
                            return grandchild
            for group in _iter_children(child):
                if group.get_role_name() == "panel":
                    for btn in _iter_children(group):
                        if btn.get_role_name() == "button" and btn.get_name() in ("Packages", "Settings", "Updates", "Your system"):
                            return group
    return None


def get_main_area(driver: AtspiDriver) -> Optional[Atspi.Accessible]:
    """Get the main content area (the panel containing the stack pages)."""
    frame = driver.app
    panels = list(_iter_children(frame))
    if len(panels) >= 2:
        return panels[1]
    return None


def find_sidebar_nav(driver: AtspiDriver, label: str) -> Optional[Atspi.Accessible]:
    """Find a sidebar navigation button by its label."""
    sidebar = get_sidebar(driver)
    if sidebar:
        for child in _iter_children(sidebar):
            if child.get_role_name() == "button" and child.get_name() == label:
                return child
    return driver.find_by_role_and_name(driver.app, "button", label)


def find_search_entry(driver: AtspiDriver) -> Optional[Atspi.Accessible]:
    """Find the store-local search entry."""
    check = driver.find_by_role_and_name(
        driver.app, "check box", "Hide packages with licence restrictions"
    )
    scope = None
    if check is not None:
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
    lists = driver.find_by_role(driver.app, "list")
    for lst in lists:
        if _first_or_none(list(_iter_children(lst))) is not None:
            return lst
    return _first_or_none(lists)


def find_stack_page(driver: AtspiDriver, page_name: str) -> Optional[Atspi.Accessible]:
    """Find a stack page by name (e.g., 'list', 'detail', 'settings', 'updates')."""
    return driver.find_by_name(driver.app, page_name)


# --- New functionality for np-drive ---

def atspi_to_units(root: Atspi.Accessible) -> List[Unit]:
    """
    Walk the AT-SPI tree and produce a list of Unit objects matching the
    snapshot schema. Only includes widgets that have meaningful geometry
    and are user-visible (like the geom_dump does).
    """
    units = []
    uid = 0

    def walk(acc: Atspi.Accessible, parent_window: Optional[Atspi.Accessible] = None):
        nonlocal uid
        try:
            role_name = acc.get_role_name()
            name = acc.get_name() or ""
        except Exception:
            return

        # Only include widget kinds that geom_dump classifies
        kind = _classify_role(role_name)
        if kind is None:
            # Still recurse into children
            for child in _iter_children(acc):
                walk(child, parent_window)
            return

        # Get geometry
        try:
            component = acc.query_component()
            if component:
                x, y = component.get_position(Atspi.CoordType.SCREEN)
                w, h = component.get_size()
            else:
                return
        except Exception:
            return

        # Skip zero-size or unrealized
        if w <= 0 or h <= 0:
            for child in _iter_children(acc):
                walk(child, parent_window)
            return

        # Get states
        states = set()
        try:
            state_set = acc.get_state_set()
            if state_set:
                for state in [Atspi.StateType.VISIBLE, Atspi.StateType.SENSITIVE,
                              Atspi.StateType.FOCUSED, Atspi.StateType.SHOWING]:
                    if state_set.contains(state):
                        states.add(state.value_nick)
        except Exception:
            pass

        visible = "visible" in states or "showing" in states
        sensitive = "sensitive" in states
        focused = "focused" in states

        # Collect text (label + tooltip)
        text_parts = []
        if name:
            text_parts.append(name)
        try:
            tooltip = acc.get_tooltip_text()
            if tooltip:
                text_parts.append(f"tip:{tooltip}")
        except Exception:
            pass
        # Also get text from child labels
        for child in _iter_children(acc):
            if child.get_role_name() in ("label", "heading"):
                child_name = child.get_name() or ""
                if child_name and child_name not in text_parts:
                    text_parts.append(child_name)

        label = " | ".join(text_parts) if text_parts else name

        units.append(Unit(
            id=uid,
            kind=kind,
            role=role_name,
            name=name,
            label=label,
            x=x, y=y, w=w, h=h,
            visible=visible,
            sensitive=sensitive,
            focused=focused,
        ))
        uid += 1

        for child in _iter_children(acc):
            walk(child, parent_window)

    walk(root)
    return units


def _classify_role(role_name: str) -> Optional[str]:
    """Map AT-SPI role to geom_dump kind."""
    role_lower = role_name.lower()
    if role_lower in ("push button", "button", "toggle button", "check box", "radio button"):
        return "button"
    if role_lower in ("entry", "search entry", "spin button", "combo box"):
        return "entry"
    if role_lower in ("list item", "row"):
        return "row"
    if role_lower in ("list", "tree", "tree table"):
        return "list"
    if role_lower in ("scroll pane", "viewport"):
        return "scroll"
    if role_lower in ("switch",):
        return "switch"
    return None


def dump_atspi_state(driver: AtspiDriver) -> Snapshot:
    """
    Produce a full Snapshot from the live AT-SPI tree.
    Mirrors the geom_dump schema: window, page, units.
    """
    if not driver.app:
        raise RuntimeError("App not connected to AT-SPI")

    # Get window geometry from the app frame
    window_x, window_y, window_w, window_h = 0, 0, 1280, 800
    try:
        component = driver.app.query_component()
        if component:
            window_x, window_y = component.get_position(Atspi.CoordType.SCREEN)
            window_w, window_h = component.get_size()
    except Exception:
        pass

    # Determine visible page by finding the visible stack child
    page = None
    for child in _iter_children(driver.app):
        if child.get_role_name() in ("panel", "frame", "scroll pane"):
            # Look for stack page names
            if child.get_name() in ("list", "detail", "settings", "updates", "your-system", "welcome"):
                states = set()
                try:
                    state_set = child.get_state_set()
                    if state_set and state_set.contains(Atspi.StateType.VISIBLE):
                        page = child.get_name()
                        break
                except Exception:
                    pass

    units = atspi_to_units(driver.app)

    return Snapshot(
        window_x=window_x,
        window_y=window_y,
        window_w=window_w,
        window_h=window_h,
        page=page,
        units=units,
    )


def xdotool_click(x: int, y: int, display: Optional[str] = None) -> bool:
    """
    Fallback click using xdotool at absolute screen coordinates.
    Requires xdotool to be installed and DISPLAY set.
    """
    display = display or os.environ.get("DISPLAY", ":0")
    try:
        subprocess.run(
            ["xdotool", "mousemove", "--sync", str(x), str(y)],
            env={**os.environ, "DISPLAY": display},
            check=True, timeout=5
        )
        subprocess.run(
            ["xdotool", "click", "1"],
            env={**os.environ, "DISPLAY": display},
            check=True, timeout=5
        )
        return True
    except (subprocess.CalledProcessError, subprocess.TimeoutExpired, FileNotFoundError):
        return False


def xdotool_type(text: str, display: Optional[str] = None) -> bool:
    """Type text using xdotool."""
    display = display or os.environ.get("DISPLAY", ":0")
    try:
        subprocess.run(
            ["xdotool", "type", "--delay", "10", text],
            env={**os.environ, "DISPLAY": display},
            check=True, timeout=10
        )
        return True
    except (subprocess.CalledProcessError, subprocess.TimeoutExpired, FileNotFoundError):
        return False


def xdotool_key(combo: str, display: Optional[str] = None) -> bool:
    """Send a key combination using xdotool (e.g., 'ctrl+f', 'Escape')."""
    display = display or os.environ.get("DISPLAY", ":0")
    try:
        subprocess.run(
            ["xdotool", "key", combo],
            env={**os.environ, "DISPLAY": display},
            check=True, timeout=5
        )
        return True
    except (subprocess.CalledProcessError, subprocess.TimeoutExpired, FileNotFoundError):
        return False


def screenshot(path: str, display: Optional[str] = None) -> bool:
    """
    Take a screenshot of the root window using ImageMagick's `import`.
    Saves to the given path.
    """
    display = display or os.environ.get("DISPLAY", ":0")
    try:
        subprocess.run(
            ["import", "-window", "root", path],
            env={**os.environ, "DISPLAY": display},
            check=True, timeout=10
        )
        return True
    except (subprocess.CalledProcessError, subprocess.TimeoutExpired, FileNotFoundError):
        return False


def click_unit(driver: AtspiDriver, unit: Unit, use_fallback: bool = True) -> bool:
    """
    Click a unit: try AT-SPI action first, fall back to xdotool at unit center.
    """
    # Try AT-SPI click via find_by_role_and_name (need to locate the accessible)
    # We don't have the accessible object directly from Unit, so try by name+role
    accessible = None
    for child in _iter_children(driver.app):
        if _find_unit_accessible(child, unit):
            accessible = _find_unit_accessible(child, unit)
            break

    if accessible and driver.click(accessible):
        return True

    if use_fallback:
        cx, cy = unit.center
        return xdotool_click(cx, cy, driver.xvfb_display)

    return False


def _find_unit_accessible(root: Atspi.Accessible, target: Unit) -> Optional[Atspi.Accessible]:
    """Find the AT-SPI accessible matching a Unit (by bounds + name)."""
    try:
        component = root.query_component()
        if component:
            x, y = component.get_position(Atspi.CoordType.SCREEN)
            w, h = component.get_size()
            if x == target.x and y == target.y and w == target.w and h == target.h:
                if root.get_name() == target.name:
                    return root
    except Exception:
        pass
    for child in _iter_children(root):
        found = _find_unit_accessible(child, target)
        if found:
            return found
    return None


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description="AT-SPI driver test")
    parser.add_argument("app_binary", help="Path to nix-pilot-app binary")
    parser.add_argument("--dump", action="store_true", help="Dump accessibility tree")
    parser.add_argument("--dump-state", action="store_true", help="Dump AT-SPI state as Unit snapshot")
    args = parser.parse_args()

    with atspi_session(args.app_binary) as driver:
        print("App started, waiting 2s for UI to settle...")
        time.sleep(2)
        if args.dump:
            print(driver.dump_tree())
        if args.dump_state:
            snap = dump_atspi_state(driver)
            print(f"Window: ({snap.window_x},{snap.window_y}) {snap.window_w}x{snap.window_h}")
            print(f"Page: {snap.page}")
            print(f"Units: {len(snap.units)}")
            for u in snap.units[:20]:
                print(f"  [{u.id}] {u.kind} {u.role} '{u.name}' ({u.x},{u.y},{u.w},{u.h}) vis={u.visible} sens={u.sensitive} foc={u.focused}")
        print("Test passed: app launched and accessible via AT-SPI")