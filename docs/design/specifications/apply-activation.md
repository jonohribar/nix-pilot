# Apply Activation Specification

Design draft — not yet decided. The remaining 7wo.3 slice: activating a built
candidate generation and rolling it back. Grounds in [architecture.md](../architecture.md)
(install and system-state model, D9) and [failure-and-recovery.md](failure-and-recovery.md)
(one transaction: explain, evaluate, build, apply; rollback after apply; boot
recovery before any app). The build half of the backend is shipped
(`app/src/apply.rs`, decision D20): the generated configuration is built with
`nixos-rebuild build`, never activated. This spec decides how the built
candidate becomes the new generation.

## Invariant

Nothing ever activates a system except through a config the app owns and has
just built successfully. Concretely:

1. `apply.rs` writes the generated configuration into the app-owned
   configuration area (below) and `nixos-rebuild build` must succeed before
   activation is offered (the Updates surface already gates on the
   transaction's BuildCheck step).
2. Activation runs `nixos-rebuild switch` pointed at that same written config
   (`-I nixos-config=<area>/configuration.nix`). `switch` builds, activates and
   records one new generation; it never edits `/etc/nixos`, so the user's
   pre-existing configuration stays untouched at its original location and is
   only imported (preserved copy) into the generated config, per D20.
3. Rollback (`nixos-rebuild --rollback`) and the boot-menu recovery story stay
   wired through the transaction machine's gated edges; the machine is already
   the only path that can reach `Applying` / `RolledBack`.

## Configuration area

The app owns a root-owned directory, `nix-pilot`'s generated configuration:

```
/etc/nix-pilot/generated/
  configuration.nix          # outer: imports + stateVersion + desired module
  hardware-configuration.nix  # copy, refreshed by the user's nixos-generate-config
  user-configuration.nix      # preserved pre-existing configuration (migration)
  generated-desired.nix       # rendered desired-state module (verbatim)
```

- Writes to this area require elevation. The elevation mechanism is an open
  decision (see Open questions); `nixos-rebuild build`/`switch` already need
  root, so a single controlled elevation boundary covers both write and
  activate. The dev/test path writes the identical layout into a scratch dir
  (as today) and never elevates.
- The pre-existing config is **imported live by pointer, never copied (D21):**
  the outer file imports `/etc/nixos/configuration.nix` by absolute path, so
  the user's own hand-edits are picked up on every build and the app never
  writes that file. `system.stateVersion` and the hardware import are therefore
  owned by the live config; the generated outer emits `stateVersion` only when
  there is no pre-existing config (fresh / non-NixOS case).

## Transaction wiring

The existing `TxStep` machine (updates.rs, failure-and-recovery semantics)
gains its real driver:

- `BuildCheck --BuildOk--> Applying` fires after a successful build of the
  candidate in the configuration area (not a scratch copy).
- `Applying --ApplyOk--> Done`; `Applying --ApplyFailed--> ApplyFailed`; the
  rollback edges (`ApplyFailed --Rollback--> RolledBack`, `Done --Rollback-->
  RolledBack`) map to `nixos-rebuild --rollback` which returns the previous
  generation. Boot recovery stays the boot-menu story in the Updates page's
  recovery card, unchanged.

The Updates page's Apply control enables only in `BuildCheck` after a green
build; the review text states what will change before anything is applied
(REQ-3) and that the user can go back to the previous system afterwards.

## Verification

- Unit: transition wiring (already covered for the machine); a contract test
  asserts activation only ever receives the app-owned config path.
- Live (opt-in, like the existing apply tests): build + switch in a scratch/
  VM harness that cannot affect the host (the same "explicit scratch dir"
  rule that gates every apply test today). Never a switch on a live system.

## Open questions

- Elevation mechanism for writing the configuration area and running `switch`
  (polkit `pkexec` vs `sudo` vs a NixOS-deployed setuid helper); affects only
  the wrapper, not the config shape.
- Whether the outer config keeps `system.stateVersion` duplicated while the
  preserved config also sets it (currently harmless — equal values merge).
- Whether `user-configuration.nix` migration should diff-audit the preserved
  file so the app can surface "this setting no longer applies" (D14/R04
  precedent) when the user's own config references removed options.
