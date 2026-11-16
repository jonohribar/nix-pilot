# nix-pilot — Project Plan

> Living document · created 2026-11-16 · Status: current This file is the navigation hub for the whole project. It explains the plan structure (user needs, risks, requirements, architecture, specifications, verification, validation), maps every part of the handover document to its destination document, and links each plan section to its beads epic. Beads issue ids are referenced as `nix-pilot-<id>`.

## Purpose

The repository is being translated from a single handover document (`nixos_system_centre_handoff.md`) into proper project documentation, one document at a time, each folding in the review findings recorded in `docs/research/`. This plan file is the index that makes the translation visible and keeps every piece findable.

Working rules:

- Any bug, idea, feature or question is filed as a beads task under the matching epic and curated/evaluated later. Nothing is decided on the spot.
- Decisions are recorded in the decision log in `docs/research/04-recommendations.md`; an open decision always has a matching beads task.
- nix-pilot is the project name; the product has no public name yet. Do not invent a product name.
- A plan section may exist without a beads epic while its content is still being gathered. Epics are created bit by bit, section by section.
- Engineering documents carry substance only (what, why, how). Governance stays in beads and in this file; the short rule set lives in `AGENTS.md` at the repo root. The project becomes open source if it proves out; governance-heavy files are rewritten at that point, not converted.

## Plan structure

The plan has seven sections. Each section has one destination under `docs/`, one beads epic (where content exists), and a status.

| # | Section | Content | Destination | Beads epic | Status |
|---|---|---|---|---|---|
| 1 | User needs | Who the product is for; primary user, destinations, example journeys | `docs/requirements/user-needs.md` | `nix-pilot-msn` | Written (handover §1–2) |
| 2 | Risks | Uncertain events with likelihood, impact, mitigation, owner | `docs/design/risks.md` | `nix-pilot-lw7` | Written (12 risks, R01–R12) |
| 3 | Requirements | What the product must do: constraints, functional needs, explicit non-goals | `docs/requirements/requirements.md` | `nix-pilot-l3q` | Written (handover §3, §5, §6) |
| 4 | Architecture | How the product is built: UI, product model, backend, catalogue service | `docs/design/architecture.md` | `nix-pilot-7wo` | Not yet translated |
| 5 | Specifications | Precise engineering specifications per subsystem | `docs/design/specifications/*.md` | `nix-pilot-9f3` | Tasks open (3 specs) |
| 6 | Verification | How we prove requirements: phase gates, acceptance criteria, measurement | `docs/design/verification.md` | `nix-pilot-nus` | Tasks open (2) |
| 7 | Validation | How we confirm the product meets user needs: usability, user testing | `docs/design/validation.md` | none yet | Deliberately empty; epic created when content exists |

## Document tree

```
docs/
  research/        stable research findings (index 00, prior art 01,
                   feasibility 02, failure modes 03, recommendations 04)
  requirements/    requirement documents (user-needs.md, requirements.md)
  design/          living project documents
    plan.md        this file (hub)
    risks.md       risk register (R01–R12)
    architecture.md            (destination of translation task)
    specifications/
      catalogue-pipeline.md    (destination of translation task)
      settings-schema.md       (destination of translation task)
      failure-and-recovery.md  (destination of translation task)
    verification.md            (destination of phase-gate criteria)
    validation.md              (empty placeholder)

nixos_system_centre_handoff.md   source document (repo root). Translated
                                 bit by bit; archived as superseded when
                                 every section has a destination.
```

## Translation map — where handover content lands

Each handover section translates to a destination document, and the review changes listed in `docs/research/04-recommendations.md` are folded in during translation, not later.

| Handover section | Destination | Review change folded in | Beads task |
|---|---|---|---|
| §1 Executive summary | Condensed into this plan file | None (superseded by plan) | — |
| §2 Product intent and UX | `docs/requirements/user-needs.md` | Outcome language; curated settings, not option trees | `nix-pilot-msn.1` |
| §3 Constraints and non-goals | `docs/requirements/requirements.md` | Precise no-curation statement (decision D4); settings surface carried as a maintained product surface | `nix-pilot-l3q.1` |
| §4 Generated catalogue | `docs/design/specifications/catalogue-pipeline.md` | No hand-maintained per-package content (D4); unfree-package policy (D1); confidence tiers enforced | `nix-pilot-9f3.3` |
| §5 Search and discovery | `docs/requirements/requirements.md` + `docs/design/specifications/settings-schema.md` | Search requirements separated from settings workflow schema | `nix-pilot-l3q.1`, `nix-pilot-9f3.2` |
| §6 Popularity and ranking | `docs/requirements/requirements.md` | Ranking as a separate signal; transparent labels; no telemetry by default | `nix-pilot-l3q.1` |
| §7 Architecture | `docs/design/architecture.md` | Settings lifecycle as a maintained surface; catalogue pipeline as a product | `nix-pilot-7wo.1` |
| §8 Install and system-state model | `docs/design/architecture.md` + `docs/design/specifications/failure-and-recovery.md` | Transaction guarantee (failed build never leaves partial state) specified and tested | `nix-pilot-7wo.1`, `nix-pilot-9f3.1` |
| §9 Risks and open questions | `docs/design/risks.md` (merged into register) | Risk register format with likelihood/impact/owner/status | Done at creation; `nix-pilot-lw7` tasks carry open decisions |
| §10 Recommended next steps | `docs/design/verification.md` (phase gates) | Negative Phase 0 result is a scope change, not a defect | `nix-pilot-nus.2` |
| §11 Acceptance criteria | `docs/design/verification.md` | Phase 1 criterion tightened: recover without a terminal | `nix-pilot-nus.1` |
| §12 Reference material | Preserved as links inside translated documents | Links kept with their source claim | — |

## Beads epics

| Epic | Id | Children | Scope |
|---|---|---|---|
| User needs and product intent | `nix-pilot-msn` | 1 | Personas, journeys, product intent (handover §1–2) |
| Risk management and open decisions | `nix-pilot-lw7` | 7 | Risk register upkeep; decisions D1–D7 (resolved); open-source docs conversion |
| Requirements | `nix-pilot-l3q` | 2 | Constraints, functional requirements, non-goals, search, ranking; unfree filter |
| Architecture | `nix-pilot-7wo` | 1 | Product model, backend, catalogue service, install model |
| Specifications | `nix-pilot-9f3` | 4 | Catalogue pipeline, settings schema, failure-and-recovery, binary-cache evaluation |
| Verification | `nix-pilot-nus` | 2 | Phase 0 measurement, acceptance criteria |
| Validation | — | — | not created yet (no content) |

## Decisions

All initial product decisions D1–D7 are resolved; their outcomes and dates are maintained in one place, the [decision log](docs/research/04-recommendations.md#decision-log). This section links rather than restates them so decisions are not kept in two places.

## Status (current)

- Research complete: `docs/research/00-README.md` through `04-recommendations.md`.
- Risk register live: `docs/design/risks.md`, 12 risks, review log opened.
- Six epics and 17 tasks filed in beads; the six decision tasks were resolved by the 2026-11-16 decision round (D1–D7). Validation intentionally without an epic.
- Handover translation in progress: `docs/requirements/user-needs.md` (handover §1–2) and `docs/requirements/requirements.md` (handover §3, §5, §6) are complete; the remaining destinations are open tasks in the translation map above.
