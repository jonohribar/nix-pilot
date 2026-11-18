# nix-pilot — desktop client (WIP scaffold)

The product GUI: **Software Store**, **Settings**, **Updates/Recovery**, global search.
Stack (decision [D19](../../docs/research/04-recommendations.md#decision-log)): **Rust + GTK4 + libadwaita** (gtk4-rs).

Current state: Software Store window that reads the generated catalogue index
(`experiments/catalogue/results/catalogue-index.json`) and offers ranked grouped
search, with a detail/install surface per result (user vs system-wide scope, [D9](../../docs/research/04-recommendations.md#decision-log)).
Ranking mirrors the verified `experiments/catalogue/search.js` scorer (exact /
prefix / contains on attr+pname, plus a description bonus), so results sort the
same as the HTML viewer. Layout and the remaining surfaces (Settings, Updates/
Recovery, global search) are tracked as `nix-pilot-msn.2`.

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
