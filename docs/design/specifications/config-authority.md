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

State reachability (decided 2026-11-25, D32): the module reads the persisted desired-state at `$HOME/.local/share/nix-pilot/desired-state.json` with `builtins.readFile` plus `builtins.fromJSON`, and the app's switch runs with `--impure`.
State stays in the app's own area and never enters the user's configuration repository, so D21 is preserved rather than relaxed.
The impurity is scoped to the app-managed surface (packages and owned settings); the built lines they produce remain content-addressed.
The module is tolerant: a missing or malformed state file evaluates to an empty desired-state, so a fresh or manually repaired system adopts on its next run instead of failing the switch.
A future per-user prime mode (a git-tracked state module the flake imports) stays an explicit opt-in, not the default.

## Adoption on first run

On first run the app reads the live system: installed packages from the current generation's `environment.systemPackages` and per-user profiles, and current option values via the existing mirror-reality reads.

It snapshots that baseline into desired-state, so the app displays exactly what is installed and effective, and becomes authoritative from then on.

No clean base is required, and nothing is lost.

## Authority over a pre-filled base

New packages and set values are emitted into the managed lists and options, where `mkOverride` wins over the base's normal assignment.

Removal of a package or option the base previously set uses `mkForce` on the whole list or option, covering `environment.systemPackages`, `users.users.<name>.packages`, and owned settings.

Removal is explicit and confirmed, never blind. The app clears a stored choice or removes a package only when the user asked, and the first-run adoption baseline must fail toward keeping: it never drops a package or value the user did not explicitly remove. A silent `mkForce` removal on a misread baseline is the worst failure mode for a source-of-truth tool, so adoption and override removal are designed to be conservative.

`hardware-configuration.nix` stays the file `nixos-generate-config` produced. The app references it and never regenerates or owns it.

## Scope and hard parts

Removal is authoritative only for the settings the app owns and for package lists. Arbitrary unmanged options in the base keep their values.

Per-user package installs are in scope and working. "Install for me" renders per-user packages through a backend that is home-manager when the target config enables it, and NixOS `users.users.<name>.packages` otherwise (D22, D23). That thin per-user-package slice is not deferred.

What is deferred is managing home-manager's full configuration surface: dotfiles, `programs.*`, per-user services, and home-manager options. That is a separate, unbounded surface and is out of scope for v1, not because per-user installs are unsupported but because editing the whole home configuration is a different power than installing a package for one user.

The module-in-config model keeps home-manager output intact during the test, because the applied switch is the user's own config rather than a rebuilt light base.

The flake integration, a module the user lists in their flake's `modules` and which follows their nixpkgs input, is the natural delivery (D22, D23). The conventional path, the module inside `/etc/nixos`, is the fallback.

## Optional later — fresh-base (greenfield) mode

Stashed as a possible later addition, not in scope now: an explicit fresh-base mode for new machines that generates a clean starter configuration importing only hardware-config, so a greenfield user builds the system up from the app with no import of a pre-existing configuration. It solves a clean first install cleanly, but it is hostile for existing machines (their packages and settings must be re-entered before the first switch), and it forces the app toward owning all of NixOS (the unbounded-surface risk). It is therefore optional and opt-in, deferred behind the module-in-config default.

## Decision

Recorded as D31: adopt the module-in-the-user's-config authority model as the default for the managed surface, replacing the separate-generated-config switch of D20, D21 and D26 for that surface. A fresh-base (greenfield) mode is a possible later addition, not part of D31.

## References

Decision D31 in the decision log.

Beads nix-pilot-r3j: applying drops the module, so the app loses the ability to revert.

Beads nix-pilot-die: long actions need visible progress and a unified revert control.

Existing D20, D21 (generated config shape and pointer import), D22, D23 (per-user backend), D24–D26 (activation, elevation, publish-before-switch), D27 (per-app version override).