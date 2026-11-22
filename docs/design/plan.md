# nix-pilot — Project Plan

> Living document · created 2026-11-16 · Status: current This file is the navigation hub for the whole project. It explains the plan structure (user needs, risks, requirements, architecture, specifications, verification, validation, application internals), maps every plan section to its destination document, and links each to its beads epic. Beads issue ids are referenced as `nix-pilot-<id>`.

## Purpose

Each plan section has one destination under `docs/` that folds in the review findings recorded in `docs/research/`. This plan file is the index that keeps every piece findable.

Working rules:

- Any bug, idea, feature or question is filed as a beads task under the matching epic and curated/evaluated later. Nothing is decided on the spot.
- Decisions are recorded in the decision log in `docs/research/04-recommendations.md`; an open decision always has a matching beads task.
- nix-pilot is the project name; the product has no public name yet. Do not invent a product name.
- A plan section may exist without a beads epic while its content is still being gathered. Epics are created bit by bit, section by section.
- Engineering documents carry substance only (what, why, how). Governance stays in beads and in this file; the short rule set lives in `AGENTS.md` at the repo root. The project becomes open source if it proves out; governance-heavy files are rewritten at that point, not converted.

## Plan structure

The plan has twelve sections. Each section has one destination under `docs/`, one beads epic (where content exists), and a status.

| # | Section | Content | Destination | Beads epic | Status |
|---|---|---|---|---|---|
| 1 | User needs | Who the product is for; primary user, destinations, example journeys | `docs/requirements/user-needs.md` | `nix-pilot-msn` | Written |
| 2 | Risks | Uncertain events with likelihood, impact, mitigation, owner | `docs/design/risks.md` | `nix-pilot-lw7` | Written (12 risks, R01–R12) |
| 3 | Requirements | What the product must do: constraints, functional needs, explicit non-goals | `docs/requirements/requirements.md` | `nix-pilot-l3q` | Written |
| 4 | Architecture | How the product is built: UI, product model, backend, catalogue service | `docs/design/architecture.md` | `nix-pilot-7wo` | Written |
| 5 | Specifications | Precise engineering specifications per subsystem | `docs/design/specifications/*.md` | `nix-pilot-9f3` | Written (failure-and-recovery, catalogue-pipeline, settings-schema) |
| 6 | Verification | How we prove requirements: phase gates, acceptance criteria, measurement | `docs/design/verification.md` | `nix-pilot-nus` | Written (Phase 0 measurement plan) |
| 7 | Validation | How we confirm the product meets user needs: usability, user testing | `docs/design/validation.md` | none yet | Deliberately empty; epic created when content exists |
| 8 | Desired-state ownership | Ownership of package intent, setting values, persistence, and validation boundaries | `docs/design/refactor-plan.md#desired-state-and-setting-ownership` | `nix-pilot-25p` | Implemented |
| 9 | System-query boundaries | Isolation of account and live-system queries from GTK surfaces | `docs/design/refactor-plan.md#system-and-account-query-boundary` | `nix-pilot-0bt` | Implemented |
| 10 | UI transaction lifecycle | Transaction state, Apply/rollback actions, and background-work lifecycle | `docs/design/refactor-plan.md#ui-transaction-and-background-work-lifecycle` | `nix-pilot-ddr` | Implemented |
| 11 | System backend cohesion | Candidate build, activation, storage, generations, and process boundaries | `docs/design/refactor-plan.md#system-backend-cohesion` | `nix-pilot-mf8` | Implemented |
| 12 | Shell and search presentation | Shell composition, search UI isolation, and derived search-cache ownership | `docs/design/refactor-plan.md#shell-and-search-presentation` | `nix-pilot-4kl` | Implemented |

## Document tree

```text
.
├── AGENTS.md
├── docs
│   ├── design
│   │   ├── architecture.md
│   │   ├── plan.md
│   │   ├── risks.md
│   │   ├── refactor-plan.md
│   │   ├── specifications
│   │   │   ├── catalogue-pipeline.md
│   │   │   ├── failure-and-recovery.md
│   │   │   └── settings-schema.md
│   │   └── verification.md
│   ├── requirements
│   │   ├── requirements.md
│   │   └── user-needs.md
│   └── research
│       ├── 00-README.md
│       ├── 01-prior-art.md
│       ├── 02-catalogue-feasibility.md
│       ├── 03-failure-modes-and-risks.md
│       ├── 04-recommendations.md
│       └── 05-phase0-findings.md
├── experiments
│   └── phase0
├── LICENSE
└── README.md
```

The tree lists actual files. Planned destinations not yet written — `validation.md` — are tracked in the tables below.

## Destination map

Each destination document folds in the review changes recorded in `docs/research/04-recommendations.md`; the beads task that produced it is listed for provenance.

| Destination | Review change folded in | Beads task |
|---|---|---|
| Condensed into this plan file | None (superseded by plan) | — |
| `docs/requirements/user-needs.md` | Outcome language; curated settings, not option trees | `nix-pilot-msn.1` |
| `docs/requirements/requirements.md` | Precise no-curation statement (decision D4); settings surface carried as a maintained product surface | `nix-pilot-l3q.1` |
| `docs/design/specifications/catalogue-pipeline.md` | No hand-maintained per-package content (D4); unfree-package policy (D1); confidence tiers enforced | `nix-pilot-9f3.3` |
| `docs/requirements/requirements.md` + `docs/design/specifications/settings-schema.md` | Search requirements separated from settings workflow schema | `nix-pilot-l3q.1`, `nix-pilot-9f3.2` |
| `docs/requirements/requirements.md` | Ranking as a separate signal; transparent labels; no telemetry by default | `nix-pilot-l3q.1` |
| `docs/design/architecture.md` | Settings lifecycle as a maintained surface; catalogue pipeline as a product | `nix-pilot-7wo.1` |
| `docs/design/architecture.md` + `docs/design/specifications/failure-and-recovery.md` | Transaction guarantee (failed build never leaves partial state) specified and tested | `nix-pilot-7wo.1`, `nix-pilot-9f3.1` |
| `docs/design/risks.md` (merged into register) | Risk register format with likelihood/impact/owner/status | Done at creation; `nix-pilot-lw7` tasks carry open decisions |
| `docs/design/verification.md` (phase gates) | Negative Phase 0 result is a scope change, not a defect | `nix-pilot-nus.2` |
| `docs/design/verification.md` | Phase 1 criterion tightened: recover without a terminal | `nix-pilot-nus.1` |
| Preserved as links inside translated documents | Links kept with their source claim | — |

## Beads epics

| Epic | Id | Children | Scope |
|---|---|---|---|
| User needs and product intent | `nix-pilot-msn` | 1 | Personas, journeys, product intent |
| Risk management and open decisions | `nix-pilot-lw7` | 7 | Risk register upkeep; decisions D1–D15 (resolved); open-source docs conversion |
| Requirements | `nix-pilot-l3q` | 2 | Constraints, functional requirements, non-goals, search, ranking; unfree filter |
| Architecture | `nix-pilot-7wo` | 1 | Product model, backend, catalogue service, install model |
| Specifications | `nix-pilot-9f3` | 4 | Catalogue pipeline, settings schema, failure-and-recovery, binary-cache evaluation |
| Verification | `nix-pilot-nus` | 2 | Phase 0 measurement, acceptance criteria |
| Validation | — | — | not created yet (no content) |
| Desired-state ownership | `nix-pilot-25p` | 3 | Single ownership contract for package intent; remove settings/state dependency cycle; clarify validation boundaries |
| System-query boundaries | `nix-pilot-0bt` | 2 | Isolate account queries from GTK and share read-only NixOS config discovery |
| UI transaction lifecycle | `nix-pilot-ddr` | 3 | Fix callback lifecycle; centralize transitions and worker completion |
| System backend cohesion | `nix-pilot-mf8` | 2 | Separate backend operation responsibilities and unify process handling |
| Shell and search presentation | `nix-pilot-4kl` | 2 | Isolate search UI and clarify derived search-cache lifecycle |

## Decisions

All product decisions D1–D15 are resolved; their outcomes are maintained in one place, the [decision log](docs/research/04-recommendations.md#decision-log). This section links rather than restates them so decisions are not kept in two places.

## Status (current)

- Research + decisions D1–D30 complete; risk register live.
- Product code: a working Rust + GTK4 desktop app (browse/search a real 27k-package catalogue, install-intent queue, 'Your system' recipe view with remove actions).
- Settings is a two-column router: its categories live in the left sidebar (System, Software, Network & internet, Accounts, Time & language, Personalisation, Privacy & security) and are always visible beside the cards, with no drill-in home page or profile header inside the surface; Updates stays a top-level sidebar row, not a category (D28), and sound + bluetooth are single-value workflows beside SSH keys and app-owned accounts.
- Every setting renders through one shared card, used by both the Settings page and the Updates Automatic upkeep card: title + info icon (detailed description on hover), short summary, inline capitalised On/Off for toggles, always-visible Reset to default, and mirror-reality live preselection (settings schema: [settings-schema.md](specifications/settings-schema.md)).
- The app-wide search sits at the top of the sidebar and searches packages + settings (D28, fyq); the store's local search + Hide non-free filter live inside the Packages tab (D29).
- The Updates page drives the transaction machine and carries an Automatic upkeep card (GC, store optimisation, system auto-upgrade), a Storage card (read-only usage + run-now cleanup through the helper) and a System-versions card (generations list + rollback).
- The app builds a candidate generation from a persisted desired-state doc (D20/D21) and ships the activation module + polkit helper (D24–D26) with the added `gc`/`optimise`/`gc-estimate` fixed actions; activation itself is spec/test-only until an installed-host verification.
- Further work tracked in beads: `7wo.3` (installed-host activation), `7wo.4` (catalogue service; slices `7wo.5`/`6`/`7`), `9f3.13` (try-without-installing), `9f3.9` (per-app version channels); catalogue service/distribution is the next big body. The 2026-11-20 visual/usability review filed UX follow-ups (gih, mmj, jag, sje, dxy, rqe, ae7, hd9, x6r, 7pa, sqi) — the spec-side rules are already folded into the destinations above; the app-side fixes are tracked on those beads.
- The app-internals review identified ownership and dependency crossings, concentrated UI composition, a possible Apply/rollback callback lifecycle defect, and backend responsibility overlap. The scoped refactor is planned in sections 8–12 and tracked by `nix-pilot-25p`, `nix-pilot-0bt`, `nix-pilot-ddr`, `nix-pilot-mf8`, and `nix-pilot-4kl`; implementation order and boundary rules are in [refactor-plan.md](refactor-plan.md).
- All destinations under `docs/` are written except `validation.md` (planned placeholder; its epic is created when content exists). The destination map above records the beads task that produced each.
