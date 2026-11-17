# Phase 0 — Catalogue generation investigation

Research instrument for the catalogue-generation wager: can a useful application catalogue be generated from Nixpkgs and upstream metadata without building everything and without per-package curation? This is task `nix-pilot-nus.3`; the measurement plan and acceptance floors live in [verification.md](../../docs/design/verification.md), the pipeline design in [catalogue-pipeline.md](../../docs/design/specifications/catalogue-pipeline.md).

Status: scoped preliminary result on this machine: enumeration proven, output-discovery and the Exec-level join look strong on a hand sample, unfree coverage confirmed open. Research-stage: harness is deliberately small and ad hoc; the finding, not the code, is the deliverable.

The canonical findings record lives in [docs/research/05-phase0-findings.md](../../docs/research/05-phase0-findings.md); this README keeps the method, run details and caveats.

## Pin

- Snapshot: the `nixpkgs` flake registered on this machine, imported impurely. narHash `sha256-qXFIqA6FCcx7XeDzw0ZNlkDT9UiCzrNDwK8RuU8RcHM=`, store path `/nix/store/cjndb9il5365hpqnn5k8gxp4w02xybsw-source`, recorded 2026-11-16.
- For a reproducible run later, pin a concrete `github:NixOS/nixpkgs/<commit>`; the current run is a local-snapshot measurement, not a pinned, reproducible CI run.

## Method

- `export.nix` walks top-level attributes of `legacyPackages.x86_64-linux` and emits one record per name with a JSON-safe metadata contract: `pname`, `version`, `description`, `homepage`, `mainProgram`, `licenseOk`, `platforms`, `broken`. Every field is forced to a primitive under `tryEval` and each entry is deep-forced so a single broken or pseudo package yields an `evalError` record instead of sinking the run. Nothing is built.
- `measure.sh [N]` evaluates the first N top-level attrs and times it; `analyze.js` summarises timing and field completeness. `discover.sh` fetches candidate outputs from the binary cache and scans them for `.desktop`/metainfo. `join.js` scores Exec-basename joins against attribute paths and pnames. `results/` is generated output and not committed.

## Results

Full set (N = 27,271), this machine, the pinned snapshot:

| Metric | Value |
|---|---|
| Top-level attributes | 27,271 |
| Derivations | 24,658 (90.4%) |
| Non-derivations (aliases, sets) | 809 (3.0%) |
| Evaluation-failing attrs | 1,804 (6.6%) |
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

The 1,804 evaluation-failing attributes (6.6%, e.g. `AAAAAASomeThingsFailToEvaluate`, which is deliberately self-describing) are the "skipped/evaluation-failing entries" metric from [verification.md](../../docs/design/verification.md); they are excluded from the catalogue.

## Discovery probe (provisional)

`discover.sh` fetches a candidate's outputs from the binary cache (no local build) and scans them. Sample of 16 (13 GUI candidates + 3 CLI controls); the pinned snapshot's derivations hash-match the official cache, so fetches succeed instantly:

| Candidate | Class | `.desktop` | AppStream metainfo |
|---|---|---|---|
| feh, kitty, rofi, xterm, htop, pcmanfm | image/launcher/terminal/file/TUI utilities | 1–2 each | 0 |
| mpv, meld, gedit, evince, gnome-calculator, baobab, eog | media + GNOME apps | 1–2 each | 1 each (evince: 8) |
| jq, curl, unzip | CLI controls | 0 | 0 |

13/13 GUI candidates shipped at least one `.desktop` entry; 7/13 also shipped AppStream metainfo (`org.gnome.*.metainfo.xml` etc.); 3/3 CLI controls correctly had none. `konsole`/`transmission` were dropped from the sample (attribute removed in this snapshot, not a discovery result).

## Join (provisional)

`join.js` takes the `.desktop` files discovered above and matches their `Exec` basename against the pinned set's top-level attribute paths and pnames:

- 18 desktop files found; 15/18 `Exec` names match a top-level attribute path exactly (feh, mpv, kitty, rofi, xterm, meld, pcmanfm, htop, gedit, evince, …).
- The remaining 3 (umpv, rofi-theme-selector, evince-previewer) are bundled helper binaries of an already-matched parent package — recoverable via the containing-output signal.
- Because discovery scans a package's own output, the containing output *is* the join; the Exec-name match is a corroborating signal, not the primary one. This supports the confidence-tiered design (D13).

## Known-apps recall

`recall.js` resolves 79 well-known GUI applications (browsers, editors, terminals, file managers, media, office, messaging, system utilities) against the exported metadata:

- 76/79 resolve to a top-level attribute; all 79 resolve once nested sets count (KDE apps live under `kdePackages.*`; Tor Browser is `tor-browser`). The 3 apparent misses were attribute-path guessing, not real gaps.
- Of resolvable apps, 100% carry a description; 73/76 are buildable on Linux.

This is an automated proxy for the QA sample; the human-reviewed sample with scores is below.

## QA sample v1 results

[qa/sample-v1.md](qa/sample-v1.md) is a 117-entry sample (known GUI apps, unfree apps, absent-app decoys, and lib/CLI decoys for precision). It was pre-scored by an independent labeller subagent, corrected for seed misses (Dropbox, notion-app, figma-linux), and approved by the project owner (skip = agree). `qa/score.js` computes:

| Metric | Value |
|---|---|
| Known apps in sample | 86 |
| Recall (apps correctly found) | 94.2 % |
| Precision (of rows presented as apps) | 98.8 % |
| Precision if CLIs/libs were naively presented as apps | 77.1 % |
| The one WRONG match | `notion` — a tiling window manager, not the notes app (precision trap) |
| MISSING | Kdenlive, Spectacle, Cursor, Docker Desktop — not installable from this set, so never listed (products the set cannot install do not appear) |

Reading: the naive name→attribute join is strong on recall (94 %) and nearly perfect on precision (98.8 %) *once classification stops libs/CLIs from being presented as app tiles* — without that guard precision would drop to 77 %. This is the empirical case for the classification tiering (D13) and for the CLI-labelled result-class proposal (`nix-pilot-9f3.7`).

## Classification probe (description heuristics)

`classify.js` tests whether description-keyword + mainProgram heuristics can satisfy "the generator distinguishes ordinary desktop-app candidates from libraries and CLI tools with measurable confidence" (initial acceptance criteria). Against the QA labels (104 rows):

| Metric | Value |
|---|---|
| Accuracy | 86.9 % |
| App precision | 86.2 % |
| App recall | 98.7 % |
| Typical false-APP | htop, btop, ripgrep, fd, bat, tmux, zsh, git, ffmpeg |

Finding: description heuristics find real apps (near-total recall) but cannot reliably reject CLIs — meaning alone is not enough. The plan's sharper signal is output-derived desktop-entry evidence, which the (deferred) coverage scan will supply; note even desktop presence has edge cases (htop ships a `.desktop`), supporting the CLI-labelled result class.

## Unfree outputs on public caches (`nix-pilot-9f3.4` baseline)

Unfree packages (`discord`, `spotify`, `slack`, `vscode`, `google-chrome`) are fully present in the default export with complete metadata — the gate refuses instantiating/building them, not enumerating them, so the catalogue can list them by default (D1) with the metadata evaluation provides.

Their outputs, however, are absent from public caches at this snapshot, checked by store-path hash:

| App | cache.nixos.org | nix-community.cachix.org |
|---|---|---|
| vscode | 404 | (not checked) |
| spotify | 404 | 404 |
| slack | 404 | 404 |

So unfree metadata is cheaply cataloguable, but output-derived enrichment (icons, desktop entries, metainfo) and binary substitution for unfree apps require a route that does not exist on the official or nix-community caches today (`nix-pilot-9f3.4`).

## First searchable catalogue

`search.js` indexes the full-set records and ranks queries against attribute path, pname and description. `node search.js --stats`:

| Metric | Value |
|---|---|
| Catalogue documents | 24,658 |
| Installable on Linux | 23,107 |
| With description | 24,340 (98.7%) |
| With licence | 24,333 (98.7%) |
| With homepage | 23,945 (97.1%) |
| With mainProgram | 15,801 (64.1%) |

Sample queries return sensible, ranked application results ("browser" → browser-sync/browserpass/browsers; "editor" → 010editor/anvil-editor/…). This is a first, flat index over all derivations; GUI-vs-non-app classification, categories and the consumer ranking are later stages.

Documented limitations: mainProgram coverage is 64%; description quality varies; the index is attribute-name/pname/description text only; no icons or screenshots yet.

## Statistical coverage sample

`sample-coverage.js` draws a random sample of derivations, dry-runs each (skipping anything that would build locally or exceed the 150 MiB cap), fetches the rest from the cache, and scans for `.desktop`/metainfo. Resumable: partial results persist in `results/coverage-sample.jsonl` and fetch-failed items are re-queued on rerun.

Result (this machine, stable network, 293 fetched of 429 sampled): **GUI-app rate 13.0 % ± 3.8 %** (95 % CI) — 38 of 293 random derivations ship a desktop entry — extrapolating to **≈ 3,200 GUI application candidates** across the ~24,600-derivation set. The rate held stable across two independent chunks (13.2 % then 13.0 %). Metainfo rate 4.0 %.

Caveats: desktop-entry presence counts window managers (`berry`, `muffin`) as GUI; the 119 unfetched items are mostly packages needing local builds (wrapped/patched), not network failures, and no correlation with GUI-ness was observed.

This is the store's app-universe size expectation and the empirical case for classification: ~87 % of derivations are libraries/CLIs that must be tiered out of ordinary browsing.

## Generator v1 — classified, versioned index

`generator.js` turns the full-set export into a versioned catalogue index (`results/catalogue-index.json`, schema 0.1) with the classification table's classes: application candidates, command-line tools (decision [D16](../../docs/research/04-recommendations.md#decision-log)) and excluded libraries/system components.

Calibration: description/mainProgram heuristics alone over-flood (54 % apps at a low threshold). Requiring app−cli ≥ 3 reproduces the measured coverage base rate almost exactly — 3,181 app candidates = 12.9 % vs the 13.0 % ± 3.8 % coverage-survey estimate. The count-calibrated universe is the defensible part; confidence in v1 is high but undifferentiated (a calibrated binary, not a tier), so rich/basic/uncertain differentiation still needs desktop-entry evidence (the coverage-pipeline upgrade path).

Index contents: 3,181 apps (100 % with description by construction; 96 % Linux-installable; 95 % with mainProgram), 18,849 command-line tools, 2,628 excluded libraries.

`search.js` queries the index when present and labels results by class — implementing D16: a search for `docker` returns the *command-line tool* group, never an app tile. This is the Phase 0 acceptance probe ("search one term, relevant results") over the classified app universe.

## What this proves

- **Enumeration + metadata export is cheap and complete.** ~30 s for the full top-level set, near-complete pname/version/description/homepage/platforms/license.
- **Output-derived discovery works.** Desktop entries and metainfo are present in the pinned set's cache-served outputs, and the Exec-level join is near-total on the sample (all unmatched cases are packaged helpers of a matched parent).
- **The unfree gap is real and unchanged** (gate + no cache coverage), matching the research's expectation.

## What remains (open sub-questions)

- **Coverage across the whole set, not a hand sample**: what share of all 24,658 derivations ship desktop entries/metainfo, and what does that imply for GUI-candidate recall.
- **Join precision/recall at scale** on a hand-labelled QA sample (hundreds of apps), including ambiguous and cross-package cases.
- **GUI-coverage estimate**: what share of all derivations ship desktop entries/metainfo. Needs stable network / real infra (this machine's cache connection is flaky; harness `sample-coverage.js` is ready and resumable).
- **Hand-labelled QA sample**: [qa/sample-v1.md](qa/sample-v1.md) is generated and pre-scored, ready for human review (QA, not curation) per the measurement plan.
- **Unfree images/metadata path** (`nix-pilot-9f3.4`): baseline recorded above (404 on official and nix-community caches); the alternate route is still open.

## Run

```sh
./phase0/measure.sh 300     # scoped sample
./phase0/measure.sh 27271   # full top-level set
```

A reproducible full run should pin a concrete Nixpkgs commit and run on real infra (CI), not a local snapshot.