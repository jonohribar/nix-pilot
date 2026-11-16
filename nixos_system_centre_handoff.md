# NixOS Consumer System Centre — Project Handoff

> This document is being migrated into `docs/`. Each extracted section is deleted here so every fact lives in one place (see `docs/design/plan.md`). Sections 1, 2, 3, 5, 6 and 9 are already removed: translated into `docs/requirements/user-needs.md` and `docs/requirements/requirements.md`, merged into `docs/design/risks.md`, or summarised in `docs/design/plan.md` / `README.md`. Remaining: sections 4, 7, 8 and 10–12.

**Status:** Concept / pre-implementation **Date:** 16 November 2026 **Working title:** NixOS System Centre (provisional)

## 4. Automatically generated software catalogue

### 4.1 Core principle

Treat Nixpkgs as the authoritative source for what can be installed, but do not treat each Nixpkgs derivation as a consumer-facing application. Generate a separate application catalogue by combining Nixpkgs package data with application metadata supplied by upstream projects and discovered from package outputs.

### 4.2 Information sources

**Nixpkgs package metadata** can provide fields such as:

- Package attribute/path and package identity.
- `pname` and `version` (derivation/package attributes, not necessarily `meta` fields).
- `meta.description` and `meta.longDescription`.
- `meta.homepage`, `meta.license`, `meta.maintainers`.
- `meta.platforms`, `meta.badPlatforms`, `meta.broken`.
- `meta.mainProgram` and, where present, software identifiers and source provenance.

These fields are useful for package search, filtering, compatibility and attribution. Availability and completeness vary by package and Nixpkgs revision.

**AppStream metadata** is designed for software-centre presentation. It can supply application IDs, names, summaries, descriptions, categories, icons, screenshots, developer information and release metadata. Upstream projects may ship metainfo files for this purpose.

**Freedesktop desktop-entry files (`.desktop`)** provide evidence that an output exposes a launchable desktop application, including display name, executable, icon and categories. AppStream generators can use existing desktop-entry files as a metadata source; upstream metainfo provides richer presentation data.

### 4.3 Proposed generation pipeline

1. **Pin the input:** select a Nixpkgs revision/channel and target system architecture(s).
2. **Evaluate package metadata:** enumerate package-set entries and export safe, serialisable metadata without building every package.
3. **Filter package candidates:** exclude broken or incompatible packages from normal installation results; retain reasons for exclusion where useful.
4. **Discover graphical applications:** inspect AppStream metainfo and desktop-entry files in package outputs where those outputs are available.
5. **Enrich records:** join application metadata to Nixpkgs package identities using evidence such as AppStream package hints, desktop-entry executable names, `mainProgram`, package output paths and upstream identifiers.
6. **Assign confidence:** record how strongly an application component maps to a Nixpkgs package. Avoid presenting ambiguous matches as certain.
7. **Index and publish:** generate a versioned catalogue for each supported Nixpkgs revision and architecture; distribute it as a static, cacheable index.
8. **Refresh:** regenerate when the tracked Nixpkgs revision or relevant metadata changes.

This is a staged pipeline, not a requirement to build the entire Nixpkgs collection. Metadata evaluation is inexpensive relative to building packages. Output inspection should be limited to candidates and should reuse existing local or binary-cache outputs when possible.

### 4.4 Important feasibility qualification

Nixpkgs does not guarantee that every package has AppStream metadata, a desktop entry, a screenshot, a standard category or a consistent application identifier. Some graphical applications will have rich upstream data; others will only have package metadata or a desktop entry; some will be difficult to classify automatically.

Therefore:

- The catalogue can be generated without manual package curation.
- Catalogue completeness and presentation quality will vary by upstream metadata quality.
- A missing screenshot or description must not prevent installation.
- Uncertain application-to-package matches must be handled conservatively.
- The system must not silently fall back to a manually maintained allowlist or mapping table.

A useful classification policy is:

| Class | Evidence | Default presentation |
|---|---|---|
| Rich desktop application | AppStream component associated with a package | Normal Store listing with available rich metadata |
| Basic desktop application | Valid `.desktop` entry associated with a package | Normal Store listing with generated/basic details |
| Uncertain GUI candidate | Heuristic evidence only | Lower-confidence or secondary results; no false certainty |
| CLI tool, library or system component | No desktop-app evidence | Exclude from ordinary app browsing; optionally expose through advanced search |

This classification is generated from evidence and rules, not a manually curated package catalogue.

## 7. Proposed architecture

### User interface

One desktop application with Settings, Software, Updates/Recovery and global search. Normal users see outcome-oriented language and visual controls, not Nix internals.

### Product model

A structured, application-owned representation of desired system state:

- Installed applications and selected versions/variants where applicable.
- User-facing system preferences.
- Enabled system capabilities/services.
- System generation and update/recovery status.

### NixOS integration backend

The backend translates the product model into generated NixOS configuration/modules, evaluates it, builds a new system generation, presents a human-readable summary and applies it. Nix remains the implementation engine.

### Catalogue service/build pipeline

An independent generator consumes a pinned Nixpkgs revision and upstream/package metadata, produces architecture-specific catalogue indexes, and publishes versioned artifacts. The desktop application consumes those artifacts and can maintain a local searchable cache.

### Developer mode (optional)

May expose raw Nix configuration, option provenance, module diagnostics and evaluation details. It is hidden or disabled by default and is not required for successful ordinary use.

## 8. Installation and system-state model

For the initial release, the application should own the configuration it generates. A structured desired-state model should be the source of truth; NixOS configuration is generated output.

Package installation should update the desired package set and deploy a new system generation. The implementation must choose and document the exact installation mechanism (for example, generated `environment.systemPackages` entries versus a user-profile strategy). Do not mix independent installation mechanisms without a clear ownership model.

The application should show a plain-language transaction summary before applying system changes, then report build/apply progress and provide rollback/recovery controls. A failed build must not leave the system in a partially applied state.

## 10. Recommended next steps

### Phase 0 — prove catalogue generation

1. Choose one pinned Nixpkgs revision and one initial target architecture (for example, `x86_64-linux`).
2. Build a small exporter that enumerates package entries and emits JSON containing package identity, version, descriptions, homepage, licence, maintainers, platform compatibility, broken status and main program where available.
3. Measure evaluation time, output size, number of skipped/evaluation-failing entries and metadata completeness.
4. Test discovery of AppStream metainfo and `.desktop` files from already available package outputs and binary-cache outputs.
5. Create automated join logic and confidence scores; do not add hand-maintained package mappings.
6. Evaluate a sample of well-known desktop applications to measure classification and mapping quality. Manual review here is QA, not catalogue curation.
7. Produce a first searchable static catalogue and document its coverage and omissions.

### Phase 1 — prove the system backend

1. Define the structured desired-state schema.
2. Generate a minimal NixOS configuration from that schema.
3. Install/remove one package through the model.
4. Enable/disable one service-backed capability through a user-facing setting.
5. Evaluate, build, apply and roll back a generation.
6. Confirm the GUI can explain each operation without exposing Nix concepts.

### Phase 2 — build the consumer experience

Implement the integrated Settings, Software, Updates and global-search UX against the proven catalogue and backend. Keep developer diagnostics separate.

## 11. Initial acceptance criteria

The first technical proof should demonstrate:

- Package metadata is exported automatically from a pinned Nixpkgs revision.
- No per-package hand-written catalogue entries or mappings are required.
- The generator distinguishes ordinary desktop-app candidates from libraries and CLI tools with measurable confidence.
- AppStream and desktop-entry data enrich listings when available, but are not prerequisites for package installation.
- A user can search one term and receive both relevant application and settings results.
- Installing/removing an application changes the application-owned desired state and deploys through a NixOS generation.
- The normal UX does not display Nix expressions, option paths or module internals.

## 12. Reference material

- [Nixpkgs Reference Manual — Meta-attributes](https://nixos.org/manual/nixpkgs/stable/#chap-meta): package metadata conventions, including descriptions, homepage, licence, maintainers, platforms, broken status and other fields.
- [AppStream — Desktop Applications](https://www.freedesktop.org/software/appstream/docs/sect-Metadata-Application.html): application metadata, desktop entries, upstream metainfo, screenshots and icons.
- [AppStream documentation](https://www.freedesktop.org/software/appstream/docs/): broader component and metadata specification.
- [NixOS package search](https://search.nixos.org/packages): existing package-index/search experience; useful as a reference for package discovery, not as the finished consumer app store.
- [Official NixOS Wiki — Searching packages](https://wiki.nixos.org/wiki/Searching_packages): current package and option search approaches.

---

**Handoff note:** The project should first prove that a useful application catalogue can be generated from Nixpkgs and upstream metadata without package-by-package curation. Do not begin by manually building a curated app list or by exposing the NixOS option tree as the primary user interface.
