# nix-pilot — project notes

Consumer desktop GUI for managing NixOS (Settings + Software Store + Updates); Nix stays hidden from users.

## Documentation map

- `docs/research/` — stable findings + decision log (`00-README`, `04-recommendations.md` D1–D19)
- `docs/design/` — living docs: `plan.md` (hub), `risks.md`, `architecture.md`, `specifications/`, `verification.md`
- `docs/requirements/` — `user-needs.md`, `requirements.md`
- `experiments/` — R&D prototypes, separate from product code; versioned, may graduate
- `app/` — product code: the desktop client (Rust + GTK4 + libadwaita, D19)

## Working rules

- Engineering docs: substance only (what, why, how); no governance tables, owners, dates, beads ids, or phase-gate language.
- Markdown: use one-sentence-per-line style (soft-wrap).
- SSOT: one fact lives in one file; link, don't duplicate.
- Ideas, bugs, features: backlog-first — file as beads tasks, decide when picked up.
- Decisions: record in the decision log; decided rows are never deleted.

## Commits

- Commit and push only when asked; never amend a pushed commit.
- Conventional Commits; commits are cohesive and self-consistent.
- Subject: concise and abstract — name the outcome, not the file list (e.g. `refactor(app): internal-boundary cleanup`).
- Body: one short paragraph, ≤72 columns per line, intent-level only.

## Standing decisions

- No hand-maintained per-package catalogue content (D4); curated behaviour allowed if explainable.
- Unfree apps listed by default (D1).
- Naming: `nix-pilot` is the project name only — the product has no public name yet.
- Beads: repo `nix-pilot`, id prefix `nix-pilot-`, one epic per plan section; AI-assisted tooling OK, engineering docs stay open-source-ready.