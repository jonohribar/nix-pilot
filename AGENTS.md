# nix-pilot — project notes

Consumer desktop GUI for managing NixOS (Settings + Software Store + Updates). Nix stays hidden from normal users. nix-pilot is the project name; the product has no public name yet.

## Documentation map

- `docs/research/` — stable findings: `00-README` (index), `01-prior-art`, `02-catalogue-feasibility`, `03-failure-modes-and-risks`, `04-recommendations` (decision log D1–D15)
- `docs/design/` — living docs: `plan.md` (hub), `risks.md` (R01–R12), `architecture.md`; `specifications/` holds the subsystem specs, `verification.md` the measurement plan
- `docs/requirements/` — `user-needs.md` (personas/stories/journeys), `requirements.md` (REQ-1…8 + constraints/search/ranking)

## Working rules

- Engineering docs: substance only (what, why, how). No governance tables, owners, dates, beads ids, phase-gate language — that lives in beads or `plan.md`.
- Ideas, bugs, features: file as beads tasks, evaluate later. Never decide on the spot.
- Decisions: record in `04-recommendations.md` decision log; decided rows are never deleted.
- Single source of truth: a fact lives in exactly one file. When content moves to another doc, delete it from the source. Prefer clickable markdown links over duplicating content.
- Markdown: one paragraph per source line; never hard-wrap at a fixed width (editors soft-wrap). Lists and tables still use one line per item or row.

## Commits

- Commit and push only when asked; never commit, push or force-push unprompted, and never amend a pushed commit.
- Commits group a coherent body of work; they need not be one atomic change each, but every commit stays self-consistent and reviewable.
- Subject line format `type(scope): summary` — type is one of `feat`, `fix`, `docs`, `chore`, `refactor`, `test`, `build`, `ci`, `perf`, `revert`; the subject is imperative and under 50 characters.
- Body on the following lines: what changed and why, at most 72 characters per line.

## Standing decisions

- No hand-maintained per-package catalogue content (D4). Curated *behaviour* (featured, categories, ranking) allowed if explainable.
- Unfree apps (Chrome, Discord) listed by default (D1); optional user filter later.
- Naming: nix-pilot is the project name only; the product has no public name yet (do not invent one).
- Beads: repo `nix-pilot`, id prefix `nix-pilot-`, one epic per plan section. Internal tooling may be AI-assisted; engineering docs must stay open-source-ready.
