# nix-pilot — Research Index

> Research note group · 2026-11-16 · Status: current Companion to `nixos_system_centre_handoff.md` in the repository root.

## How to read this folder

This folder records research done before implementation begins. It exists to prevent the project from repeating the mistakes of its predecessors. Read `01` and `02` for facts, `03` for analysis, `04` for decisions still open.

| File | Content | Reader |
|---|---|---|
| `01-prior-art.md` | Every significant prior attempt at a NixOS GUI, and why each stalled or died | Anyone new to the project |
| `02-catalogue-feasibility.md` | Technical evidence for the automatically generated app catalogue (handoff Phase 0) | Engineers starting Phase 0 |
| `03-failure-modes-and-risks.md` | The failure patterns that killed prior work, mapped to this project | Product owners, reviewers |
| `04-recommendations.md` | Concrete changes to the handoff, plus a decision log | Decision makers |

Living project artifacts live in `../design/` — navigation hub `../design/plan.md` (plan structure, handover translation map, epic links), risk register `../design/risks.md`, and future architecture/spec/verification documents. Requirement statements live in `../requirements/`. This folder holds stable research findings only.

## Research summary (verdict)

- **Possible:** yes. Every component has precedent. Package metadata evaluation without building is proven (search.nixos.org). Scoped GUIs over NixOS config have been demonstrated repeatedly.
- **Feasible as scoped:** conditionally. The handoff's scope constraints are correct and are the main reason this attempt is better positioned than its predecessors.
- **Unproven:** the catalogue generation pipeline (Phase 0). The underlying AppStream-metadata gap has been an open nixpkgs RFC since 2016. The environment is now the most favourable it has ever been (see `02`), but the work is unproven until measured.
- **Highest risk:** sustainability. Every predecessor, including the most popular one, died from single-maintainer collapse, not technical failure.

## Maintenance guidance

- One file per topic. New findings extend the existing file; a genuinely new topic gets a new numbered file and an index row.
- Update the "Status" line at the top of a file when its content goes stale. Superseded research is edited in place, never deleted.
- Every claim that informs a product decision must carry its source URL in the file where it first appears.
- Single source of truth: a fact lives in exactly one file. When content moves to another doc, delete it from the source (the handoff is being migrated this way). Do not duplicate facts; prefer clickable markdown links between files.
- Facts and opinions: factual evidence lives in `01` and `02`; analysis and recommendations live in `03` and `04`.
- Writing style: engineering documents carry substance only (what, why, how). No governance tables, owners, dates, beads issue ids or phase-gate language inside them — process lives in beads and in `../design/plan.md`. Governance-heavy files (`plan.md`, this index) are internal and are rewritten, not converted, at open-source release. Markdown is written with editor soft-wrap: one paragraph per source line, no fixed-width hard wraps. See `AGENTS.md` at the repo root for the short version of these rules.
