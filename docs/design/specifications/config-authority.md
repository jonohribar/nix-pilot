# Configuration authority — the managed surface's source of truth

Status: proposal, recorded as decision D31.

Purpose: decide where the truth for installed packages and app-owned settings lives, so the app can be the controlling interface on a machine that already runs a NixOS configuration.

## Problem

A consumer settings + store app must decide what is authoritative for the things it manages.

Today the app generates a second, separate configuration that imports `/etc/nixos/configuration.nix` (D20, D21) and switches to it through the helper (D24–D26).

That model targets the conventional `/etc/nixos` layout and is not faithful on flake-first hosts.

On a flake host the applied generation does not include the user's real modules (home-manager, overlays, etc.), so the switched system is lighter and different from what the user normally runs.

Because the applied generation is built from a base that does not enable the app module, the module is absent from the running system after an apply, and the app can no longer revert or apply (nix-pilot-r3j).

The user's intent is that the app controls packages and Nix-owned settings, on top of the machine they already run, without a clean base or a second config to reconcile.

## Principles

One authoritative artifact holds the managed surface (packages + app-owned settings). Everything else derives from it.

The app must not require a clean base configuration. Nix module priorities (`mkOverride`, `mkForce`) let the app win over values the base already sets.

The app never edits existing configuration files. It contributes a layer; it does not rewrite the user's config (D21).

First run adopts the live system, so nothing the user had is lost or misrepresented.

## Recommended model — the app module lives inside the user's config

The app is installed as a NixOS module (`services.nix-pilot.enable = true`) inside the user's own configuration, whether flake or `/etc/nixos`.

At switch time the module reads the app's persisted desired-state, the single source of truth for the managed surface, and applies packages and settings within the same evaluation of the user's real config.

There is one configuration and one switch: the user's own `nixos-rebuild switch`. There is no separate competing generated configuration.

The module survives every switch, so the app keeps its apply and revert capability on every applied generation (fixes nix-pilot-r3j).

Base selection is no longer guessed. "The base" is the user's own config, because the module lives in it.

State reachability (revised 2026-11-25, D33, supersedes D32): the app writes a generated module `nix-pilot/state.nix` into the user's config tree carrying the managed surface, and the user's config imports it once (`imports = [ ./nix-pilot/state.nix ]`).
The state travels inside the config, so evaluation stays pure and the config remains reproducible and portable — it can be picked up and rebuilt elsewhere, which an impure live read (D32) broke.
The app writes only that one generated file; it never touches the user's own hand-written files (D21 core kept).
The file is plain Nix rendered by the app (its existing renderer), with static option keys, so there is no generic state processing in the module and no module-system recursion.
A missing file simply means no managed surface (fresh system); a malformed one fails the switch loudly rather than being mis-read as empty.
The first-import is a one-time line the app assists with; the app-owned file is tracked like any generated config (add to git once; local rebuilds read the working tree).

## Adoption on first run

On first run the app reads the live system: installed packages from the current generation's `environment.systemPackages` and per-user profiles, and current option values via the existing mirror-reality reads.

It snapshots that baseline into desired-state, so the app displays exactly what is installed and effective, and becomes authoritative from then on.

No clean base is required, and nothing is lost.

## Authority over a pre-filled base (tandem)

The app's surface coexists with the user's config rather than overriding it.
List options (`environment.systemPackages`, `users.users.<name>.packages`) are emitted as plain definitions, so they concatenate with the user's own entries and the module's binaries; the user can force the whole list with `mkForce` to take full control.
Scalar settings use `lib.mkOverride 90`: the app wins over the user's plain assignment without a conflict, and the user's `mkForce` still wins over the app.
Removal is by omission: the app stops emitting a package or setting, and it disappears unless the user's own config also declares it — the user's hand-written entries stay unless the user removes them (the user is never silently overridden).
First-run adoption snapshots the live system into the state file, so nothing is lost before the app becomes authoritative.

Removal is explicit and confirmed, never blind. The app clears a stored choice or removes a package only when the user asked, and the first-run adoption baseline must fail toward keeping: it never drops a package or value the user did not explicitly remove. A silent `mkForce` removal on a misread baseline is the worst failure mode for a source-of-truth tool, so adoption and override removal are designed to be conservative.

`hardware-configuration.nix` stays the file `nixos-generate-config` produced. The app references it and never regenerates or owns it.

## Scope and hard parts

Removal is authoritative only for the settings the app owns and for package lists. Arbitrary unmanged options in the base keep their values.

Per-user package installs are in scope and working. "Install for me" renders per-user packages through a backend that is home-manager when the target config enables it, and NixOS `users.users.<name>.packages` otherwise (D22, D23). That thin per-user-package slice is not deferred.

What is deferred is managing home-manager's full configuration surface: dotfiles, `programs.*`, per-user services, and home-manager options. That is a separate, unbounded surface and is out of scope for v1, not because per-user installs are unsupported but because editing the whole home configuration is a different power than installing a package for one user.

The module-in-config model keeps home-manager output intact during the test, because the applied switch is the user's own config rather than a rebuilt light base.

The flake integration, a module the user lists in their flake's `modules` and which follows their nixpkgs input, is the natural delivery (D22, D23). The conventional path, the module inside `/etc/nixos`, is the fallback.

## Install and first run

Two install paths, both feeding the same module-in-config model.

**Fresh (greenfield) — one click (deferred, not in v1).** For a machine with no config to preserve (a fresh NixOS install, or a machine the user explicitly wants rebuilt from a clean app-managed base), the app's installer asks for hostname, user, and bootloader (UEFI detected by default), runs `nixos-generate-config` against the live system to produce the hardware and base config, writes a complete starter configuration with the nix-pilot module enabled, the state import, and the empty state file, then rebuilds and switches. Nothing is carried over beyond hardware-config; everything is GUI-managed from then on. This is the former stashed fresh-base option, now the designed new-machine install path; it stays opt-in and is never the default for a machine that already has a config (adoption remains the safe path there).

**Existing config — copy-paste snippet (shipped v1).** The app detects the config kind and shows a short snippet the user pastes once: `modules = [ ./nix-pilot/module.nix ./nix-pilot/state.nix ];` (flake) or `imports = [ ./nix-pilot/module.nix ./nix-pilot/state.nix ];` plus `services.nix-pilot.enable = true;` (both kinds). The app writes the app-owned files into the config tree — `nix-pilot/module.nix` (its own module, so both config kinds import the same path and the version always matches the app) and `nix-pilot/state.nix` (the rendered desired state) — and adds them to git once. A first-run setup page shows the snippet with Copy / Open my config / parse-only verify; once the config references nix-pilot, the store shell takes over and a Settings Integration card shows status, setup re-entry, and confirmed removal.

The config reference to the module can never be fully automated for an existing config — the config is the only entry point into the system — so the snippet is the transparent two-line cost of the module-in-config model.

## Decision

Recorded as D31: adopt the module-in-the-user's-config authority model as the default for the managed surface, replacing the separate-generated-config switch of D20, D21 and D26 for that surface. State delivery is D33 (app-generated `nix-pilot/state.nix`, pure, tandem). The fresh-base (greenfield) installer path is the designed new-machine install; the snippet path covers existing configs.

## References

Decision D31 in the decision log.

Beads nix-pilot-r3j: applying drops the module, so the app loses the ability to revert.

Beads nix-pilot-die: long actions need visible progress and a unified revert control.

Existing D20, D21 (generated config shape and pointer import), D22, D23 (per-user backend), D24–D26 (activation, elevation, publish-before-switch), D27 (per-app version override).