# nix-pilot AT-SPI Integration Tests

This directory contains a headless accessibility-driven test harness for the
nix-pilot GTK4 application. Tests drive the real application binary through
its AT-SPI accessibility tree (roles, names, states, actions) rather than
pixels, enabling fast, reliable CI verification of UI behaviour.

## Components

- `driver.py` — Core driver: launches Xvfb + at-spi-bus-launcher + the app,
  waits for the AT-SPI registry, exposes helpers to find/click/read
  accessibles by role + accessible name.
- `run-test.sh` — Runner: boots Xvfb on a fresh display and wraps the whole
  test (Python a11y client + launcher + app) in ONE `dbus-run-session`.
- `test_nav_settings.py` — Test: click Settings sidebar nav → Settings home shows.
- `test_search.py` — Test: type in search entry → results change.

## Requirements

The test environment is provided by `app/test-atspi.nix` (see below). It includes:

- Xvfb (virtual display)
- dbus (session bus)
- at-spi2-core (accessibility registry)
- python3 + pygobject3 (GTK/Python bindings + AT-SPI)
- dogtail (optional, GSettings check disabled)
- The nix-pilot build dependencies (gtk4, libadwaita, cargo, rustc, etc.)

## Running the Tests

### 1. Build the app

```bash
cd app
nix-shell shell.nix --run 'cargo build'
# Binary at ./target/debug/nix-pilot-app
```

### 2. Enter the AT-SPI test shell

```bash
nix-shell test-atspi.nix
```

This drops you into a shell with all test dependencies and the app binary
on PATH (if you built it in step 1, or it will build on first run).

### 3. Run a test

```bash
python3 scripts/a11y/test_nav_settings.py ./target/debug/nix-pilot-app
python3 scripts/a11y/test_search.py ./target/debug/nix-pilot-app
```

Or run both via the justfile recipe (see below):

```bash
just app atspi-test
```

## Adding a New Test

1. Create a new test file in `scripts/a11y/` (e.g., `test_my_feature.py`).
2. Use the `atspi_session` context manager from `driver.py`:

```python
from driver import atspi_session, find_by_role, find_by_name

def test_my_feature(app_binary: str) -> bool:
    with atspi_session(app_binary) as driver:
        time.sleep(2)  # let UI settle
        # Use driver.find_by_role_and_name, driver.click, driver.get_text, etc.
        ...
        return True
```

3. Follow the pattern: assert on `role_name` and `name` (accessible name),
   not on visual properties. Use `driver.dump_tree()` for debugging.

## How It Works

```
Xvfb (:99)          dbus-daemon (session)          at-spi-bus-launcher
    │                     │                              │
    └─────────────────────┼──────────────────────────────┘
                          ▼
                   nix-pilot-app (GTK4)
                          │
              GTK_MODULES=gail:atk-bridge
                          ▼
                   AT-SPI Registry (on session bus)
                          │
              Python test (PyATSPI) ◄─── queries/acts on tree
```

The runner (`run-test.sh`):
1. Starts Xvfb on a fresh display (`:100+`, pid-derived) so stale locks never
   block a re-run.
2. Runs the whole test inside ONE `dbus-run-session`.

The driver (inside that one session):
1. Starts `at-spi-bus-launcher` (registers the AT-SPI registry on the bus)
2. Launches the app (GTK4 connects to the already-live `org.a11y.Bus`)
3. Calls `Atspi.init()` and waits for `nix-pilot-app` to appear in the tree
4. Exposes high-level helpers (`find_by_role_and_name`, `click`, `get_text`, etc.)

**The critical lesson (2026-11-20):** the Python `Atspi` client, the
`at-spi-bus-launcher`, and the app MUST all share the same session bus. If
the driver runs `dbus-run-session` in a child of its own process, the app
registers on that child's bus while `Atspi.get_desktop()` reads the parent's
bus — the app never appears and every test fails with “did not appear in
AT-SPI tree”. Also strip inherited `DBUS_SESSION_BUS_ADDRESS`/
`AT_SPI_BUS_ADDRESS` before starting children, or the harness may dial a
stale/host session instead of the isolated one.

## Debugging

- Set `A11Y_DEBUG=1` in the environment for verbose AT-SPI logs.
- Use `driver.dump_tree()` to print the full accessibility tree.
- The test shell includes `xwininfo`, `xwd`, `imagemagick` for screenshots if needed.

## CI Integration

The `just app atspi-test` recipe runs both tests and exits with non-zero on
failure. It can be added to a CI pipeline that has Nix available.

## Notes

- Dogtail is included but its GSettings accessibility check is disabled
  (`config.checkForA11y = False`) because the test environment doesn't have
  the full GNOME schema setup. We use PyATSPI directly instead.
- The app's `application_id` is `dev.nixpilot.app` (set in `main.rs`).
- Tests isolate `XDG_DATA_HOME` to a temp dir so headless runs don't pollute
  the user's real config.
- `run-test.sh` points `NIX_PILOT_CATALOGUE` at a real catalogue index when
  present (dev worktrees may not carry the gitignored index; the main
  checkout's `experiments/catalogue/results/catalogue-index.json` is the
  fallback). Without a catalogue the store list is empty and
  catalogue-dependent tests fail.