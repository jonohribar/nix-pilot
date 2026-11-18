# Coding Style

Brief, high-level style guide for the product code in `app/`. Tenets: readability first, then maintainability, testability, scalability (AGENTS.md). The client is Rust + GTK4 + libadwaita (decision D19).

## Module boundaries

Code follows the architecture boundaries rather than inventing a second structure:

- `src/model.rs` — catalogue domain: typed `Class`/`Tier` enums, `IndexEntry`, pre-lowered search text. No GTK, no I/O.
- `src/search.rs` — ranking (ported 1:1 from the verified `experiments/catalogue/search.js` scorer). Pure.
- `src/store.rs` — application actions: the desired-change queue and the check-result rule. Pure.
- `src/install.rs` — the nix adapter: discovers the client, runs the dry-run check, returns a typed `Outcome`. All external I/O lives here.
- `src/updates.rs` — the updates backend: safe build-verify of queued changes (dry-run); the apply/rollback path is gated until the desired-state layer exists.
- `src/desired_state.rs` — the app-owned desired-state model (7wo.2): holds the package set by scope and renders the generated NixOS module (pure, deterministic, live parse-checked). The build/apply step that consumes it stays gated.
- `src/ui/` — the GTK surface: `shell.rs` (sidebar/navigation + store list/search, the composition point), `detail.rs` (detail page + install-intent state machine), `updates_page.rs` (updates review + safe prepare), `widgets.rs` (shared row builders).
- `src/main.rs` — thin startup: initialise, load the index, resolve nix, call `ui::run`. Keep it small; new behaviour goes in a module, not here.

The dependency rule: UI calls application actions; domain and action code do not depend on GTK; adapters own external I/O. That keeps behaviour testable without opening a window or invoking nix.

## Types over strings

Repeated string-valued categories are enums, not raw strings: package class and evidence tier are `Class`/`Tier` with labels on the type. Adapter results are typed: `install::Outcome` distinguishes an ok result, an unfree eval block, an unavailable attr, and a check failure, so the UI maps outcome to wording instead of a bool plus a message.

## GTK and threads

GTK widgets are not `Send`; only plain data crosses threads (std `mpsc` + a main-loop `timeout_add_local` poller). A finished check carries the requested attribute so a stale result never updates the card of a different selection.

## Tests and the gate

Pure logic carries unit tests next to the code. Live nix behaviour is covered by `#[ignore]` integration tests run explicitly. The repeatable gate is

```sh
just app check     # cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
just app smoke     # headless window-launch smoke test
```

Run `just app check` before committing; keep it green.
