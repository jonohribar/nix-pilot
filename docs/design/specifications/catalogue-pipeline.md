# Catalogue Pipeline Specification

How the Software store's application catalogue is generated, end to end. The catalogue is an independent generator product, not a side effect of the application; the desktop application only consumes its published artifacts ([architecture.md, Catalogue service](../architecture.md#catalogue-service)). This spec pins the pipeline decisions; the measurement of its coverage and precision is planned in [verification.md](../verification.md).

## Scope

- The first-release catalogue lists GUI applications the average user can install and run: rich desktop applications, basic desktop applications and uncertain GUI candidates. It does not attempt to classify the whole Nixpkgs package set (decision [D10](../../research/04-recommendations.md#decision-log)).
- Libraries and low-level tools are excluded from ordinary browsing (requirement [REQ-7](../../requirements/requirements.md)); they are not part of the catalogue.
- The catalogue is generated, never curated: no hand-maintained per-package content, mappings, screenshots or descriptions (decision [D4](../../research/04-recommendations.md#decision-log)).

## Inputs and cadence

- Inputs are a pinned Nixpkgs revision and upstream metadata (AppStream metainfo, freedesktop desktop entries) discoverable from that revision.
- The catalogue regenerates weekly by default (decision [D12](../../research/04-recommendations.md#decision-log)); the cadence is a tuning knob, not a fixed contract.
- Each regeneration is over one tracked Nixpkgs revision and one target architecture set; the result is a versioned, static, cacheable index (decision [D11](../../research/04-recommendations.md#decision-log)).

## Pipeline stages

1. **Pin the input.** Select a Nixpkgs revision and the target architecture set.
2. **Evaluate package metadata.** Enumerate package-set entries and export safe, serialisable metadata without building every package, per the metadata contract defined in the Phase-0 exporter output ([verification.md](../verification.md)).
3. **Filter candidates.** Exclude broken or incompatible packages from normal results, retaining the reason for exclusion where useful. Keep only GUI-app candidates (decision [D10](../../research/04-recommendations.md#decision-log)).
4. **Discover graphical applications.** Inspect AppStream metainfo and desktop-entry files in package outputs where those outputs are available, reusing existing local or binary-cache outputs rather than building.
5. **Enrich records.** Join application metadata to Nixpkgs package identity using evidence: AppStream package hints, desktop-entry executable names, `mainProgram`, output paths and upstream identifiers.
6. **Assign confidence.** Record how strongly an application component maps to a package. Join is multi-signal and tiered: package name, desktop-entry ID and binary name; only high-confidence joins are shown in ordinary browsing (decision [D13](../../research/04-recommendations.md#decision-log)).
7. **Index and publish.** Generate a versioned catalogue with per-entry architecture availability flags, so one artifact serves all supported architectures and clients filter by what an entry can build for (decision [D11](../../research/04-recommendations.md#decision-log)).
8. **Refresh.** Regenerate on the weekly cadence — periodic, not event-driven, so drift is observed by the run rather than discovered by users (risk [R12](../risks.md#r12)). A change to the pinned revision or upstream metadata takes effect at the next run.

## Record model

Each catalogue entry carries:

| Field | Contents |
|---|---|
| Identity | `pname`, `version`, Nixpkgs attribute path (the install target) |
| Presentation | display name, summary, description, categories, icons, screenshots where available |
| Compatibility | architecture availability flags, `meta.platforms`, broken status |
| Install | the attribute path, and the install scope default (per-user, decision [D9](../../research/04-recommendations.md#decision-log)) |
| Classification | the application class (below) |
| Confidence | the join confidence tier for the app-to-package mapping |

Presentation fields (AppStream data, screenshots, icons) enrich a listing when available but are never prerequisites for installation; a missing screenshot or description must not prevent install.

## Classification and confidence

A record is classified from evidence and rules, matching the agreed classification table:

| Class | Evidence | Presentation |
|---|---|---|
| Rich desktop application | AppStream component joined to a package | Normal Store listing, full available metadata |
| Basic desktop application | Valid `.desktop` entry joined to a package | Normal Store listing, generated or basic details |
| Uncertain GUI candidate | Heuristic evidence only | Lower-confidence or secondary results; never shown as certain |
| Non-application | No desktop-app evidence | Excluded from ordinary browsing |

- Uncertain matches are never presented as certain; the confidence tier travels with the record so the UI can demote them.
- The system must not silently fall back to a hand-maintained allowlist or mapping table.

## Availability and compatibility

- Because each record carries architecture availability flags, a listing can be "unavailable on your system" when the entry cannot build for the running architecture — a first-class state, not an error (risk [R07](../risks.md#r07)).

## Unfree packages

- Unfree applications are listed by default (decision [D1](../../research/04-recommendations.md#decision-log)); an optional user filter may hide them later.
- Hydra builds do not cover unfree packages (they are not built on Hydra), so deferring their images and metadata to Hydra alone is not possible; the primary source is Hydra where it applies, and binary-cache inspection for unfree coverage is evaluated separately.

## Relationship to measurement

Whether the pipeline reaches the scope target — coverage of GUI applications, precision of joins, metadata completeness — is unproven and must be measured in Phase 0. The measurement plan, the exact metrics and the provisional acceptance floors are in [verification.md](../verification.md) (decision [D15](../../research/04-recommendations.md#decision-log)).