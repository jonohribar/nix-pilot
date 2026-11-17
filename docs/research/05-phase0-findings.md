# Phase 0 Findings — Catalogue Generation

> Research note · 2026-11-17 · Status: current Measured outcome of the catalogue-generation wager against one pinned Nixpkgs snapshot (narHash `sha256-qXFIqA6FCcx7XeDzw0ZNlkDT9UiCzrNDwK8RuU8RcHM=`). Instrument and run details: `experiments/phase0/` (README and scripts). The study's open questions map to [02-catalogue-feasibility.md](02-catalogue-feasibility.md) and the acceptance floors in [verification.md](../design/verification.md).

## In one paragraph

The wager is substantially proven, with two caveats. Enumerating the whole package set's metadata is cheap and near-complete (~30 s, ~99 % of fields). The name→package join finds real apps with high recall and near-perfect precision once libraries and CLIs are tiered out. Roughly 13 % of derivations — about 3,200 applications — are genuine GUI candidates on which the join has data to work. The caveats: description-keyword classification alone cannot reliably reject CLIs (it needs output-derived desktop evidence), and unfree applications have complete metadata but no public-cache outputs, so their enrichment and installation take a separate route.

## Findings

| Area | Measured result |
|---|---|
| Enumeration | 30.2 s for all 27,271 top-level attributes; 24,658 derivations; 1,804 (6.6 %) fail to evaluate |
| Metadata completeness | pname/version ~99.5 %; description ~98.7 %; homepage ~97 %; licence ~98.7 %; mainProgram 64 % |
| Join recall | 94.2 % of known apps found correctly (86 known apps in the QA sample) |
| Join precision | 98.8 % of rows presented as apps are right; 77.1 % if libs/CLIs are naively presented as apps |
| Classification (description heuristics) | 86.9 % accuracy on the QA sample; at app-candidate scale ~44 % of heuristic app candidates are evidence-backed (rich 18.5 % / basic 25.3 % / uncertain 56.2 % over 482 tiered), the uncertain majority being TUI/CLI tools; the app pool also carries an unfree/commercial class whose instantiation is refused on an unfree licence (no cache route, enrichment deferred) |
| Coverage | 13.0 % ± 3.8 % of random derivations ship a desktop entry ≈ 3,200 GUI-app candidates; metainfo 4.0 % |
| Unfree | metadata fully present in the default export; outputs 404 on cache.nixos.org and nix-community.cachix.org |

## What this means

- A generated-only store has a real app universe: roughly three thousand installable GUI candidates. That is smaller than a curated store's listing and larger than the failure case, and it matches the "coverage is low and rising slowly" caution from the research — the finding must keep allowing scope change rather than loosening the no-curation rule.
- Classification is not optional. Presenting every derivation as an app collapses precision to 77 %. The tiered classification in the pipeline spec is now empirically justified, and it must use desktop-entry evidence; the coverage sampler is the instrument that gathers it.
- Unfree applications are first-class in the catalogue (they evaluate with full metadata) but second-class in enrichment: their icons and screenshots have no public-cache source. The interim route is list-with-eval-metadata and install via local build; a community-cache or analytics-fed alternative is the evaluated future.
- The join's single wrong case in the QA sample (plain `notion` is a tiling window manager, not the notes app) is the shape of what confidence tiering must catch: common names can collide with unrelated packages.

## Integration points

- Pipeline: [catalogue-pipeline.md](../design/specifications/catalogue-pipeline.md) — sequence, record model, classification table.
- Floors: [verification.md](../design/verification.md) — provisional acceptance floors against which these numbers will be reviewed.
- Scope decisions: unfree listing ([D1](../research/04-recommendations.md#decision-log)), catalogue scope ([D10](../research/04-recommendations.md#decision-log)), join tiering ([D13](../research/04-recommendations.md#decision-log)), provisional floors ([D15](../research/04-recommendations.md#decision-log)).
- Open product decisions this note feeds: the CLI-labelled result class (should CLI tools appear in search, labelled, never as app tiles?) and the unfree enrichment route.