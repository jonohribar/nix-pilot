# Coding Style

Brief, high-level style guide for the product code in `app/`. Tenets: readability first, then maintainability, testability, scalability (AGENTS.md). The client is Rust + GTK4 + libadwaita (decision D19).

## Module boundaries

Organize modules around responsibilities and dependency direction rather than around individual types or functions. A useful general separation is pure domain logic, application workflows, external-system adapters, UI composition, and startup code; the exact file and directory layout should follow the subsystem's actual boundaries. Let modules depend on narrow contracts, keep I/O at adapter edges, and keep UI frameworks out of domain logic. Avoid duplicate mutable representations of the same state.

This guide defines coding principles, not the current source tree or the implementation plan. Consult the architecture and subsystem design documents for project-specific boundaries and current module ownership; record planned or completed structural changes in the project plan rather than maintaining a second module inventory here.

## Code documentation

Rustdoc comments are the standard for code-level intent and internal API documentation; generated docs are available with `just app docs` and are written to `app/target/doc/`. Keep product behavior, architectural rationale, and subsystem specifications in Markdown rather than duplicating them in code comments.

- Start each module with `//!` describing its responsibility, boundary, and important dependencies or constraints. Link to the relevant design spec when it is the source of detailed behavior.
- Use `///` on cross-module `pub` items, data types, and fields when their role, invariants, or semantics are not obvious from their names. These are internal APIs in this binary crate, not promises of a stable external library API.
- Document functions in terms of intent and contract: what the operation does, meaningful input constraints, returned/error outcomes, side effects, and safety or lifecycle guarantees. Include only the parts that are not already clear from the signature.
- Use `# Errors`, `# Panics`, and `# Safety` sections when those behaviors apply; add `# Examples` when an example clarifies correct use, not as boilerplate.
- Prefer concise present-tense descriptions. Explain why a non-obvious rule exists; do not narrate each line or restate the implementation.
- Ordinary private helpers do not need comments unless their logic or constraints are non-obvious. Keep implementation-local rationale next to the relevant code.

The `docs` recipe builds documentation for private items as well as `pub` items and treats rustdoc warnings (including broken intra-doc links) as errors. It is a focused documentation check, separate from the general `check` gate.

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

## Logging

The app uses the `log` crate with `env_logger` as the backend (single stderr destination, level-filtered, zero async runtime).

- Initialise in `main.rs` (and the `proto`/`wireframe` binaries) via:
  ```rust
  env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
  ```
- Control level with the standard `RUST_LOG` env var, e.g. `RUST_LOG=debug cargo run`. Default is `info`.
- Import macros with `use log::{debug, info, warn, error};` and use the appropriate level:
  - `error!` — failures that affect user-visible outcomes (save failed, nix check failed)
  - `warn!` — unexpected but recovered situations
  - `info!` — notable lifecycle events (startup, nix client resolved, apply begun)
  - `debug!` — detailed diagnostics for development (search ranking, queue mutations, nix command lines)
- Rule: **UI shows plain summaries to the user; raw internals go to the log**. Never dump nix output, backtraces, or internal state into the user-facing window (REQ-6). The one remaining `eprintln!` with the `nix-pilot:` prefix in `shell.rs` was migrated to `error!` — it now appears only when `RUST_LOG` permits.
- Binary entry points (`main`, `proto`, `wireframe`) each initialise the logger so `cargo run --bin <name>` works under `RUST_LOG` independently.
