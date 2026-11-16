# Recommendations and Open Decisions

> Research note · 2026-11-16 · Status: current Concrete changes to `nixos_system_centre_handoff.md`, derived from the analysis in `03-failure-modes-and-risks.md`. The decision log at the end tracks each open question to a named owner and decision date.

## In one paragraph

The handoff is sound on scope and ordering; these recommendations close the gaps it leaves: a failure-and-recovery spec written before any UI work, a risk-table entry for the settings surface, an explicit failure-handling acceptance criterion, a precise statement of the no-curation rule, pinned Phase-0 inputs (especially the unfree-package policy), and a sustainability commitment. Most are one-page additions; none require changing the phased plan.

## Recommendations

### 1. Add a prior-art section to the handoff

Summarise `01-prior-art.md` in 10 lines: what was tried, why each project died, and the two patterns to avoid. Future readers will otherwise re-derive this at cost — the handoff currently reads as if the space were empty.

### 2. Write the failure-and-recovery spec before Phase 2 (highest priority)

One page answering, for a non-technical user:

- What shows on screen when a rebuild fails (plain-language summary of the error, the exact raw error one click away, offered recovery actions).
- What happens when the system does not boot (boot-time older-generation selection; documentation shipped *before* the failure, e.g. a recovery card in Settings).
- How rollback is explained without Nix vocabulary ("go back to yesterday's system") and how the app recovers when a applied generation partially changed the system.
- The explicit guarantee: a failed build must never leave the system partially applied (the handoff already promises this; the spec defines how it is tested).

### 3. Tighten the Phase 1 failure-handling acceptance criterion

"Confirm the GUI can explain each operation without exposing Nix concepts" is too weak. Replace with: "a user who makes a change that fails to build or apply can recover to a working system without opening a terminal, and the UI showed the error and its remedy in plain language."

### 4. State the no-curation rule precisely

Current wording invites a de facto allowlist through "safe, explicit product rules". Proposed statement:

- Forbidden: hand-authored per-package catalogue entries, screenshots, descriptions, or app-to-package mappings.
- Allowed and product-owned: curated *behaviour* — featured lists, categories, ranking policy, settings workflows — provided it is explainable and not required for a package to be installable.

### 5. Add the settings surface to the risk table

Nixpkgs cannot generate a consumer settings interface (handoff section 5, agreed), which means the settings workflows are a hand-maintained, unbounded surface that tracks Nixpkgs option renames forever. Give it a risk row with the same weight as the catalogue, and a mitigation: a versioned settings schema with a documented migration path (this also protects users from option renames between Nixpkgs revisions).

### 6. Pin the Phase-0 inputs and policy before starting

Three decisions from `02-catalogue-feasibility.md` that change the data model, so they cannot be deferred into the build:

- Unfree packages: indexed or not? If yes, metadata source (they are not on Hydra) and redistribution policy for icons/screenshots.
- Output inspection: Hydra-built outputs only, or arbitrary binary-cache inspection (storage/bandwidth engineering either way).
- Coverage targets: precision/recall floors for "GUI application found from Nixpkgs", set before any UI is built.

### 7. Budget for the negative Phase-0 result

Phase 0 is a 6–18 month research project with a real chance of "coverage is low and rising slowly". Write down in advance what that finding does to scope (e.g. narrower app universe for v1, closer coupling to the 2025–2026 nixpkgs indexing work — PR #556838, #579597) rather than loosening the curation rule.

### 8. State the sustainability commitment

One line: "this project must be maintainable by a 1–2 person volunteer team for five years". Apply it as a filter to every Phase-2 design decision — settings schema versioning, catalogue format stability, UI framework choice. Predecessors failed this test; naming it does not solve it, but it stops decisions being made without it.

## Decision log

Every decision is recorded here once made; decided rows are never deleted. Open decisions are filed as beads tasks under the Risks epic so they are not lost (see `../design/plan.md`). As of 2026-11-16, all initial decisions D1–D7 are resolved; the commit-workflow decision is D8.

| # | Decision | Outcome | Owner | Date | Status |
|---|---|---|---|---|---|
| D1 | Unfree-package policy for the catalogue | **Include unfree applications by default** — the average user expects Chrome and Discord. At most an optional user filter to hide unfree apps (feature task filed). Metadata and images for unfree apps come from binary-cache inspection, filed as a follow-up evaluation | Project owner | 2026-11-16 | Decided |
| D2 | Output inspection source | **Hydra first** (zero cost, covers most free GUI apps). Binary-cache inspection evaluated later, because Hydra cannot provide unfree packages or their images | Project owner | 2026-11-16 | Decided |
| D3 | Coverage targets for classification | **Set provisional precision/recall targets before Phase 0 measurement**, reviewed after the QA sample; exact floors defined in the Phase 0 planning task (`nix-pilot-nus.2`) | Project owner | 2026-11-16 | Decided |
| D4 | No-curation rule wording | **No hand-maintained per-package content.** Curated *behaviour* (featured lists, categories, ranking policy) remains allowed product choice, provided it is explainable and never required for an app to be installable | Project owner | 2026-11-16 | Decided |
| D5 | Failure-and-recovery spec | **Accepted as a Phase 2 prerequisite**; writing it is filed under the Specifications epic (`nix-pilot-9f3.1`) | Project owner | 2026-11-16 | Decided |
| D6 | Settings schema versioning | **Versioned schema with documented migration** | Project owner | 2026-11-16 | Decided |
| D7 | Repo/community home for sustained maintenance | **Local repo for now.** If the project proves out it becomes an open-source project (nix-community considered at that point). Internal docs may rely on AI-assisted tooling (e.g. beads); before any open-source release they are converted to developer-friendly, tool-independent format (chore task filed) | Project owner | 2026-11-16 | Decided |
| D8 | Home of the commit-workflow rule | **Root AGENTS.md, "Commits" section.** AGENTS.md is the short rule set, always loaded by the tooling at work time; beads and plan.md were rejected as homes because neither is read at commit time. At open-source release the rule moves to CONTRIBUTING.md as part of the AGENTS.md rewrite (D7) | Project owner | 2026-11-16 | Decided |

## Cross-references

- `00-README.md` — index and maintenance guidance
- `01-prior-art.md` — the record of previous attempts
- `02-catalogue-feasibility.md` — Phase 0 evidence and open questions
- `03-failure-modes-and-risks.md` — the analysis backing these recommendations
