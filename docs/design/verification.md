# Verification Plan

How the product proves its core assumptions before the consumer experience is built. The most consequential is Phase 0: the catalogue generation pipeline is the project's central risk — every component is precedented, but the AppStream-metadata gap is unproven (research, [catalogue-feasibility](../research/02-catalogue-feasibility.md)). This plan turns that risk into a measured gate.

## Phase 0 — measure catalogue generation

Run in this order; each step produces a measured result, not a fixed pipeline.

1. **Pin the input.** One pinned Nixpkgs revision and one target architecture (for example `x86_64-linux`).
2. **Evaluate metadata.** A small exporter enumerates package entries and emits JSON: identity, version, description, homepage, licence, maintainers, platform compatibility, broken status and `mainProgram` where present.
3. **Measure, on the exporter output**: evaluation wall-time, output size, number of skipped and evaluation-failing entries, and metadata completeness by field.
4. **Test discovery.** Whether AppStream metainfo and desktop-entry files are discoverable from already-available package outputs and binary-cache outputs, reusing them rather than building. This is the unproven half and the crux of the whole risk.
5. **Build the join.** Automated, multi-signal, confidence-scored join logic (decision [D13](../research/04-recommendations.md#decision-log)); no hand-maintained mappings.
6. **QA sample.** A hand-labelled sample of applications — hundreds of apps, not a short list of well-known ones — to measure classification and mapping quality. Manual review here is QA, not catalogue curation.
7. **Produce a first searchable catalogue** and document its coverage and omissions (decision [D10](../research/04-recommendations.md#decision-log) scope).

## Metrics

| Metric | What it measures |
|---|---|
| Evaluation wall-time | Whether export is cheap enough to run on the cadence (decision [D12](../research/04-recommendations.md#decision-log)) |
| Output size | Whether the versioned index is cacheable and small enough to distribute |
| Skipped / evaluation-failing entries | How many packages the exporter cannot evaluate, and why |
| Metadata completeness | Coverage per field (`pname`, version, description, licence, …) across candidates |
| Discovery rate | Share of GUI candidates with discoverable metainfo or desktop-entry data |
| Join precision / recall | How many true app-to-package joins are found, and how many shown are wrong |
| Classification accuracy | How well rich/basic/uncertain/non-app classes match the hand-labelled sample |

## Provisional acceptance floors

Provisional floors are set before Phase 0 runs, per decision [D3](../research/04-recommendations.md#decision-log), and are reviewed against the QA sample (decision [D15](../research/04-recommendations.md#decision-log)):

- **Coverage (recall):** at least 95% of the hand-labelled GUI-app sample is discovered.
- **Precision:** at most 2% of entries presented as certain are wrong joins; uncertain matches are always demoted, never shown as certain.
- **Evaluation cost:** the exporter completes within a bounded wall-time and produces an index small enough to distribute, both set numerically in the measurement harness.
- **Completeness:** a defined share of GUI entries carry at least a name and a summary.

These are provisional: if the QA sample contradicts a floor, the floor is revised and the revision recorded, not silently met by loosening the curation rule.

## Negative-result budget

Phase 0 may return "coverage is low and rising slowly". That finding must change scope rather than loosen the no-curation rule (recommendation [7](../research/04-recommendations.md#7-budget-for-the-negative-phase-0-result)). Contingencies are documented in advance — for example a narrower first-release app universe, or closer coupling to the 2025–2026 nixpkgs indexing work (`packages-info`, treewide metainfo) — so the fallback is a decision, not a scramble.

## Binary-cache inspection evaluation

Follow-up to decision [D2](../research/04-recommendations.md#decision-log) (Hydra first). Hydra does not build unfree packages, so their images and metadata must come from elsewhere. This evaluation answers, for arbitrary binary-cache inspection: bandwidth and storage cost at the project's scale, coverage of unfree GUI applications, and whether the result is viable next to Hydra's metadata. It runs late in Phase 0, once the Hydra-derived baseline exists to compare against.