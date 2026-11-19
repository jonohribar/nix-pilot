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
  configuration.nix          # outer: imports the live config + desired module (D21)
  generated-desired.nix      # rendered desired-state module (verbatim)
```

(D21: the live `/etc/nixos/configuration.nix` is imported by absolute pointer,
never copied, and already imports hardware — so the app does not keep a
`user-configuration.nix` or a hardware copy.)

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

## Review requirements (2026-11-19 independent review)

Fixes already applied in code: the env-gated activation codepath was removed
from the product binary (the shipped surface stays build-only per D20; the
Apply button is present but disabled), and settings values are escaped for
Nix interpolation so an accepted value can never become Nix code.

Open contract requirements this spec must satisfy before any product
activation code lands:

- **Publish-before-switch**: the candidate that gets applied must be
  byte-identical to the candidate that was prepared. The helper must receive
  the generated configuration in-band (not a path the app happens to have
  left somewhere), write it atomically into the app-owned area, and only then
  switch — one transaction, no stale-config window.
- **Failure restores the system**: on activation failure the transaction must
  restore the previous generation (the failure-and-recovery contract), not
  merely report. The UI never asserts "the system was not changed" without
  verification.

## Decisions (2026-11-19)

- **Elevation (D24): polkit `pkexec`.** Activation runs
  `pkexec nix-pilot-apply <subcommand>`; the desktop auth dialog is the gate,
  the rule authorizes only `org.nix-pilot.apply` for wheel, the helper is
  argument-free and pins the config path.
- **Failure recovery (D25): sanctioned go-back.** After a failed apply the
  page states honestly that the change did not complete and offers "Go back
  to the previous system" (`pkexec nix-pilot-apply rollback`) behind a
  one-dialog confirmation (REQ-3); the boot-menu card stays the boot-level
  fallback. No silent auto-rollback.
- **stateVersion is resolved by D21**: the generated outer emits
  `system.stateVersion` only in the fresh/no-user-config case; it is never
  duplicated.

## Publish-before-switch (D26, decided)

The applied candidate must be byte-identical to the prepared one.
**In-band stdin handoff:** Apply re-renders the generated configuration from
the persisted desired-state doc and feeds it to `pkexec nix-pilot-apply
switch` via stdin; the helper writes it atomically (temp + rename) into the
area and only then switches — one transaction, no stale-config window. The
renderer must be location-independent (byte-identical in a scratch dir and in
the area), enforced by a unit test. Rejected: write-then-hash-verify
(check-then-switch window, elevation on every prepare) and single-source-area
(elevation before any dry-run).

## Deferred

- Diff-auditing the user's config ("your config references a removed option"):
  **symbol-based, via evaluation** (option eval errors + definition
  provenance from the module system), read-only, as a diagnostic — never
  text-parsing arbitrary Nix. Shares machinery with 9f3.8's "current value"
  read-out. Low priority (P3).
