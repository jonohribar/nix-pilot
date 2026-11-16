# Prior Art — GUI Attempts for Nix / NixOS

> Research note · 2026-11-16 · Status: current Sources: GitHub repositories and issues, NixOS Discourse threads. All links were live at the time of writing.

## In one paragraph

At least seven substantive projects have tried to give Nix or NixOS a graphical interface. All are dead or stagnant. Two patterns recur: projects that tried to render the full NixOS option tree collapsed under the language's expressiveness, and every project without an exception died of single-maintainer burnout. The only attempt at a consumer-facing store (nix-software-center) attracted real interest (839 stars) but never reached stable maintenance. No attempt has ever solved the metadata gap that a consumer store needs (see `02-catalogue-feasibility.md`).

## Project-by-project record

### nixui (2014–2015)

- Repo: https://github.com/matejc/nixui
- Web UI built with NodeWebkit and Polymer; wrapped `nix-env` and offered a configuration browser.
- Last commit 2015, 52 stars. Killed by ecosystem change (channels went stale, flakes arrived) that the wrapper had no path to follow.
- Recorded on the NixOS Wiki as a historical configuration editor: https://wiki.nixos.org/wiki/NixOS_configuration_editors

### nix-gui (2021–2022)

- Repo: https://github.com/nix-gui/nix-gui ("Use NixOS Without Coding")
- Loaded the **entire NixOS option tree** from the module system (paths, types, defaults, descriptions), rendered type-specific widgets, and wrote changes back to configuration modules.
- Documented limitations that became its death spiral:
  - Required `configuration.nix` to be a *function* module; set modules failed ("attempt to call something which is not a function" — issue #54).
  - `ListOf` / `AttrsOf` / `package` / `lambda` option types had no widgets; editing them fell back to raw Nix expressions — which defeats the purpose for the target audience.
  - Because it was "not yet thoroughly vetted", changes were written to a copy at `~/.config/nixgui/configurations/` rather than the real config.
- Maintenance stopped around 2022: issue #279 "project unmaintained?" (Sep 2022); issue #285 "Archive it, its been 4 years" (Jun 2026).
- Design explained in its own discussion threads: https://discourse.nixos.org/t/nix-gui-use-nixos-without-coding/15409

### nix42b

- GitLab: https://gitlab.com/juliendehos/nix42b
- Small graphical configuration effort; dead. Listed alongside the others in: https://discourse.nixos.org/t/thoughts-on-gui-package-management-in-nixos/73791

### nix-software-center (snowfall, 2022–)

- Repo: https://github.com/snowfallorg/nix-software-center
- The closest prior art to this project's "Software" area. GTK4/libadwaita store in Relm4, "heavily inspired by GNOME Software". 839 stars.
- Features: search, install/uninstall by editing `configuration.nix` (or `nix profile` / `nix-env`), updates, "launch without installing" via `nix-shell` / `nix run`, flakes toggle.
- Fate:
  - Configuration edits done through its `nix-editor` middleware, which string-edits the config file. The author's own assessment: "far from perfect and in desperate need of a refactor" (https://discourse.nixos.org/t/nixos-is-the-best-linux-back-end-ever-designed-but-it-cant-become-mainstream-without-a-front-end/46526/35)
  - **Offline package snapshot**: it could only list packages known from its own snapshot; anything else showed an "unavailable" dialog. This is the catalogue/system-revision mismatch made concrete.
  - 58 open issues; community verdict "abandonware" (Apr 2025): https://discourse.nixos.org/t/nixos-is-the-best-linux-back-end-ever-designed-but-it-cant-become-mainstream-without-a-front-end/46526/76
  - Archive request in Jun 2026: https://github.com/nix-gui/nix-gui/issues/285
  - The idea of a GNOME Software plugin was proposed in Dec 2022 and never happened: https://github.com/snowfallorg/nix-software-center/issues/16

### nixos-conf-editor and nix-editor (snowfall, 2022–)

- Repos: https://github.com/snowfallorg/nixos-conf-editor, https://github.com/snowfallorg/nix-editor
- The "VS Code settings" approach: parse an existing `configuration.nix`, present options, patch the file back preserving structure. Same author and same maintenance fate as nix-software-center.
- Parsing arbitrary Nix (functions, aliases, submodules, merging) is the hard core; the middleware was never finished to the author's satisfaction (refactor request above).

### SnowflakeOS (2022–)

- Announcement: https://discourse.nixos.org/t/snowflakeos-creating-a-gui-focused-nixos-based-distro/21856
- Opinionated NixOS "distro": calamares/os-installer installer, conf editor, software center, GNOME default. Intended as a gateway to NixOS, with tools to be upstreamed into nixpkgs.
- Declared effectively dead by the community in 2025: https://discourse.nixos.org/t/nixos-is-the-best-linux-back-end-ever-designed-but-it-cant-become-mainstream-without-a-front-end/46526/76
- Its author's own warning is the key quote for this project: "a simplification/GUI, an abstraction on top of an abstraction, will always be limiting" — he refused to ship an opinionated configuration format inside NixOS itself (same thread, post 35).

### PackageKit Nix backend (merged Feb 2022)

- PR: https://github.com/NixOS/nixpkgs/pull/94055
- The "official" route: a Nix backend for PackageKit so that GNOME Software could install packages. Merged after two years of work, then:
  - Crashed on search under GNOME Software ("code should not be reached", g_assert in pk-backend-nix.cc).
  - Remove was broken; nested attributes (gnome3.*) unsearchable until `recurseForDerivations` was added.
  - GNOME Software stayed empty without AppStream data (RFC #15932, see `02`).
  - Broke against the unstable Nix C++ API shortly after merge.
- It never became a supported path. The GNOME module still only enables gnome-software when flatpak is enabled. This is the closest thing to an official attempt, and it failed largely on the metadata gap.

### Adjacent routes (not NixOS-config GUIs)

- **GLF OS** ships Flatpak as its GUI app layer instead of Nix — works for apps, cannot manage CLI tools or system configuration, which is why it cannot replace Nix: https://discourse.nixos.org/t/thoughts-on-gui-package-management-in-nixos/73791
- **nixpak** (runtime sandboxing) and **declarative-flatpak** are the escape-hatch/alternative-layer proposals raised in the same thread.
- The "bridge nixpkgs to Flatpak" idea (Nix derivations as Flatpak apps) was prototyped and reported not to map cleanly (sandbox access to /nix/store; Electron/Tauri refusal): same thread, post 1.

## Key primary sources

- https://github.com/nix-gui/nix-gui (readme, issues, milestones)
- https://github.com/snowfallorg/nix-software-center (readme, issues)
- https://discourse.nixos.org/t/thoughts-on-gui-package-management-in-nixos/73791
- https://discourse.nixos.org/t/nixos-is-the-best-linux-back-end-ever-designed-but-it-cant-become-mainstream-without-a-front-end/46526
- https://discourse.nixos.org/t/snowflakeos-creating-a-gui-focused-nixos-based-distro/21856
- https://github.com/NixOS/nixpkgs/pull/94055
