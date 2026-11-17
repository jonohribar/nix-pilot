# nix-pilot — Risk Register

> Living document · created 2026-11-16 · Status: current This register is reviewed and updated at each phase gate (Phase 0 exit, Phase 1 exit, Phase 2 milestones). Research analysis backing these risks lives in `../research/03-failure-modes-and-risks.md`; this register is the actionable artifact and does not duplicate that analysis.

## How this register works

**Scales**

- Likelihood: Low / Medium / High / Extreme — how likely the risk materialises within the project horizon.
- Impact: Low / Medium / High / Severe — worst realistic consequence for the project or its users.

**Status values**

- `Open` — acknowledged, no mitigation in place yet.
- `Mitigating` — a mitigation is designed and being implemented.
- `Accepted` — consciously accepted; the decision is recorded in the decision log (`../research/04-recommendations.md`).
- `Closed` — no longer applicable, or mitigation verified effective.

**Review cadence**

- Reviewed at every phase gate by the project owner; status and likelihood/impact updated in place.
- A risk is only `Closed` with a one-line reason in the review log.
- New risks are added to the summary table and detailed below it; the ID prefix `R` is never reused.

## Summary table

| ID | Risk | Likelihood | Impact | Status |
|---|---|---|---|---|
| R01 | Maintainer sustainability / single-maintainer collapse | Extreme | Severe | Mitigating |
| R02 | Phase 0 catalogue coverage below product floor | High | High | Mitigating |
| R03 | Knowledge gap at failure time (rebuild errors, no boot) | High | High | Mitigating |
| R04 | Settings surface maintenance / Nixpkgs option drift | High | Medium | Mitigating |
| R05 | Users outgrow the app-owned config (config-as-state corner) | Medium | Medium | Open |
| R06 | AppStream-to-package join errors at scale | High | Medium | Open |
| R07 | Catalogue and system revision drift | High | Medium | Open |
| R08 | Failed apply leaves system partially changed | Medium | High | Open |
| R09 | Scope: three products in one, settings unbounded | Medium | High | Open |
| R10 | No-curation rule erodes into a de facto allowlist | Medium | Medium | Mitigating |
| R11 | Licensing / redistribution of unfree metadata and icons | Medium | Medium | Mitigating |
| R12 | Nixpkgs / ecosystem movement outpaces the pipeline | Medium | Medium | Open |

## Detailed entries

### R01 — Maintainer sustainability / single-maintainer collapse

- **Description:** the project becomes the work of one volunteer; interest fades or life intervenes; the product rots without anyone noticing responsibly. This killed every predecessor, including the most popular (nix-software-center, 839 stars, "abandonware" verdicts, archive request 2026).
- **Evidence:** `../research/01-prior-art.md`; `../research/03-failure-modes-and-risks.md` risk B.
- **Mitigation:** (design) rapid de-risking phases so value is proven early; (process) a stated sustainability commitment — "maintainable by a 1–2 person volunteer team for five years"; repo home decided (D7: local for now, open-source project if it proves out, with a docs-conversion step before release); contribution paths for both engineers and catalogue-quality fixers; pipeline runs on cheap CI, not personal machines.
- **Status:** Mitigating (decisions recorded; contributor base unproven).

### R02 — Phase 0 catalogue coverage below product floor

- **Description:** the generated catalogue cannot reliably distinguish desktop applications from libraries/CLI tools, or coverage of real GUI apps falls below what a store needs to feel useful. The AppStream-metadata gap behind this has been an open nixpkgs RFC since 2016; unfree packages are not built on Hydra, so output-derived metadata for them is unavailable.
- **Evidence:** `../research/02-catalogue-feasibility.md`.
- **Mitigation:** (Phase 0 as written) measurement before UI; provisional precision/recall targets set before measuring (D3), reviewed after the QA sample; the negative result is a legitimate scope-changing finding, not a trigger to loosen the curation rule; unfree policy and output source decided (D1: unfree included by default; D2: Hydra first, binary-cache evaluation tracked in `../design/verification.md`).
- **Status:** Mitigating (decisions recorded; measurement pending).

### R03 — Knowledge gap at failure time

- **Description:** hiding Nix concepts works until something fails; then recovery requires exactly the concepts that were hidden (rebuild, generations, boot selection). A failed `nixos-rebuild` produces errors in dependency/linker language; a non-booting system cannot be helped by the app at all.
- **Evidence:** `../research/03-failure-modes-and-risks.md` risk C.
- **Mitigation:** failure-and-recovery spec accepted as a Phase 2 prerequisite (D5), specified in `../design/specifications/failure-and-recovery.md`; tightened Phase 1 acceptance criterion — "recover without opening a terminal" (recommendation 3, recorded in `../design/verification.md`); boot-time recovery story and pre-failure documentation; "details one click away, always shown on error" branding instead of "hides Nix".
- **Status:** Mitigating (prerequisite accepted; spec written).

### R04 — Settings surface maintenance / Nixpkgs option drift

- **Description:** settings workflows are product-authored by design (decision [D4](../research/04-recommendations.md#decision-log)); Nixpkgs renames, moves and deprecates options continuously, so the mapping from consumer setting to NixOS option silently breaks. This surface has no precedent and no automated safety net.
- **Evidence:** `../research/03-failure-modes-and-risks.md` risk A.
- **Mitigation:** versioned settings schema with documented migration (D6, decided); settings surface carried in the risk table with catalogue-level weight (recommendation 5); consumer-facing checks that surface "this setting no longer applies" instead of silently generating stale config.
- **Status:** Mitigating (approach decided; design pending).

### R05 — Users outgrow the app-owned config

- **Description:** the app owns the configuration it generates; users who eventually want something the app cannot express are in a corner — the generated config is not theirs, and the escape hatch (raw Nix) is out of scope for v1. Mixed handwritten + generated config is historically a trap (reverts to config-as-state).
- **Evidence:** `../research/03-failure-modes-and-risks.md` pattern 3, risk E.
- **Mitigation:** explicit, designed escape hatch (view/share the generated config; developer mode) even if editing stays out of scope; do not half-support "import existing config" in v1 — either full support or none.
- **Status:** Open.

### R06 — AppStream-to-package join errors at scale

- **Description:** mapping AppStream components to Nixpkgs packages via package names, desktop-entry names and executable names is ambiguous; the PackageKit-Nix backend's lived experience (unsearchable attributes, install/remove mismatches, crashes) is the baseline for naive joins.
- **Evidence:** `../research/02-catalogue-feasibility.md` ("joining AppStream components").
- **Mitigation:** confidence tiers and conservative presentation (catalogue-pipeline.md); Phase 0 QA sample in the hundreds, reviewed manually as QA not curation (verification.md).
- **Status:** Open.

### R07 — Catalogue and system revision drift

- **Description:** the catalogue is generated for a pinned Nixpkgs revision; the running system tracks another. Search can advertise packages that evaluate differently on the user's system. nix-software-center hit exactly this with its offline snapshot and "unavailable" dialog.
- **Evidence:** `../research/01-prior-art.md` (nix-software-center).
- **Mitigation:** bind catalogue to revision + architecture (catalogue-pipeline.md); validate installation against the actual system before applying; treat "unavailable on your system" as a first-class UI state, not an error.
- **Status:** Open.

### R08 — Failed apply leaves system partially changed

- **Description:** a rebuild interrupted or failed mid-transaction leaves the system in a confusing intermediate state for a user who cannot diagnose it.
- **Evidence:** the failed-build-never-partially-applies guarantee ([failure-and-recovery.md](specifications/failure-and-recovery.md)); `../research/03-failure-modes-and-risks.md` risk C.
- **Mitigation:** transaction semantics specified and tested in Phase 1 (evaluate → build → apply as a single unit; never partial); failure-and-recovery spec (recommendation 2) defines the guarantee and how it is tested.
- **Status:** Open.

### R09 — Scope: three products in one, settings unbounded

- **Description:** the product bundles a data platform (catalogue), an OS transaction engine (desired state → apply), and a consumer UX (Settings + Software + Updates). Each is a multi-year project; the settings surface in particular is unbounded and has no precedent.
- **Evidence:** `../research/03-failure-modes-and-risks.md` risk E.
- **Mitigation:** the phased plan is in place (plan.md); scope boundaries (no existing configs, no raw Nix in normal mode) are firm; if Phase 0 results are poor, narrow the v1 app universe rather than expanding engineering.
- **Status:** Open.

### R10 — No-curation rule erodes into a de facto allowlist

- **Description:** "no manual curation" is a hard requirement; but "safe, explicit product rules" is how a hand-maintained mapping sneaks back in, one justified exception at a time. Generated-only stores also look empty next to curated ones (GNOME Software, Discover), creating constant temptation.
- **Evidence:** `../research/02-catalogue-feasibility.md` question 4.
- **Mitigation:** rule decided 2026-11-16 — no hand-maintained per-package content; curated *behaviour* (featured lists, categories, ranking policy) allowed and product-owned, provided it is explainable and never required for installability (decision D4 in `../research/04-recommendations.md`). Enforcement against the pipeline must be demonstrated before Phase 2.
- **Status:** Mitigating (decision recorded; enforcement not yet implemented).

### R11 — Licensing / redistribution of unfree metadata and icons

- **Description:** indexing unfree packages, or redistributing their AppStream data and icons in a catalogue artifact, has licensing consequences (potentially making the index itself unfree). Unfree packages are also absent from Hydra, complicating any unified pipeline.
- **Evidence:** `../research/02-catalogue-feasibility.md` question 1 and the vlinkz POC finding.
- **Mitigation:** unfree policy decided (D1: unfree applications listed by default; optional user filter to hide unfree apps, implemented as an opt-in filter); images and metadata for unfree apps to come from binary-cache inspection, evaluation tracked in `../design/verification.md`; catalogue artifact split so the unfree portion can be distributed separately if required.
- **Status:** Mitigating (policy decided; implementation pending).

### R12 — Nixpkgs / ecosystem movement outpaces the pipeline

- **Description:** Nixpkgs is not a stable platform: package renames, option renames, channels vs flakes, changes to the package set structure. The pipeline (generator, schema, joins) has four surfaces tracking this drift permanently.
- **Evidence:** `../research/03-failure-modes-and-risks.md` risk B.
- **Mitigation:** pinned-revision lifecycle as a first-class pipeline concept (catalogue-pipeline.md); schema versioning; periodic (not event-driven) regeneration so drift is observed, not discovered by users.
- **Status:** Open.

## Review log

| Date | Review | Outcome |
|---|---|---|
| 2026-11-16 | Initial register created from research analysis | All risks Open; D1, D2, D3, D6, D7 pending owner assignment |
| 2026-11-16 | Decision round: D1–D7 resolved (see `../research/04-recommendations.md`) | R01–R04, R10, R11 moved to Mitigating |

## Cross-references

- `../research/00-README.md` — index of research notes
- `../research/03-failure-modes-and-risks.md` — analysis behind R01–R12
- `../research/04-recommendations.md` — mitigations (recommendations 1–8) and decision log (D1–D15)
