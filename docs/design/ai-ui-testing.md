# AI-driven integration testing — motivating problems and driver design

Purpose: capture the failures that made hand-over system testing unreliable,
and design an interactive UI driver the AI can run itself — a REPL that runs
the app, reports every visible and clickable UI unit, and executes the manual
test steps turn by turn.

## The motivating problems (2026-11-27 D33 system test)

These are the concrete failures from the first live run. Each is an example
of the class of bug the tester must catch before a human ever sees the app.

**Problem 1 — the app targeted the wrong configuration silently.**
The app detected `/etc/nixos` (a stale classic copy) instead of the real flake
at `~/Projects/nixos-config`, because discovery checked `/etc/nixos` first.
Every downstream step was then wrong: the snippet was the classic variant,
`Open my config` opened the wrong folder, and verify parsed the wrong file.
Result: the user pasted the classic snippet into the wrong config and got a
duplicate-attribute parse error. The manual described the flake flow, so the
instructions and the app disagreed, and the failure looked like broken
instructions.

The detection logic had no test that simulated the machine layout
(flake in home + stale `/etc/nixos`). A driver-based test would have caught it
at the first launch: read the setup page, read the snippet, click open, and
check the target path.

Fix (landed): discovery now scans home flake directories first, and a flake
anywhere beats a classic `/etc/nixos`. Unit tests pin the ordering.

**Problem 2 — the first-run setup page is disconnected from the rest of the
app.**
The setup page is a stack page inside the normal shell, so the sidebar stays
visible with `Packages` highlighted and the page reads like a package, not a
workflow. There is no obvious way to reach it again, no statement of what
state it needs, and no clear exit. The user asked: what state do I need to be
in to see this page, how do I get here, how do I return?

Design questions to settle (open): reachability (a Setup item in a menu, a
Settings card button, a deep link), the entry trigger (detect the need and
offer a guided popup that leads into the wizard), and the state model
(not-integrated vs integrated vs disconnected shown consistently across the
shell, sidebar, and headers).

**Problem 3 — the first-run step makes the user write code.**
The snippet is a paste-into-your-config action with no alternative. There is
no clean slate, so a user who cannot or will not edit a flake has no path.
The planned one-click edit (4tk.1) and fresh installer (4tk.3) address this,
but the manual-first experience needs at least a plain explanation and a
copyable, validated block.

**Problem 4 — instructions and app behavior are not coupled.**
The manual is prose the user follows by hand; the app behaves differently and
nothing checks the two agree. The tester changes this: the manual becomes a
script the driver runs, and the driver asserts the app state after each step.

## What we already have

- `NIX_PILOT_DEBUG_GEOM` geometry dump (`geom_dump.rs`): JSON of every
  button/switch/row/list/entry with label, position, size, visibility — the
  "log the locations of all the buttons" idea, already built.
- AT-SPI test harness (`app/scripts/a11y/`, `test-atspi.nix`): Python driver
  over pyatspi with click/find helpers; flaky under plain Xvfb because a
  bare Xvfb session has no working accessibility bus.
- Screenshot capture scripts and `just` recipes for build/smoke/module-check.

## What the field offers (research, 2026-11-27)

For Linux/GTK specifically, the useful landscape:

- **AT-SPI accessibility tree** is the semantic backbone. Tools that read it
  give roles, names, bounds, and states without OCR.
- **computer-use-linux** (agent-sh): AT-SPI semantic trees, bounded
  screenshots, compositor-aware window targeting, X11/Wayland input
  synthesis, post-action verification. Broad desktop support including KDE.
- **Spire**: a thin AT-SPI tool that emits compact numbered snapshots an LLM
  can reason about and drives actions via AT-SPI plus xdotool — the closest
  match to the requested REPL.
- **DesktopKit**: a Playwright-like API for GTK/GNOME apps
  (`get_by_role("button", name=...).click()`) plus an MCP server, so an AI
  assistant can drive the app directly.
- **pinchy**: starts an isolated, invisible GNOME session (own D-Bus,
  accessibility bus, headless mutter) so automation never fights the real
  session — the proper fix for our Xvfb AT-SPI hang.
- **Agent S3/S4** (Simular), **UI-TARS** (ByteDance, open weights, desktop
  variant), **Midscene**, **OpenAdapt** (records human actions, replays via
  AT-SPI on Linux), **Pantomime** (natural-language test cases).
- Benchmarks/literature: OSWorld, GUI-360°, OS-ATLAS; "GUI Agents: A Survey".

## Proposed design — the driver REPL

A small CLI (`np-drive`) plus a session wrapper, in this repository under
`app/scripts/drive/`. The AI (or a human) runs the manual test script turn by
turn; after every action the driver prints the current UI state.

Session layout (fixes the a11y hang):
- One isolated session per run: `dbus-run-session` + a dedicated X display
  (or pinchy-style headless session if the sandbox needs it), the
  at-spi-bus-launcher inside it, and the app launched with the test env
  (`NIX_PILOT_SKIP_SETUP` not set, catalogue index, geometry dump on).

Driver commands:
- `state` — print numbered UI units: role, name, bounds, visible, clickable,
  window, from the AT-SPI tree (fallback to the geometry-dump JSON when the
  a11y tree is unavailable).
- `click <number|label>` — AT-SPI action or xdotool click at the unit bounds.
- `type <text>` / `key <combo>` — keyboard input to the focused unit.
- `shot <name>` — save a screenshot for visual verification.
- `assert <condition>` — assert a unit exists, is visible, has text, etc.
- `wait <seconds>` / `wait-for <condition>` — settle steps.

Manual-to-script mapping:
- Each manual step becomes `# comment` + driver commands; the driver stops on
  a failed assert. The system-test manual is split into `steps/*.drv` files
  or a single annotated file the driver parses.
- The driver runs headless, so `just test-run` does the whole loop without a
  human and without touching the host (no real switch — the integration
  target is a scratch flake in a temp dir, or the VM harness where a real
  switch is wanted).

Build order:
1. Driver core: session wrapper + `state`/`click`/`type`/`shot` backend
   (AT-SPI first; geometry dump as fallback). Prove it drives the existing
   nav/search flows.
2. Step runner: parse the annotated manual, run it, stop on assert failure.
3. First real scenario: the first-run setup flow against a scratch flake
   fixture (detects the flake, writes the app files, shows the snippet,
   verifies parse) — this is exactly what caught Problems 1-3.
4. Extend to the apply flow against the VM harness.

Adopt off-the-shelf (computer-use-linux, DesktopKit, Spire, or pinchy) only
if the homegrown driver proves limiting; the homegrown core is small and uses
what we already ship.

## Confirmed session recipe (2026-11-27, sandbox-verified)

The app appears in the AT-SPI tree when `at-spi-bus-launcher` runs INSIDE
`dbus-run-session`. The earlier 300-600s "hang" was a teardown timeout, not a
failure to find the app.

```bash
cd app
xvfb-run --auto-servernum dbus-run-session -- at-spi-bus-launcher --launch-immediately &
./target/debug/nix-pilot-app &
# then: from gi.repository import Atspi; Atspi.get_desktop(0) sees frame 'nix-pilot'
```

Environment: `GDK_BACKEND=x11`; `AT_SPI_BUS_ADDRESS` must NOT be set (the
app discovers the bus through the session); unique `XDG_RUNTIME_DIR`,
`XDG_DATA_HOME`, `XDG_CONFIG_HOME` per test. No window manager required.

## Snapshot schema (the integration seam)

The app emits one canonical description of every user-visible unit; the
`np-drive` driver consumes it. App side: `geom_dump.rs` becomes the UI
snapshot emitter (env `NIX_PILOT_UI_DUMP`, file `NIX_PILOT_UI_DUMP_FILE`,
periodic + on-change writes). Driver side: AT-SPI for live semantics, the
snapshot file for geometry, screenshots for glyph-level checks.

```json
{
  "window": { "x": 0, "y": 0, "w": 1180, "h": 800 },
  "page": "list",
  "units": [
    {
      "id": 0,
      "kind": "button",
      "role": "button",
      "name": "Apply changes",
      "label": "Apply changes | tip:...",
      "x": 12, "y": 300, "w": 100, "h": 30,
      "visible": true,
      "sensitive": true,
      "focused": false
    }
  ]
}
```

- `id`: index in the dump (stable per dump; driver addresses by it).
- `kind`: widget class (button/switch/row/list/scroll/entry/searchentry).
- `role`: GTK accessible role.
- `name`: the accessible name, else the first label fragment — the unit's
  clickable identity.
- `label`: full concatenated text (labels, button text, tooltips).
- `page`: the visible stack child (list/detail/settings/updates/...).

## REPL command surface

Core observation:
- `state [filter]` — numbered units with id/kind/role/name/bounds/visible/
  sensitive/focused.
- `units [role]` — shorthand for all interactive units ("all clickable
  elements").
- `text [filter]` — all text in the window (labels + button text).
- `describe <id>` — every field of one unit.
- `focus` — the focused unit.
- `page` — the visible stack page.
- `changed` — diff against the previous snapshot: units that appeared,
  disappeared, moved, or changed state after the last action (bug-finding
  breadcrumbs: click X, see what appeared).

Readability / cleanliness audit:
- `summary` — counts by role; unnamed interactive units (accessibility gap);
  duplicate names (click ambiguity); hidden or offscreen units; the focused
  unit; the visible page; total visible text length as a clutter proxy.

Actions:
- `click <id|name|label>` — AT-SPI action, else xdotool at the unit bounds.
- `type <text>` / `type-into <id> <text>` — keyboard input.
- `key <combo>` — key chords (for example `ctrl+f`).
- `open <surface>` — click the sidebar nav item for a named surface.
- `shot <name>` — screenshot to `shots/`.
- `wait <seconds>` / `wait-for <name>`.

Assertions (the step runner stops on failure):
- `assert unit:<name>` / `hidden:<name>` / `text:<substring>` /
  `page:<id>` / `units == N` / `none-unnamed`.
- `shell <cmd>` — run a command, print output (integration checks).
- `shell-assert <cmd>` — fail unless the command exits 0.
- `fail <message>` — stop with a message.
- `# comment` and blank lines are ignored.

The `summary` command is the direct answer to "how cluttered or clean or
state-readable is this app state": it quantifies unnamed units, ambiguous
names, hidden/offscreen widgets, and visible text volume per state.
