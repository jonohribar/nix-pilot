# nix-pilot — desktop client (WIP scaffold)

The product GUI: **Software Store**, **Settings**, **Updates/Recovery**, global search.
Stack (decision [D19](../../docs/research/04-recommendations.md#decision-log)): **Rust + GTK4 + libadwaita** (gtk4-rs).

Current state: Software Store window that reads the generated catalogue index
(`experiments/catalogue/results/catalogue-index.json`) and offers ranked grouped
search from the global header (the v1 whole-app search; catalogue-only until the
Settings/Updates surfaces land), with a detail/install surface per result (user vs
system-wide scope, [D9](../../docs/research/04-recommendations.md#decision-log)).
Ranking mirrors the verified `experiments/catalogue/search.js` scorer (exact /
prefix / contains on attr+pname, plus a description bonus); results are bucketed
into the D16/D18 labelled groups. The remaining surfaces (Settings, Updates/
Recovery) are tracked as `nix-pilot-msn.2`; the global-search design is
[`docs/design/specifications/global-search.md`](../../docs/design/specifications/global-search.md).

## Code layout + the check gate

Pure logic is split from the UI so it is testable without a window or nix:
`src/model.rs` (typed catalogue domain), `src/search.rs` (ranking),
`src/store.rs` (desired-change queue + result rule), `src/install.rs` (nix
adapter, typed `Outcome`), `src/updates.rs` (safe build-verify of queued
changes; apply gated until the desired-state layer), `src/ui/` (the GTK surface:
shell, detail+install, updates review), and `src/main.rs` (thin startup
composition). See `docs/design/coding-style.md`. Run `just app check` (fmt +
clippy + tests) before committing.

## Install intent (v1)

The detail page's Install button is an *intent flow*, not an imperative
installer: it checks the attribute against the system's configured nixpkgs
(`nix build --dry-run`, which never fetches or writes; revision drift surfaces
as the first-class "unavailable on your system" state, risk R07), reports that
state when the attr is missing or an alias, and drops the app into an
in-memory ready-to-change list with its scope (user/system, decision D9). Applying the queued changes is the Updates
transaction (failure-and-recovery.md), which is the next surface; nothing in
this flow mutates the system. The check logic lives in `src/install.rs` and is
covered by unit tests plus live `--ignored` integration tests.

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
