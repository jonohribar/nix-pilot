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
