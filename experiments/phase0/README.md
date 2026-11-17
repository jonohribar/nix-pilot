# Phase 0 — Catalogue generation investigation

Research instrument for the catalogue-generation wager: can a useful application catalogue be generated from Nixpkgs and upstream metadata without building everything and without per-package curation? This is task `nix-pilot-nus.3`; the measurement plan and acceptance floors live in [verification.md](../../docs/design/verification.md), the pipeline design in [catalogue-pipeline.md](../../docs/design/specifications/catalogue-pipeline.md).

Status: scoped preliminary result on this machine: enumeration proven, output-discovery and the Exec-level join look strong on a hand sample, unfree coverage confirmed open. Research-stage: harness is deliberately small and ad hoc; the finding, not the code, is the deliverable.

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

This is an automated proxy for the QA sample (link recall in the common-name → attribute-path direction); the hand-labelled precision/recall sample remains the open step.

## Unfree outputs on public caches (`nix-pilot-9f3.4` baseline)

With `allowUnfree = true` the unfree packages evaluate and their metadata is fully present in the exported set — the default gate, not the set, is what hides them from the catalogue.

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

## What this proves

- **Enumeration + metadata export is cheap and complete.** ~30 s for the full top-level set, near-complete pname/version/description/homepage/platforms/license.
- **Output-derived discovery works.** Desktop entries and metainfo are present in the pinned set's cache-served outputs, and the Exec-level join is near-total on the sample (all unmatched cases are packaged helpers of a matched parent).
- **The unfree gap is real and unchanged** (gate + no cache coverage), matching the research's expectation.

## What remains (open sub-questions)

- **Coverage across the whole set, not a hand sample**: what share of all 24,658 derivations ship desktop entries/metainfo, and what does that imply for GUI-candidate recall.
- **Join precision/recall at scale** on a hand-labelled QA sample (hundreds of apps), including ambiguous and cross-package cases.
- **Full-set GUI-coverage scan**: what share of all 24,658 derivations ship desktop entries/metainfo. Needs fetching or building thousands of outputs; the natural next Phase-0 stage on real infra, not a quick probe.
- **Hand-labelled QA sample**: precision/recall scored by manual review of hundreds of apps (QA, not curation) per the measurement plan.
- **Unfree images/metadata path** (`nix-pilot-9f3.4`): baseline recorded above (404 on official and nix-community caches); the alternate route is still open.

## Run

```sh
./phase0/measure.sh 300     # scoped sample
./phase0/measure.sh 27271   # full top-level set
```

A reproducible full run should pin a concrete Nixpkgs commit and run on real infra (CI), not a local snapshot.