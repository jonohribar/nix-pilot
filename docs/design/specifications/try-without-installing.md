# Try Without Installing — Interaction Design

Status: design recommendation for review (9f3.13). Not yet a decided spec;
decisions are recorded in the decision log after review. It answers the
question the task raises before implementation: the process lifecycle and what
cancelled / finished / running mean to the user.

## What the action does

From a store detail page the user can start a desktop app without installing
it. The app runs through `nix run <channel>#<attr>` as a detached child
process; the window the app opens *is* the "try" session. Nothing is written
to the system configuration, so on exit the app is gone (only its Nix store
artifacts remain, as with any cached build).

The precedent is the "try it without installing" action on the store card in
[DrakeFlake](https://drakeflake.garudalinux.org/), a NixOS desktop app. The
behaviour matches its own docs: packages from the system's nixpkgs can be
"added to the system or tried without installing".

## Where it surfaces

- A secondary **"Try it"** button on the detail action card, beside the
  primary "Install for me" button. It is visually quieter than Install and is
  never the default action.
- The button is present for rich and basic desktop applications only.
  Command-line tools and terminal apps are not "tried" from the store (they
  would run into a terminal), so Try is hidden for those classes.
- "Try it" runs the app from the system's channel the user already chose; it
  does not pull a newer or different channel.

## The lifecycle the user sees

The session is a small four-state machine. The store detail page never blocks;
the try session runs detached and the UI reflects its state.

1. **Starting** — "Building X — the first run downloads and builds it; this
   can take a few minutes." The first `nix run` of a not-yet-cached attribute
   builds before the window opens ([Determinate Systems on nix run](https://determinate.systems/blog/nix-run/): first invocation takes a while, later ones are near-instant from the cache). Cancel is available here: it kills the child and returns to the idle card.
2. **Running** — the app window is up and the user is using it. The card
   reads "Running X" with a **Stop** control and a "won't be installed" hint.
3. **Ended by the app** — the user closed the app window; the session is over.
   The card returns to idle with "Finished. It wasn't installed — choose
   'Install for me' to keep it."
4. **Stopped by the user** — same as ended, via the Stop control.

### Cancelled, timeout and process hygiene

- **Cancel** cancels a non-started build; **Stop** ends a running session. Both
  terminate the `nix run` child (and its process group, so the app's own
  children do not leak).
- **No idle timeout.** The user is actively using the app; a session is not
  time-bounded. The honest guard is process hygiene, not a timer.
- **Parent-exit reaping.** The child is killed if the nix-pilot app itself
  quits while a session is running, so a "try" never survives the parent and
  never leaves an orphaned process. This is the lifecycle's hard invariant.

## Sandbox honesty (a separate decision)

A bare `nix run` is not an isolation sandbox: it runs the app with the user's
own privileges, exactly as an installed app would. There is no filesystem,
network or permission boundary by default. True isolation would need a wrapper
(`bubblewrap` / `firejail`) or a container, which is its own scoped decision
and a later hardening.

The v1 wording must therefore promise only what is true — "runs without being
installed" — and never claim the app is sandboxed or isolated. The sandbox is
listed as a future direction, not part of this design.

## Interaction with licence settings

`nix run` honours the same `nixpkgs.config.allowUnfree` the system evaluates
with. Without it, trying an unfree app fails at evaluation and the card shows
the same guidance that governs installs: turn on **Restricted-licence
software** in Settings → Software to allow it. This keeps the [licence
workflow](settings-schema.md) one consistent path across install and try.

## Acceptance for a first slice

- A "Try it" action on the detail card of a rich/basic desktop app.
- Pressing it shows Starting → Running with a Stop control, and the session
  ends cleanly on app exit, Stop, or nix-pilot exit (no orphaned process).
- The system configuration is never modified; an app-only build is the only
  side effect.
- Cancel of a not-yet-started build works; unfree apps degrade to the existing
  licence guidance.

## Sources

- DrakeFlake — try without installing: https://drakeflake.garudalinux.org/
- Determinate Systems — using nix run (first-run latency):
  https://determinate.systems/blog/nix-run/