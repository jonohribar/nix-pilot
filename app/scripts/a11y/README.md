# nix-pilot AT-SPI Integration Tests

This directory contains a headless accessibility-driven test harness for the
nix-pilot GTK4 application. Tests drive the real application binary through
its AT-SPI accessibility tree (roles, names, states, actions) rather than
pixels, enabling fast, reliable CI verification of UI behaviour.

## Components

- `driver.py` — Core driver: launches Xvfb + at-spi-bus-launcher + the app,
  waits for the AT-SPI registry, exposes helpers to find/click/read
  accessibles by role + accessible name.
- `run-test.sh` — Runner: starts one `dbus-run-session` for the Python client,
  accessibility launcher, and app; the driver starts Xvfb on the selected
  display.
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

This drops you into a shell with the test dependencies. Build the app in step
1 first if `./target/debug/nix-pilot-app` does not already exist.

### 3. Run a test

Run an individual test through the runner so it gets an isolated session bus
and display:

```bash
./scripts/a11y/run-test.sh \
  ./scripts/a11y/test_nav_settings.py ./target/debug/nix-pilot-app
./scripts/a11y/run-test.sh \
  ./scripts/a11y/test_search.py ./target/debug/nix-pilot-app
```

Or run both via the justfile recipe (see below):

```bash
just app atspi-test
```

## Adding a New Test

1. Create a new test file in `scripts/a11y/` (e.g., `test_my_feature.py`).
2. Use the `atspi_session` context manager from `driver.py`:

```python
import time

from driver import atspi_session

def test_my_feature(app_binary: str) -> bool:
    with atspi_session(app_binary) as driver:
        time.sleep(2)  # let UI settle
        settings = driver.find_by_role_and_name(driver.app, "button", "Settings")
        assert settings is not None
        return True
```

3. Follow the pattern: assert on `role_name` and `name` (accessible name),
   not on visual properties. Use `driver.dump_tree()` for debugging.

## How It Works

```
run-test.sh
└── dbus-run-session (shared session bus)
    └── Python test / driver
        ├── Xvfb (test display)
        ├── at-spi-bus-launcher → AT-SPI bus and registry
        └── nix-pilot-app (GTK4; connects to the AT-SPI bus)
```

The runner (`run-test.sh`):
1. Selects a display number and runs the test inside ONE `dbus-run-session`.
2. Uses the repository catalogue index when `NIX_PILOT_CATALOGUE` is unset.

The driver (inside that one session):
1. Starts Xvfb on the selected display and `at-spi-bus-launcher`.
2. Launches the app with the same `dbus-run-session` bus.
3. Calls `Atspi.init()` and waits for `nix-pilot-app` to appear in the tree.
4. Exposes helpers (`find_by_role_and_name`, `click`, `get_text`, etc.).

**The critical lesson (2026-11-20):** the Python `Atspi` client, the
`at-spi-bus-launcher`, and the app MUST all share the same session bus. If
the test processes use different buses, the app registers somewhere that
`Atspi.get_desktop()` does not read and never appears in the tree. The runner's
`dbus-run-session` provides the shared isolated bus: the driver preserves its
`DBUS_SESSION_BUS_ADDRESS` and clears only a potentially stale
`AT_SPI_BUS_ADDRESS`. It selects the `dbus-daemon` AT-SPI backend because the
isolated test session does not provide a systemd user unit manager.

## Debugging

- Check the runner output for D-Bus, AT-SPI registry, and application startup
  diagnostics.
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
- Each test gets private temporary `XDG_DATA_HOME`, `XDG_CONFIG_HOME`, and
  `XDG_RUNTIME_DIR` directories; the driver removes them during cleanup.
- `run-test.sh` uses `NIX_PILOT_CATALOGUE` when set, otherwise it looks for
  `experiments/catalogue/results/catalogue-index.json` in this repository.
  If no index is available it prints a warning; catalogue-dependent tests
  need a real index to populate the store list.
