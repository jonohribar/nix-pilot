# Coding Style

Brief, high-level style guide for the product code in `app/`. Tenets: readability first, then maintainability, testability, scalability (AGENTS.md). The client is Rust + GTK4 + libadwaita (decision D19).

## Module boundaries

Code follows the architecture boundaries rather than inventing a second structure:

- `src/model.rs` — catalogue domain: typed `Class`/`Tier` enums, `IndexEntry`, pre-lowered search text. No GTK, no I/O.
- `src/search.rs` — ranking (ported 1:1 from the verified `experiments/catalogue/search.js` scorer). Pure.
- `src/store.rs` — application actions: the desired-change queue and the check-result rule. Pure.
- `src/install.rs` — the nix adapter: discovers the client, runs the dry-run check, returns a typed `Outcome`. All external I/O lives here.
- `src/updates.rs` — the updates backend: the transaction state machine (`TxStep`, failure-and-recovery semantics, driven by the Updates page for its safe steps) plus safe build-verify of queued changes (dry-run). The apply/rollback edges are spec-tested and gated until the desired-state apply backend exists.
- `src/apply.rs` — the apply adapter (7wo.2 + activation surface 7wo.3/D24–D26): writes the generated configuration (D21 shape: imports the live config by pointer + the desired-state module) into an explicit generation dir, *builds* the candidate with `nixos-rebuild build` — never switches or activates unless `can_activate()` (module installed + `NIX_PILOT_ALLOW_SWITCH`), and even then only via `pkexec nix-pilot-apply` with an in-band tar of the exact rendered area. Storage upkeep (7wo.9) adds the fixed `gc`/`optimise`/`gc-estimate` cleanup actions and the read-only generations listing (7wo.8, world-readable profile symlinks). All external I/O; live `--ignored` tests prove a scratch build on a NixOS host.
- `src/desired_state.rs` — the app-owned desired-state model (7wo.2): the persisted source of truth (versioned JSON, per-account file, atomic writes; packages + user-set settings + per-account authorized keys + app-owned accounts) plus the generated-NixOS-module renderer (pure, deterministic, live parse-checked). On launch the shell loads it back into the change queue; every queue change rewrites the running account's user scope and the system scope, preserving other sections.
- `src/catalogue.rs` — catalogue consumption (7wo.4): source resolution (CLI arg > `NIX_PILOT_CATALOGUE` > conventional local paths), remote fetch via curl into the app cache, and `.sha256` sidecar verification (mismatch is a hard error).
- `src/config_value.rs` — read-only live-config reader for the Settings surface (evaluate one NixOS option with `nix-instantiate`, typed `ConfigValue`; `NoSuchOption` classification for "no longer applies").
- `src/settings.rs` — the Settings surface (9f3.8/D14): the owned setting workflows as pure data (label, plain-language blurb, value validation) and the render of user-set values as app-wins `lib.mkOverride 10` lines into the desired module. No GTK, no Nix.
- `src/ui/` — the GTK surface: `shell.rs` (sidebar/navigation + store list/search, the composition point), `detail.rs` (detail page + install-intent state machine), `updates_page.rs` (Updates page: transaction prepare + candidate build + Automatic/Storage/System-versions upkeep cards), `settings_page.rs` (Settings surface: REQ-5 areas, D14 pointers), `your_system.rs` (read-only recipe view with remove actions), `widgets.rs` (shared row builders).
- `src/main.rs` — thin startup: initialise, load the index, resolve nix, call `ui::run`. Keep it small; new behaviour goes in a module, not here.

The dependency rule: UI calls application actions; domain and action code do not depend on GTK; adapters own external I/O. That keeps behaviour testable without opening a window or invoking nix.

## Types over strings

Repeated string-valued categories are enums, not raw strings: package class and evidence tier are `Class`/`Tier` with labels on the type. Adapter results are typed: `install::Outcome` distinguishes an ok result, an unfree eval block, an unavailable attr, and a check failure, so the UI maps outcome to wording instead of a bool plus a message.

## GTK and threads

GTK widgets are not `Send`; only plain data crosses threads (std `mpsc` + a main-loop `timeout_add_local` poller). A finished check carries the requested attribute so a stale result never updates the card of a different selection.

Never `match` on `cell.borrow()` when any arm might mutate the same cell: the borrow's temporary lives to the end of the `match`, so the arm's `borrow_mut()` panics (`RefCell already borrowed` — the Install-click crash, 2026-11-20). Snapshot the value first (`let state = *cell.borrow();`) when the type is `Copy`, or re-borrow inside the arms otherwise.

## Tests and the gate

Pure logic carries unit tests next to the code. Live nix behaviour is covered by `#[ignore]` integration tests run explicitly. The repeatable gate is

```sh
just app check     # cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
just app smoke     # headless window-launch smoke test
```

Run `just app check` before committing; keep it green.
