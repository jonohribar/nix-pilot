# Catalogue Generation Feasibility (Handoff Phase 0)

> Research note · 2026-11-16 · Status: current Evidence for the handoff's core wager: that a useful application catalogue can be generated from Nixpkgs and upstream metadata without per-package curation. See handoff sections 4, 9, 10.

## In one paragraph

The two halves of Phase 0 have very different evidence. **Package metadata evaluation without building is proven**: search.nixos.org has done exactly that (evaluate, serialise, index) for years. **Output-derived discovery (desktop entries, AppStream metainfo) is unproven**: the AppStream-metadata RFC in nixpkgs has been open since 2016, every proposed route has stalled, and the unfree-package coverage problem remains unsolved. However, nixpkgs itself started moving toward this approach's data needs in 2025–2026 (desktop-entry indexing via `passthru`; treewide metainfo installation), so the environment is the most favourable it has ever been. Phase 0 remains a research project with a real chance of a "coverage is low and rising slowly" finding, and that finding must be allowed to change the product scope.

## What is proven

### Package metadata evaluation without building

- search.nixos.org evaluates (does not build) Nixpkgs and indexes the result in Elasticsearch, in a GitHub action, for several Nixpkgs revisions and public flakes: https://discourse.nixos.org/t/why-doesnt-search-nixos-org-also-search-binary-names/16728
- Its own author confirms the hard limitation: "As we do not build any derivation there is no way to tell the outputs given just the nix definition" — i.e. **binary/output names are precisely what evaluation cannot see**. That is the whole of the handoff's Phase-0 problem in one sentence.
- Known evaluation hazards (feed the Phase-0 exporter's measurement list):
  - recursion cut-offs: `nodePackages` hidden behind `dontRecurseIntoAttrs` (https://github.com/NixOS/nixpkgs/issues/376662);
  - aliases and non-derivation attrs;
  - lazy evaluation failures and skipped entries.
- Feasibility conclusion: metadata export is cheap relative to building — the handoff's claim, and consistent with how search.nixos.org already operates.

## What is unproven

### The AppStream-metadata gap (open since 2016)

- RFC "Generating AppStream Metadata", nixpkgs issue #15932, created Jun 2016, still open, marked "Still actual topic" as recently as Jun 2026: https://github.com/NixOS/nixpkgs/issues/15932
- Routes proposed and stalled:
  - Hydra-generated flat file of outputs (narlist aggregation) → abandoned;
  - `nix2appstream` (matthewbauer, 2017) → abandoned;
  - `appstream-generator` (the tool normal distros use) → requires a **backend written in D**, never written.
- The practical blocker found by vlinkz's 2022 proof of concept (nixos-appstream-generator, which builds a package and scrapes its outputs): **unfree packages are not built on Hydra**, so output-derived metadata cannot cover them from Hydra data alone, and hosting their icons downstream may itself make the index unfree: https://discourse.nixos.org/t/brainstorming-generating-appstream-metadata/19273
- Practical consequence: "discover desktop apps without building everything" is a real research question, not a solved one. It also creates the derivative question of what to do about unfree/redistribution policy — a decision the handoff has not yet taken (see `04-recommendations.md`).

### Joining AppStream components to Nix packages

- The PackageKit Nix backend's lived experience is the baseline for naive joins: nested attributes unsearchable, install/remove mismatches, search crashes (https://github.com/NixOS/nixpkgs/pull/94055). The handoff's confidence-tiered join design is the right response, but it is untested at scale. Phase 0 step 6's QA sample should be "hundreds of apps", not "a sample of well-known desktop applications".

## Encouraging new evidence (2025–2026)

- nixpkgs PR #556838: `packages-info` indexes packages' desktop entries by exposing entry metadata via `passthru` (structured values without parsing files, no rebuilds). This is the data the handoff's catalogue needs, becoming available inside the package set itself: https://github.com/NixOS/nixpkgs/pull/556838
- nixpkgs PR #579597 (treewide): install upstream AppStream metainfo for CMake-based packages into `share/metainfo`: https://github.com/NixOS/nixpkgs/pull/579597
- Fresh community interest: "NixOS plugin for GNOME Software" (Feb 2026): https://discourse.nixos.org/t/nixos-plugin-for-gnome-software/75453

## Decision-relevant open questions

1. **Unfree policy.** Do we index unfree packages at all? If yes, how do we get their output-derived metadata (they are not on Hydra), and what do we redistribute (icons, screenshots) under what licensing?
2. **Output inspection source.** Hydra-built outputs only, or arbitrary binary-cache inspection? Arbitrary cache inspection is a storage/bandwidth engineering problem the handoff hand-waves ("reuse existing local or binary-cache outputs when possible").
3. **Coverage floor.** What is an acceptable precision/recall target for "GUI application found from Nixpkgs"? The classification policy table in the handoff is sensible but entirely unmeasured; set targets before building the UI.
4. **Curation-rule tension.** Distro stores that feel good (GNOME Software, Discover) are heavily curated upstream. A generated-only catalogue will be technically pure and visibly empty. Decide now whether "no manual curation" means no per-package *content*, while curated *behaviour* (featured lists, categories, ranking) remains allowed product choice. See `04-recommendations.md` item 4.

## Sources

- https://github.com/NixOS/nixpkgs/issues/15932 (RFC, 2016–2026)
- https://discourse.nixos.org/t/brainstorming-generating-appstream-metadata/19273
- https://discourse.nixos.org/t/why-doesnt-search-nixos-org-also-search-binary-names/16728
- https://github.com/NixOS/nixpkgs/pull/94055 (PackageKit Nix backend)
- https://github.com/NixOS/nixpkgs/pull/556838 (packages-info)
- https://github.com/NixOS/nixpkgs/pull/579597 (treewide metainfo)
