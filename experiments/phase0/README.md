# Phase 0 — Catalogue generation investigation

Research instrument for the catalogue-generation wager: can a useful application catalogue be generated from Nixpkgs and upstream metadata without building everything and without per-package curation? This is task `nix-pilot-nus.3`; the measurement plan and acceptance floors live in [verification.md](../../docs/design/verification.md), the pipeline design in [catalogue-pipeline.md](../../docs/design/specifications/catalogue-pipeline.md).

Status: scoped preliminary result on this machine. Research-stage: harness is deliberately small and ad hoc; the finding, not the code, is the deliverable.

## Pin

- Snapshot: the `nixpkgs` flake registered on this machine, imported impurely. narHash `sha256-qXFIqA6FCcx7XeDzw0ZNlkDT9UiCzrNDwK8RuU8RcHM=`, store path `/nix/store/cjndb9il5365hpqnn5k8gxp4w02xybsw-source`, recorded 2026-11-16.
- For a reproducible run later, pin a concrete `github:NixOS/nixpkgs/<commit>`; the current run is a local-snapshot measurement, not a pinned, reproducible CI run.

## Method

- `export.nix` walks top-level attributes of `legacyPackages.x86_64-linux` and emits one record per name with a JSON-safe metadata contract: `pname`, `version`, `description`, `homepage`, `mainProgram`, `licenseOk`, `platforms`, `broken`. Every field is forced to a primitive under `tryEval` and each entry is deep-forced so a single broken or pseudo package yields an `evalError` record instead of sinking the run. Nothing is built.
- `measure.sh [N]` evaluates the first N top-level attrs and times it; `analyze.js` summarises timing and field completeness. `results/` is generated output and not committed.

## Results

Full set (N = 27,271), this machine, the pinned snapshot:

| Metric | Value |
|---|---|
| Top-level attributes | 27,271 |
| Derivations | 24,658 (90.4%) |
| Non-derivations | 2,613 (9.6%) |
| Metadatum export wall-time | 30.2 s (~900 records/s) |
| Raw output size | ~21 MB (records.json) |
| Exported without eval error | 24,527 / 24,658 |
| `pname` present | 99.5 % |
| `version` present | 99.5 % |
| `description` present | 98.7 % |
| `homePage` present | 97.1 % |
| `platforms` present | 100 % |
| `license` resolvable | 98.7 % |
| `mainProgram` present | 64.1 % |

## What this proves

- **Enumeration + metadata export is cheap and complete.** Evaluating the whole top-level set takes ~30 s on this machine and yields near-complete pname/version/description/homepage/platforms/license. This is the half the research called *proven* (search.nixos.org does it); here it is measured, not assumed. The ~9.6 % non-derivations are aliases and nested sets, not GUI candidates.
- **Output-derived desktop data is present in available outputs.** This machine's already-installed store contains 12,036 `.desktop` files and `share/metainfo` directories, so discovery from already-available outputs is observable. Caveat: these come from the system's store at a different Nixpkgs revision, not the pinned set, so they do not yet establish coverage for the catalogue's own outputs.

## What remains (open sub-questions)

- **Coverage within a single pinned set**: how many GUI candidates in the pinned revision actually ship `.desktop`/metainfo. Requires inspecting that revision's outputs (build or binary-cache nar listing).
- **The join**: mapping a store output / desktop entry to a Nixpkgs package, and its precision/recall on a hand-labelled QA sample (hundreds of apps).
- **Unfree coverage** (`nix-pilot-9f3.4`): images/metadata for unfree packages via binary-cache inspection, since Hydra does not build them.
- **First searchable catalogue** over the proven metadata, with documented coverage and omissions.

## Run

```sh
./phase0/measure.sh 300     # scoped sample
./phase0/measure.sh 27271   # full top-level set
```

A reproducible full run should pin a concrete Nixpkgs commit and run on real infra (CI), not a local snapshot.