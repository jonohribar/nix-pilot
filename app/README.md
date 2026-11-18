# nix-pilot — desktop client (WIP scaffold)

The product GUI: **Software Store**, **Settings**, **Updates/Recovery**, global search.
Stack (decision [D19](../../docs/research/04-recommendations.md#decision-log)): **Rust + GTK4 + libadwaita** (gtk4-rs).

Current state: a single window with a sidebar (Home / Settings / Updates) and a
stack of surfaces that read the generated catalogue index
(`experiments/catalogue/results/catalogue-index.json`) plus the app-owned
persisted desired state. The Home surface offers ranked grouped search from the
global header (the v1 whole-app search, per
[`global-search.md`](../../docs/design/specifications/global-search.md)), with a
detail/install surface per result (user vs system-wide scope,
[D9](../../docs/research/04-recommendations.md#decision-log)). Ranking mirrors
the verified `experiments/catalogue/search.js` scorer (exact / prefix / contains
on attr+pname, plus a description bonus); results are bucketed into the D16/D18
labelled groups. The Updates surface drives the transaction machine: Prepare and
build dry-runs each change, then *builds* the candidate generation from the
desired state (`nixos-rebuild build`, store-only — never activates; see
`src/apply.rs`); activation/rollback stay gated on 7wo.3. The Settings surface
lists the REQ-5 areas with plain-language wording; setting workflows and the
settings schema are product work (beads 9f3.8).

## Code layout + the check gate

Pure logic is split from the UI so it is testable without a window or nix:
`src/model.rs` (typed catalogue domain), `src/search.rs` (ranking),
`src/store.rs` (desired-change queue + result rule), `src/install.rs` (nix
adapter, typed `Outcome`), `src/updates.rs` (transaction machine + safe build-verify of queued
changes), `src/apply.rs` (builds the candidate generation from the desired
state with `nixos-rebuild build` — build-only, never activates; verified by
a live scratch-build test on a NixOS host), `src/desired_state.rs`
(the persisted desired-state model: versioned JSON at
`$XDG_DATA_HOME/nix-pilot/desired-state.json`, loaded on launch and rewritten
on every change, atomic writes), `src/ui/` (the GTK surface:
shell with sidebar navigation, store, detail+install, updates page), and
`src/main.rs` (thin startup composition). See `docs/design/coding-style.md`. Run `just app check` (fmt +
clippy + tests) before committing.

## Install intent (v1)

The detail page's Install button is an *intent flow*, not an imperative
installer: it checks the attribute against the system's configured nixpkgs
(`nix build --dry-run`, which never fetches or writes; revision drift surfaces
as the first-class "unavailable on your system" state, risk R07), reports that
state when the attr is missing or an alias, and drops the app into a
ready-to-change set with its scope (user/system, decision D9), which is
persisted to the desired-state file so selections survive a restart. Applying
the queued changes is the Updates transaction (failure-and-recovery.md):
Prepare-and-build already *builds* the candidate generation (store-only via
`nixos-rebuild build`); activating it (switching) and rollback stay gated until
7wo.3. Nothing in this flow mutates the system. The check logic lives in
`src/install.rs` and is covered by unit tests plus live `--ignored` integration
tests.

## Dev shell + run

```sh
nix-shell app/shell.nix          # gives cargo/rustc + GTK4 + libadwaita (pinned nixpkgs)
cd app
cargo build
cargo run                        # index auto-detected from repo-root or phase0 cwd;
                                 # or: cargo run -- /path/to/catalogue-index.json
```

The catalogue index is generated output (gitignored) — regenerate it with the
pipeline runbook in `experiments/catalogue/README.md` if it is absent.
