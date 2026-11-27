#!/usr/bin/env python3
"""
Interactive REPL and scenario runner for np-drive.

Commands:
- state [filter]          — numbered units with id/kind/role/name/bounds/visible/sensitive/focused
- units [role]            — shorthand for all interactive units
- text [filter]           — all text in the window (labels + button text)
- describe <id>           — every field of one unit
- focus                   — the focused unit
- page                    — the visible stack page
- changed                 — diff vs the previous snapshot
- summary                 — counts by role; unnamed interactive; duplicate names; hidden/offscreen; focused; page; text length
- click <id|name|label>   — AT-SPI action, else xdotool at unit bounds
- type <text>             — keyboard input to focused unit
- type-into <id> <text>   — click unit then type
- key <combo>             — key chords (e.g., ctrl+f, Escape)
- open <surface>          — click sidebar nav button matching label (Packages/Settings/Updates/Your system)
- shot <name>             — screenshot to shots/
- wait <secs>             — sleep
- wait-for <name>         — poll until unit with name appears
- assert unit:<name> | hidden:<name> | text:<substring> | page:<id> | units == N | none-unnamed
- shell <cmd>             — run a command, print output
- shell-assert <cmd>      — fail unless command exits 0
- fail <msg>              — stop with a message
- # comment / blank       — ignored
"""

import os
import sys
import time
import subprocess
import shlex
import re
from typing import Optional, List, Dict, Any, Callable
from dataclasses import dataclass
from pathlib import Path

from .snapshot import Unit, Snapshot, parse_snapshot, load_snapshot, summarize, diff_snapshots, format_summary, format_diff
from .driver import AtspiDriver, atspi_session, dump_atspi_state, click_unit, xdotool_type, xdotool_key, screenshot
from .session import Session, isolated_session, find_catalogue_index


@dataclass
class ReplState:
    """Mutable state for the REPL/scenario runner."""
    driver: Optional[AtspiDriver] = None
    session: Optional[Session] = None
    current_snapshot: Optional[Snapshot] = None
    previous_snapshot: Optional[Snapshot] = None
    shots_dir: Path = Path("shots")
    app_binary: str = ""
    catalogue_path: Optional[str] = None
    config_dir: Optional[str] = None
    dump_file: str = "/tmp/nix-pilot-ui.json"
    last_command: str = ""
    running: bool = True


class Repl:
    """Interactive REPL and scenario runner."""

    def __init__(self, state: ReplState):
        self.state = state
        self.state.shots_dir.mkdir(parents=True, exist_ok=True)
        self._build_command_map()

    def _build_command_map(self):
        self.commands: Dict[str, Callable[[List[str]], bool]] = {
            "state": self.cmd_state,
            "units": self.cmd_units,
            "text": self.cmd_text,
            "describe": self.cmd_describe,
            "focus": self.cmd_focus,
            "page": self.cmd_page,
            "changed": self.cmd_changed,
            "summary": self.cmd_summary,
            "click": self.cmd_click,
            "type": self.cmd_type,
            "type-into": self.cmd_type_into,
            "key": self.cmd_key,
            "open": self.cmd_open,
            "shot": self.cmd_shot,
            "wait": self.cmd_wait,
            "wait-for": self.cmd_wait_for,
            "assert": self.cmd_assert,
            "shell": self.cmd_shell,
            "shell-assert": self.cmd_shell_assert,
            "fail": self.cmd_fail,
            "help": self.cmd_help,
            "quit": self.cmd_quit,
            "exit": self.cmd_quit,
            "refresh": self.cmd_refresh,
            "load": self.cmd_load,
        }

    def _ensure_snapshot(self) -> bool:
        """Ensure we have a current snapshot, loading from dump file if needed."""
        if self.state.current_snapshot is not None:
            return True
        if os.path.exists(self.state.dump_file):
            try:
                self.state.current_snapshot = load_snapshot(self.state.dump_file)
                return True
            except Exception as e:
                print(f"Error loading snapshot: {e}")
                return False
        print("No snapshot available. Use 'refresh' to capture from live session, or 'load <file>'.")
        return False

    def _refresh_snapshot(self) -> bool:
        """Capture a fresh snapshot from the live session (AT-SPI or dump file)."""
        # First try the dump file (geometry dump from app)
        if os.path.exists(self.state.dump_file):
            try:
                self.state.previous_snapshot = self.state.current_snapshot
                self.state.current_snapshot = load_snapshot(self.state.dump_file)
                return True
            except Exception as e:
                print(f"Error reading dump file: {e}")

        # Fallback to AT-SPI dump if driver is available
        if self.state.driver:
            try:
                self.state.previous_snapshot = self.state.current_snapshot
                self.state.current_snapshot = dump_atspi_state(self.state.driver)
                return True
            except Exception as e:
                print(f"Error dumping AT-SPI state: {e}")

        return False

    def _find_unit(self, arg: str) -> Optional[Unit]:
        """Find a unit by id, exact name, or label substring."""
        if not self._ensure_snapshot():
            return None
        snap = self.state.current_snapshot

        # Try as integer ID
        try:
            uid = int(arg)
            return snap.find_by_id(uid)
        except ValueError:
            pass

        def resolve(units: List[Unit]) -> Optional[Unit]:
            if len(units) == 1:
                return units[0]
            if len(units) > 1:
                # Resolve ambiguity by preferring a unit that is really on
                # screen and interactive (the nav button over a hidden
                # heading that shares its name).
                visible = [
                    u for u in units
                    if u.is_effectively_visible(snap.window_w, snap.window_h)
                ]
                interactive = [u for u in visible if u.is_interactive]
                if len(interactive) == 1:
                    return interactive[0]
                if len(visible) == 1:
                    return visible[0]
            return None

        # Try exact name match
        units = snap.find_by_name(arg, exact=True)
        hit = resolve(units) if units else None
        if hit is not None:
            return hit
        if len(units) > 1:
            print(f"Multiple units with name '{arg}': {[u.id for u in units]}")

        # Try label substring
        units = snap.find_by_label_substring(arg)
        hit = resolve(units) if units else None
        if hit is not None:
            return hit
        if len(units) > 1:
            print(f"Multiple units with label containing '{arg}': {[u.id for u in units]}")

        if not units:
            print(f"No unit found for '{arg}'")
        return None

    def _format_unit(self, u: Unit, verbose: bool = False) -> str:
        """Format a unit for display."""
        if verbose:
            return (f"  [{u.id}] {u.kind:8s} {u.role:20s} "
                    f"'{u.name}' ({u.x:4d},{u.y:4d} {u.w:3d}x{u.h:3d}) "
                    f"vis={u.visible} sens={u.sensitive} foc={u.focused}")
        else:
            label = u.label[:60] + ("..." if len(u.label) > 60 else "")
            return (f"  [{u.id}] {u.kind:8s} '{u.name}' ({u.x},{u.y},{u.w},{u.h}) "
                    f"vis={u.visible} sens={u.sensitive} foc={u.focused} | {label}")

    # --- Command implementations ---

    def cmd_help(self, args: List[str]) -> bool:
        print("""
np-drive commands:
  state [filter]           List units (optionally filter by kind/role/name substring)
  units [role]             List interactive units (button/switch/entry/row), optionally filtered by role
  text [filter]            Show all visible text (labels, button text), optionally filtered
  describe <id>            Show all fields of a unit
  focus                    Show the currently focused unit
  page                     Show the visible page
  changed                  Diff against previous snapshot (units appeared/disappeared/moved/changed)
  summary                  Show summary metrics (counts, unnamed, duplicates, hidden, offscreen, etc.)
  click <id|name|label>    Click a unit (AT-SPI then xdotool fallback)
  type <text>              Type text into focused unit
  type-into <id> <text>    Click unit then type text
  key <combo>              Send key combination (e.g., ctrl+f, Escape, Tab)
  open <surface>           Click sidebar nav: Packages, Settings, Updates, Your system
  shot <name>              Save screenshot to shots/<name>.png
  wait <secs>              Sleep for seconds
  wait-for <name>          Poll until unit with name appears (max 30s)
  assert unit:<name>       Assert a unit with this name exists and is visible
  assert hidden:<name>     Assert a unit with this name exists but is hidden
  assert text:<substring>  Assert visible text contains substring
  assert page:<id>         Assert current page equals id
  assert units == N        Assert total unit count equals N
  assert none-unnamed      Assert no unnamed interactive units
  shell <cmd>              Run shell command, print output
  shell-assert <cmd>       Run shell command, fail if non-zero exit
  fail <message>           Stop scenario with message
  refresh                  Recapture snapshot from live session/dump file
  load <file>              Load snapshot from JSON file
  help                     Show this help
  quit / exit              Exit REPL
""")
        return True

    def cmd_quit(self, args: List[str]) -> bool:
        self.state.running = False
        return True

    def cmd_refresh(self, args: List[str]) -> bool:
        if self._refresh_snapshot():
            snap = self.state.current_snapshot
            print(f"Snapshot refreshed: page={snap.page}, units={len(snap.units)}")
        else:
            print("Failed to refresh snapshot")
        return True

    def cmd_load(self, args: List[str]) -> bool:
        if not args:
            print("Usage: load <snapshot.json>")
            return True
        try:
            self.state.previous_snapshot = self.state.current_snapshot
            self.state.current_snapshot = load_snapshot(args[0])
            snap = self.state.current_snapshot
            print(f"Loaded: page={snap.page}, units={len(snap.units)}")
        except Exception as e:
            print(f"Error loading snapshot: {e}")
        return True

    def cmd_state(self, args: List[str]) -> bool:
        if not self._ensure_snapshot():
            return True
        snap = self.state.current_snapshot
        filter_str = " ".join(args).lower() if args else None

        units = snap.units
        if filter_str:
            units = [u for u in units if filter_str in u.kind.lower()
                     or filter_str in u.role.lower()
                     or filter_str in u.name.lower()
                     or filter_str in u.label.lower()]

        print(f"Window: ({snap.window_x},{snap.window_y}) {snap.window_w}x{snap.window_h}")
        print(f"Page: {snap.page or 'unknown'}")
        print(f"Units ({len(units)}):")
        for u in units:
            print(self._format_unit(u))
        return True

    def cmd_units(self, args: List[str]) -> bool:
        if not self._ensure_snapshot():
            return True
        snap = self.state.current_snapshot
        role_filter = args[0] if args else None

        units = snap.interactive_units
        if role_filter:
            units = [u for u in units if u.role == role_filter or u.kind == role_filter]

        print(f"Interactive units ({len(units)}):")
        for u in units:
            print(self._format_unit(u))
        return True

    def cmd_text(self, args: List[str]) -> bool:
        if not self._ensure_snapshot():
            return True
        snap = self.state.current_snapshot
        filter_str = " ".join(args).lower() if args else None

        texts = []
        for u in snap.visible_units:
            if u.label.strip():
                if not filter_str or filter_str in u.label.lower():
                    texts.append(f"  [{u.id}] {u.kind} '{u.name}': {u.label}")

        print(f"Visible text ({len(texts)} units):")
        for t in texts:
            print(t)
        return True

    def cmd_describe(self, args: List[str]) -> bool:
        if not args:
            print("Usage: describe <id>")
            return True
        unit = self._find_unit(args[0])
        if not unit:
            return True
        print(f"Unit [{unit.id}]:")
        print(f"  kind:       {unit.kind}")
        print(f"  role:       {unit.role}")
        print(f"  name:       '{unit.name}'")
        print(f"  label:      '{unit.label}'")
        print(f"  bounds:     ({unit.x}, {unit.y}, {unit.w}, {unit.h})")
        print(f"  center:     {unit.center}")
        print(f"  visible:    {unit.visible}")
        print(f"  sensitive:  {unit.sensitive}")
        print(f"  focused:    {unit.focused}")
        print(f"  interactive: {unit.is_interactive}")
        print(f"  unnamed:    {unit.is_unnamed}")
        return True

    def cmd_focus(self, args: List[str]) -> bool:
        if not self._ensure_snapshot():
            return True
        snap = self.state.current_snapshot
        focused = snap.focused_unit
        if focused:
            print(f"Focused unit: [{focused.id}] {focused.kind} '{focused.name}'")
        else:
            print("No focused unit")
        return True

    def cmd_page(self, args: List[str]) -> bool:
        if not self._ensure_snapshot():
            return True
        snap = self.state.current_snapshot
        print(f"Page: {snap.page or 'unknown'}")
        return True

    def cmd_changed(self, args: List[str]) -> bool:
        if not self.state.previous_snapshot or not self.state.current_snapshot:
            print("Need two snapshots to diff. Run 'refresh' after an action.")
            return True
        diff = diff_snapshots(self.state.previous_snapshot, self.state.current_snapshot)
        print(format_diff(diff))
        return True

    def cmd_summary(self, args: List[str]) -> bool:
        if not self._ensure_snapshot():
            return True
        snap = self.state.current_snapshot
        summary = summarize(snap)
        print(format_summary(summary))
        return True

    def cmd_click(self, args: List[str]) -> bool:
        if not args:
            print("Usage: click <id|name|label>")
            return True
        arg = " ".join(args)
        unit = self._find_unit(arg)
        if not unit:
            return True

        if not unit.visible:
            print(f"Unit [{unit.id}] is not visible")
            return True
        if not unit.sensitive:
            print(f"Unit [{unit.id}] is not sensitive")
            return True

        print(f"Clicking unit [{unit.id}] '{unit.name}' at {unit.center}...")
        success = False
        # xdotool must run against the live session's X display, not the REPL
        # process's environment (the AT-SPI driver is not connected from
        # outside the dbus session, so the fallback is the active path).
        display = None
        if self.state.session is not None:
            display = self.state.session.xvfb_display

        # Try AT-SPI click if driver available
        if self.state.driver:
            success = click_unit(self.state.driver, unit, use_fallback=False, display=display)
            if success:
                print("  AT-SPI click succeeded")
            else:
                print("  AT-SPI click failed, trying xdotool fallback...")
                success = click_unit(
                    self.state.driver, unit, use_fallback=True, display=display
                )
        else:
            # No driver, use xdotool directly
            success = click_unit(None, unit, use_fallback=True, display=display)

        if success:
            print("  Click succeeded")
            # Auto-refresh after action
            time.sleep(0.5)
            self._refresh_snapshot()
        else:
            print("  Click failed")
        return True

    def cmd_type(self, args: List[str]) -> bool:
        if not args:
            print("Usage: type <text>")
            return True
        text = " ".join(args)
        if self.state.driver:
            # Type via xdotool (AT-SPI set_text is unreliable for non-focused)
            success = xdotool_type(text, self.state.driver.xvfb_display)
            if success:
                print(f"Typed: {text}")
            else:
                print("Type failed")
        else:
            print("No driver session for typing")
        return True

    def cmd_type_into(self, args: List[str]) -> bool:
        if len(args) < 2:
            print("Usage: type-into <id|name|label> <text>")
            return True
        target = args[0]
        text = " ".join(args[1:])
        unit = self._find_unit(target)
        if not unit:
            return True
        # Click first
        self.cmd_click([target])
        time.sleep(0.2)
        # Then type
        self.cmd_type([text])
        return True

    def cmd_key(self, args: List[str]) -> bool:
        if not args:
            print("Usage: key <combo> (e.g., ctrl+f, Escape, Tab)")
            return True
        combo = " ".join(args)
        if self.state.driver:
            success = xdotool_key(combo, self.state.driver.xvfb_display)
            if success:
                print(f"Sent key: {combo}")
            else:
                print("Key send failed")
        else:
            print("No driver session for keys")
        return True

    def cmd_open(self, args: List[str]) -> bool:
        if not args:
            print("Usage: open <surface> (Packages, Settings, Updates, Your system)")
            return True
        surface = " ".join(args)
        # Map surface names to sidebar button labels
        label_map = {
            "packages": "Packages",
            # Settings is a two-column router: the sidebar shows only the
            # category items, so opening it clicks the first category.
            "settings": "System",
            "system": "System",
            "software": "Software",
            "network": "Network & internet",
            "accounts": "Accounts",
            "time": "Time & language",
            "personalisation": "Personalisation",
            "privacy": "Privacy & security",
            "updates": "Updates",
            "your system": "Your system",
            "your-system": "Your system",
        }
        label = label_map.get(surface.lower(), surface)
        print(f"Opening '{label}'...")
        self.cmd_click([label])
        return True

    def cmd_shot(self, args: List[str]) -> bool:
        if not args:
            print("Usage: shot <name>")
            return True
        name = args[0]
        path = self.state.shots_dir / f"{name}.png"
        display = self.state.driver.xvfb_display if self.state.driver else os.environ.get("DISPLAY", ":0")
        if screenshot(str(path), display):
            print(f"Screenshot saved to {path}")
        else:
            print("Screenshot failed (need ImageMagick 'import')")
        return True

    def cmd_wait(self, args: List[str]) -> bool:
        if not args:
            print("Usage: wait <seconds>")
            return True
        try:
            secs = float(args[0])
            print(f"Waiting {secs}s...")
            time.sleep(secs)
        except ValueError:
            print("Invalid number of seconds")
        return True

    def cmd_wait_for(self, args: List[str]) -> bool:
        if not args:
            print("Usage: wait-for <name>")
            return True
        name = " ".join(args)
        print(f"Waiting for unit '{name}' to appear (max 30s)...")
        start = time.time()
        while time.time() - start < 30:
            self._refresh_snapshot()
            if self.state.current_snapshot:
                units = self.state.current_snapshot.find_by_name(name, exact=False)
                if units:
                    print(f"Found: [{units[0].id}] {units[0].kind} '{units[0].name}'")
                    return True
            time.sleep(0.5)
        print("Timeout: unit did not appear")
        return False  # Fail the scenario

    def cmd_assert(self, args: List[str]) -> bool:
        if not args:
            print("Usage: assert unit:<name> | hidden:<name> | text:<substring> | page:<id> | units == N | none-unnamed")
            return True
        assertion = " ".join(args)
        if not self._ensure_snapshot():
            print("ASSERT FAIL: No snapshot")
            return False
        snap = self.state.current_snapshot

        # Parse assertion
        if assertion.startswith("unit:"):
            name = assertion[5:].strip()
            units = snap.find_by_name(name, exact=True)
            visible_units = [u for u in units if u.visible]
            if not visible_units:
                print(f"ASSERT FAIL: No visible unit named '{name}'")
                return False
            print(f"ASSERT OK: unit '{name}' visible (ids: {[u.id for u in visible_units]})")
            return True

        elif assertion.startswith("hidden:"):
            name = assertion[7:].strip()
            units = snap.find_by_name(name, exact=True)
            hidden_units = [u for u in units if not u.visible]
            if not hidden_units:
                print(f"ASSERT FAIL: No hidden unit named '{name}'")
                return False
            print(f"ASSERT OK: unit '{name}' hidden (ids: {[u.id for u in hidden_units]})")
            return True

        elif assertion.startswith("text:"):
            substr = assertion[5:].strip()
            found = False
            for u in snap.visible_units:
                if substr.lower() in u.label.lower():
                    found = True
                    print(f"ASSERT OK: text '{substr}' found in unit [{u.id}] '{u.name}'")
                    break
            if not found:
                print(f"ASSERT FAIL: Text '{substr}' not found in any visible unit")
                return False
            return True

        elif assertion.startswith("page:"):
            expected = assertion[5:].strip()
            if snap.page == expected:
                print(f"ASSERT OK: page == '{expected}'")
                return True
            else:
                print(f"ASSERT FAIL: page is '{snap.page}', expected '{expected}'")
                return False

        elif assertion.startswith("units =="):
            try:
                expected = int(assertion[8:].strip())
                actual = len(snap.units)
                if actual == expected:
                    print(f"ASSERT OK: units == {expected}")
                    return True
                else:
                    print(f"ASSERT FAIL: units == {actual}, expected {expected}")
                    return False
            except ValueError:
                print("ASSERT FAIL: Invalid units count")
                return False

        elif assertion == "none-unnamed":
            unnamed = [
                u for u in snap.units
                if u.is_unnamed and u.is_effectively_visible(snap.window_w, snap.window_h)
                and u.sensitive
                and not u.role.startswith("__Unknown")
                and not u.is_window_chrome(snap.window_w, snap.window_h)
            ]
            if not unnamed:
                print("ASSERT OK: no unnamed interactive units")
                return True
            else:
                print(f"ASSERT FAIL: {len(unnamed)} unnamed interactive units: {[u.id for u in unnamed]}")
                return False

        else:
            print(f"Unknown assertion: {assertion}")
            return False

    def cmd_shell(self, args: List[str]) -> bool:
        if not args:
            print("Usage: shell <command>")
            return True
        cmd = " ".join(args)
        print(f"$ {cmd}")
        try:
            result = subprocess.run(cmd, shell=True, capture_output=True, text=True, timeout=30)
            if result.stdout:
                print(result.stdout.rstrip())
            if result.stderr:
                print(result.stderr.rstrip(), file=sys.stderr)
            print(f"[exit {result.returncode}]")
        except subprocess.TimeoutExpired:
            print("Command timed out")
        except Exception as e:
            print(f"Error: {e}")
        return True

    def cmd_shell_assert(self, args: List[str]) -> bool:
        if not args:
            print("Usage: shell-assert <command>")
            return True
        cmd = " ".join(args)
        print(f"$ {cmd}")
        try:
            result = subprocess.run(cmd, shell=True, capture_output=True, text=True, timeout=30)
            if result.returncode == 0:
                print("ASSERT OK: command exited 0")
                if result.stdout:
                    print(result.stdout.rstrip())
                return True
            else:
                print(f"ASSERT FAIL: command exited {result.returncode}")
                if result.stdout:
                    print(result.stdout.rstrip())
                if result.stderr:
                    print(result.stderr.rstrip(), file=sys.stderr)
                return False
        except subprocess.TimeoutExpired:
            print("ASSERT FAIL: command timed out")
            return False
        except Exception as e:
            print(f"ASSERT FAIL: {e}")
            return False

    def cmd_fail(self, args: List[str]) -> bool:
        msg = " ".join(args) if args else "Failed by command"
        print(f"FAIL: {msg}")
        return False

    # --- REPL loop ---

    def run_line(self, line: str) -> bool:
        """Execute a single command line. Returns False to stop."""
        line = line.strip()
        if not line or line.startswith("#"):
            return True

        parts = shlex.split(line)
        if not parts:
            return True

        cmd = parts[0].lower()
        args = parts[1:]

        self.state.last_command = line

        handler = self.commands.get(cmd)
        if not handler:
            print(f"Unknown command: {cmd} (type 'help' for list)")
            return True

        try:
            return handler(args)
        except Exception as e:
            print(f"Error executing '{cmd}': {e}")
            return True

    def run_interactive(self) -> int:
        """Run the interactive REPL loop."""
        print("np-drive REPL. Type 'help' for commands, 'quit' to exit.")
        if self.state.driver:
            print(f"Connected to app via AT-SPI on display {self.state.driver.xvfb_display}")
        else:
            print("No live session; using dump file only.")
        print()

        self._refresh_snapshot()

        while self.state.running:
            try:
                line = input("np-drive> ")
            except (EOFError, KeyboardInterrupt):
                print()
                break
            if not self.run_line(line):
                return 1
        return 0

    def run_scenario(self, path: str) -> int:
        """Run a scenario file line by line."""
        print(f"Running scenario: {path}")
        self._refresh_snapshot()

        with open(path, "r") as f:
            lines = f.readlines()

        for i, line in enumerate(lines, 1):
            line = line.rstrip("\n")
            print(f"{i:4d}> {line}")
            if not self.run_line(line):
                print(f"Scenario stopped at line {i} due to failure")
                return 1
        print("Scenario completed successfully")
        return 0

    def close(self) -> None:
        """Tear down the live session, if any."""
        if self.state.session is not None:
            self.state.session.close()
            self.state.session = None


def create_repl(
    app_binary: str = "",
    catalogue_path: Optional[str] = None,
    config_dir: Optional[str] = None,
    dump_file: str = "/tmp/nix-pilot-ui.json",
    use_session: bool = False,
    skip_setup: bool = False,
) -> Repl:
    """Factory to create a Repl with optional live session."""
    state = ReplState(
        app_binary=app_binary,
        catalogue_path=catalogue_path,
        config_dir=config_dir,
        dump_file=dump_file,
    )

    if use_session and app_binary:
        # Start isolated session
        session = Session(
            app_binary=app_binary,
            catalogue_path=catalogue_path,
            config_dir=config_dir,
            dump_file=dump_file,
            skip_setup=skip_setup,
        )
        session.start()
        time.sleep(2)  # Let session stabilize
        state.session = session
        # The dump file was cleared at session start; wait for the app's
        # first snapshot so the REPL never reads stale state.
        if not session.wait_for_snapshot():
            print("warning: no UI snapshot appeared from the app")

    repl = Repl(state)
    if use_session and app_binary:
        repl._refresh_snapshot()
    return repl


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description="np-drive REPL")
    parser.add_argument("scenario", nargs="?", help="Scenario file to run")
    parser.add_argument("--app", help="Path to nix-pilot-app binary")
    parser.add_argument("--catalogue", help="Path to catalogue-index.json")
    parser.add_argument("--config-dir", help="Path to config directory")
    parser.add_argument("--dump-file", default="/tmp/nix-pilot-ui.json", help="UI dump file path")
    parser.add_argument("--self-test", action="store_true", help="Run unit tests")
    args = parser.parse_args()

    if args.self_test:
        # Run unit tests
        import subprocess
        result = subprocess.run([sys.executable, "-m", "pytest", "tests/", "-v"])
        sys.exit(result.returncode)

    repl = create_repl(
        app_binary=args.app or "",
        catalogue_path=args.catalogue,
        config_dir=args.config_dir,
        dump_file=args.dump_file,
        use_session=bool(args.app),
    )

    if args.scenario:
        sys.exit(repl.run_scenario(args.scenario))
    else:
        sys.exit(repl.run_interactive())