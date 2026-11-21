# Recommendations and Open Decisions

> Research note · 2026-11-16 · Status: current Recommendations derived from the analysis in `03-failure-modes-and-risks.md`, with a decision log at the end that tracks each open question to a named owner and decision date.

## In one paragraph

The brief is sound on scope and ordering; these recommendations close the gaps it leaves: a failure-and-recovery spec written before any UI work, a risk-table entry for the settings surface, an explicit failure-handling acceptance criterion, a precise statement of the no-curation rule, pinned Phase-0 inputs (especially the unfree-package policy), and a sustainability commitment. Most are one-page additions; none require changing the phased plan.

## Recommendations

### 1. Surface prior art in the project docs

Summarise `01-prior-art.md` in 10 lines: what was tried, why each project died, and the two patterns to avoid. Future readers will otherwise re-derive this at cost — the docs should make the prior-art record visible rather than read as if the space were empty.

### 2. Write the failure-and-recovery spec before Phase 2 (highest priority)

One page answering, for a non-technical user:

- What shows on screen when a rebuild fails (plain-language summary of the error, the exact raw error one click away, offered recovery actions).
- What happens when the system does not boot (boot-time older-generation selection; documentation shipped *before* the failure, e.g. a recovery card in Settings).
- How rollback is explained without Nix vocabulary ("go back to yesterday's system") and how the app recovers when a applied generation partially changed the system.
- The explicit guarantee: a failed build must never leave the system partially applied (the brief already promised this; the spec defines how it is tested).

### 3. Tighten the Phase 1 failure-handling acceptance criterion

"Confirm the GUI can explain each operation without exposing Nix concepts" is too weak. Replace with: "a user who makes a change that fails to build or apply can recover to a working system without opening a terminal, and the UI showed the error and its remedy in plain language."

### 4. State the no-curation rule precisely

Current wording invites a de facto allowlist through "safe, explicit product rules". Proposed statement:

- Forbidden: hand-authored per-package catalogue entries, screenshots, descriptions, or app-to-package mappings.
- Allowed and product-owned: curated *behaviour* — featured lists, categories, ranking policy, settings workflows — provided it is explainable and not required for a package to be installable.

### 5. Add the settings surface to the risk table

Nixpkgs cannot generate a consumer settings interface (a settled point), which means the settings workflows are a product-authored, unbounded surface that tracks Nixpkgs option renames forever. Give it a risk row with the same weight as the catalogue, and a mitigation: a versioned settings schema with a documented migration path (this also protects users from option renames between Nixpkgs revisions).

### 6. Pin the Phase-0 inputs and policy before starting

Three decisions from `02-catalogue-feasibility.md` that change the data model, so they cannot be deferred into the build:

- Unfree packages: indexed or not? If yes, metadata source (they are not on Hydra) and redistribution policy for icons/screenshots.
- Output inspection: Hydra-built outputs only, or arbitrary binary-cache inspection (storage/bandwidth engineering either way).
- Coverage targets: precision/recall floors for "GUI application found from Nixpkgs", set before any UI is built.

### 7. Budget for the negative Phase-0 result

Phase 0 is a 6–18 month research project with a real chance of "coverage is low and rising slowly". Write down in advance what that finding does to scope (e.g. narrower app universe for v1, closer coupling to the 2025–2026 nixpkgs indexing work — PR #556838, #579597) rather than loosening the curation rule.

### 8. State the sustainability commitment

One line: "this project must be maintainable by a 1–2 person volunteer team for five years". Apply it as a filter to every Phase-2 design decision — settings schema versioning, catalogue format stability, UI framework choice. Predecessors failed this test; naming it does not solve it, but it stops decisions being made without it.

## Decision log

Every decision is recorded here once made; decided rows are never deleted. Open decisions are tracked in the plan (`../design/plan.md`) so they are not lost. As of 2026-11-16, all initial decisions D1–D7 are resolved; the commit-workflow decision is D8.

| # | Decision | Outcome | Owner | Date | Status |
|---|---|---|---|---|---|
| D1 | Unfree-package policy for the catalogue | **Include unfree applications by default** — the average user expects Chrome and Discord. At most an optional user filter to hide unfree apps (implemented as an opt-in filter). Metadata and images for unfree apps come from binary-cache inspection, tracked as a follow-up evaluation. User-facing wording avoids packaging jargon: the filter label should read like "Hide packages with licence restrictions", matching the plain-language unfree-block guidance (REQ-6 register, 2026-11-20; wording updated 2026-11-21 with the Packages naming, D29) | Project owner | 2026-11-16 | Decided |
| D2 | Output inspection source | **Hydra first** (zero cost, covers most free GUI apps). Binary-cache inspection evaluated later, because Hydra cannot provide unfree packages or their images | Project owner | 2026-11-16 | Decided |
| D3 | Coverage targets for classification | **Set provisional precision/recall targets before Phase 0 measurement**, reviewed after the QA sample; exact floors defined in the Phase 0 measurement plan (`../design/verification.md`) | Project owner | 2026-11-16 | Decided |
| D4 | No-curation rule wording | **No hand-maintained per-package content.** Curated *behaviour* (featured lists, categories, ranking policy) remains allowed product choice, provided it is explainable and never required for an app to be installable | Project owner | 2026-11-16 | Decided |
| D5 | Failure-and-recovery spec | **Accepted as a Phase 2 prerequisite**; specified in `../design/specifications/failure-and-recovery.md` | Project owner | 2026-11-16 | Decided |
| D6 | Settings schema versioning | **Versioned schema with documented migration** | Project owner | 2026-11-16 | Decided |
| D7 | Repo/community home for sustained maintenance | **Local repo for now.** If the project proves out it becomes an open-source project (nix-community considered at that point). Internal docs may rely on AI-assisted tooling (e.g. beads); before any open-source release they are converted to developer-friendly, tool-independent format | Project owner | 2026-11-16 | Decided |
| D8 | Home of the commit-workflow rule | **Root AGENTS.md, "Commits" section.** AGENTS.md is the short rule set, always loaded by the tooling at work time; beads and plan.md were rejected as homes because neither is read at commit time. At open-source release the rule moves to CONTRIBUTING.md as part of the AGENTS.md rewrite (D7) | Project owner | 2026-11-16 | Decided |
| D9 | Install mechanism for generated packages | **Declarative, single owner: the app's generated NixOS config.** Each install carries a scope, chosen at install time: per-user by default, generated into `users.users.<name>.packages`; system-wide only when explicitly chosen, generated into `environment.systemPackages`. No imperative per-user profiles. One system generation per change, uniform rollback through the existing generation + boot-menu recovery | Project owner | 2026-11-16 | Decided |
| D10 | Catalogue scope for first release | **GUI/installable applications only.** The store lists applications the average user can install and run; it does not attempt to classify the whole nixpkgs package set | Project owner | 2026-11-16 | Decided |
| D11 | Catalogue artifact architecture handling | **One universal index with per-entry architecture availability flags.** A single artifact per catalogue version; clients filter by which architectures the entry can build for. Supports the "unavailable on your system" state without shipping one index per architecture | Project owner | 2026-11-16 | Decided |
| D12 | Catalogue regeneration cadence | **Weekly by default** | Project owner | 2026-11-16 | Decided |
| D13 | App identity and join strategy | **Multi-signal tiered match** (package name, desktop-entry ID, binary name), showing only high-confidence joins and carrying a confidence tier per entry | Project owner | 2026-11-16 | Decided |
| D14 | Settings surface boundary | **System-only settings plus pointers.** nix-pilot manages Nix-owned system settings that map to a stable NixOS option with the same meaning across desktops and ownable versioning/migration; all other preferences (desktop chrome, per-application settings) are deferred by linking to the tool that owns them, never reimplemented | Project owner | 2026-11-16 | Decided |
| D15 | Phase 0 acceptance floors | **Provisional floors set now, reviewed after the QA sample.** The exact values are defined in the Phase 0 measurement plan (`../design/verification.md`); they are provisional and revisited when a hand-labelled QA sample exists | Project owner | 2026-11-16 | Decided |
| D16 | CLI tools in search results | **Labelled, never tiled.** Command-line tools are indexed and appear in search as a labelled "Command-line tool" group alongside apps, settings and help; they are never presented as application tiles. Libraries and system components remain excluded. Provisional: revisit after the generator implements it | Project owner | 2026-11-17 | Decided |
| D17 | Unfree app enrichment and install route | **Interim: list with evaluation metadata, install by local build.** Unfree apps are listed with the metadata evaluation provides (name, description); icon/screenshot enrichment is deferred; installing builds locally with honest progress. Community-cache and analytics-fed alternatives are the future upgrade path. Provisional: revisit after implementation | Project owner | 2026-11-17 | Decided |
| D18 | Presentation of unconfirmed app candidates | **A distinct, labelled "Unverified application" group**, never shown as application tiles. Candidates the evidence pass could not confirm (uncertain/no desktop/metainfo evidence, too-large, unevidenced) stay findable in search under an explicit "unverified" label; they are **not collapsed into the command-line group**, because a minority are genuine apps the cache could not evidence. Real apps the heuristic wrongly labels "command-line tool" are recovered by a targeted evidence pass (batch) | Project owner | 2026-11-18 | Decided |
| D19 | Desktop client language and UI toolkit | **Rust with GTK4 + libadwaita (gtk4-rs).** Native GNOME look and behaviour for a Settings + Software Store app, deep integration with the NixOS desktop (themes, accessibility, HiDPI), and the largest mature widget ecosystem — the lowest-risk 5-year maintainability choice (decision D7). Alternatives evaluated: Slint (better UX velocity, but no HIG/native look and a younger ecosystem), Tauri (webview weight + two-codebase maintainability), iced/egui (right paradigm, but limited for rich consumer UI), Qt (licensing/weight) | Project owner | 2026-11-18 | Decided |
| D20 | Generated NixOS configuration shape (apply backend v1) | **The app owns the outer generated `configuration.nix`** in an explicit generation directory: it imports the machine's `hardware-configuration.nix` and the pre-existing `configuration.nix` (nothing the user has is dropped), and imports the app's desired-state module verbatim. Build-only in v1 — the backend only ever *builds* the candidate generation (`nixos-rebuild build`), never switches or activates. Baseline options (boot, network, …) move into the settings surface later as owned setting workflows | Project owner | 2026-11-19 | Decided |
| D21 | Ownership of the pre-existing configuration (vs a copy) | **Live pointer import, never a copy.** The outer generated config imports `/etc/nixos/configuration.nix` by absolute path, so hand-edits to it are picked up on every build and the app never writes it; `system.stateVersion` is then owned by that file and the generated outer only emits it in the no-user-config (fresh) case. Rejected: copying a snapshot (drift if the user hand-edits — the app would need refresh prompts) and evaluating-and-re-emitting the config (a frozen, unmaintainable option dump). Verified by a live pointer build | Project owner | 2026-11-19 | Decided |
| D22 | Per-user install backend (home-manager vs NixOS users) | **Two backends.** The render supports home-manager (`home-manager.users.<name>.home.packages`) for systems whose config tree enables the home-manager NixOS module — the natural case is the app module added inside a home-manager-enabled flake. The choice is never auto-detected from the OS user's state (a user can run home-manager standalone while the built config does not enable its module, which makes the option undefined and breaks the build — hit live); explicit for now (`NIX_PILOT_USER_BACKEND=home-manager`). Per-user installs on a home-manager system roll back as home-manager generations (lighter, no boot-menu stakes); system changes remain one system generation with boot-menu recovery | Project owner | 2026-11-19 | Decided |
| D23 | Per-user install default (supersedes D22's default) | **home-manager is the product-default per-user install method** whenever the target config enables the home-manager module; NixOS-native `users.users.<name>.packages` (D9) is the fallback when it does not (an emitted home-manager option against a non-home-manager config is an undefined-option build error, verified live). Rationale: flakes/home-manager are the direction NixOS is heading (prior art: wrappers that did not follow eventually died), and per-user installs on home-manager are lighter and safer (home generations, no boot stakes). In the flake integration the module follows the user's nixpkgs input, so the default is sound; in the standard pointer path the choice is confirmed by the prepare dry-build (exact undefined-option -> fallback) before it stands. Implementation lands with the flake-module integration and prepare probe; the code default stays `users.users` until then | Project owner | 2026-11-19 | Decided |
| D24 | Activation elevation mechanism | **polkit `pkexec` with a packaged action and rule.** Activation runs `pkexec nix-pilot-apply <fixed subcommand>`; the desktop's normal authentication dialog is the gate, the polkit rule authorizes exactly the `org.nix-pilot.apply` action for wheel users, and the helper is argument-free (build/switch/rollback) with the config path pinned. Rejected: a sudo whitelist (clunkier GUI auth, broader grant) and a setuid helper (root without a prompt). Publish-before-switch (D26) extends the helper's interface to receive the config in-band; the action and rule are unchanged | Project owner | 2026-11-19 | Decided |
| D25 | Activation failure recovery | **Sanctioned go-back, no silent auto-rollback.** After a failed apply the app states honestly that the change did not complete (never asserts "system unchanged" without verification) and offers a plain-language "Go back to the previous system" control running `pkexec nix-pilot-apply rollback`, behind a one-dialog confirmation (REQ-3). The boot-menu recovery card remains the boot-level fallback. Rejected: silent automatic rollback (masks the failure cause) and leaving no recovery path | Project owner | 2026-11-19 | Decided |
| D26 | Publish-before-switch mechanism | **In-band stdin handoff.** Apply re-renders the generated configuration from the persisted desired-state doc and feeds it to `pkexec nix-pilot-apply switch` via stdin; the helper writes it atomically (temp + rename) into the app-owned area and only then switches — one transaction, no stale-config window. The renderer must be location-independent (byte-identical in a scratch dir and in the area), enforced by a unit test. Rejected: write-then-hash-verify (check-then-switch window, elevation on every prepare) and single-source-area (elevation before any dry-run) | Project owner | 2026-11-19 | Decided |
| D27 | Package version choice (per-app override) | **Per-app channel override (option B).** At install the user may pick stable (default, current version highlighted) or bleeding-edge for a single app; the app resolves that app against a per-app channel (nixpkgs unstable). This is a deliberate, scoped exception to the "never impose a pin" reading of D23: the app may impose its own nixpkgs pin *for one named app*, never for the whole system. Free-form custom version strings are out (nixpkgs has no per-package version matrix). Gated on per-channel catalogue infra (7wo.4) to know whether a newer version exists | Project owner | 2026-11-19 | Decided |
| D28 | Search placement + navigation model | **Global search in the sidebar top; store search inside Packages; Updates is a top-level surface only.** The app-wide search box lives at the top of the sidebar (not the window HeaderBar), searching packages + settings in v1 (help joins when it exists); it always searches the same scope regardless of surface, avoiding the KDE Discover confusion where a search silently re-scopes. Curated per-surface searches stay where they belong: the store's scoped search + non-free filter inside the Packages tab (GNOME Software / Discover convention). Updates single-tops: it is a sidebar surface only — the Settings home's "Updates" category row and its "Open Updates" cross-link were removed (upkeep settings still render on the Updates page). Grounded in industry research: Windows 11 / macOS put settings search at the sidebar top; store search lives in the store page | Project owner | 2026-11-20 | Decided |
| D29 | Store surface naming | **The catalogue is called "Packages" to users.** The store tab label and its user-facing strings say "packages" (sidebar row, store search placeholder, browse summary, licence filter, empty-state text, global-search scope wording), because the catalogue carries applications, terminal apps and command-line tools — "apps" under-covers it (verified: no library class in the index, app/term-app/cli only). Per-entry class labels (Application / Terminal app / Command-line tool) and the D16/D18 search group headers stay, since they are evidence classes within the catalogue | Project owner | 2026-11-21 | Decided |
| D30 | Settings search recall | **Every settings spec carries an authored static `tags` list feeding the global search.** Specs gain a const `tags: &'static [&'static str]` of consumer synonyms ("wifi" for Network connections, "rename" for Device name), indexed by the same scorer as label/key/blurb so natural-language settings queries hit the right row | Project owner | 2026-11-21 | Decided |

## Cross-references

- `00-README.md` — index and maintenance guidance
- `01-prior-art.md` — the record of previous attempts
- `02-catalogue-feasibility.md` — Phase 0 evidence and open questions
- `03-failure-modes-and-risks.md` — the analysis backing these recommendations
