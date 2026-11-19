# Package Version Selection

Design draft — not yet decided. An idea raised 2026-11-19: when installing an
app, let a non-technical user choose between the stable, the bleeding-edge
(unstable), or a specific version of that package.

## The request, in the user's words

- Stable is the default, with the current version highlighted.
- A dropdown offers "unstable — bleeding edge" as the cutoff, plus all other
  available versions.
- Or a single custom option that reveals a text field; the options filter as
  the user types the version number.

## What nixpkgs actually offers (feasibility ground truth)

- nixpkgs ships exactly **one** version of most packages per snapshot. There
  is no per-package version matrix to pick from; arbitrary "version numbers"
  mostly do not exist as installable choices.
- The real levers are **nixpkgs channels/revisions**: a *stable* channel
  (e.g. 26.05) and the *unstable* branch, plus the ability to pin a specific
  nixpkgs revision (which resolves every app to the version that revision
  ships).
- "Bleeding edge for one app but stable for everything else" is therefore not
  expressible by per-app version strings; it is a whole-catalogue/channel
  question re-scoped to that app (an override), which carries maintenance and
  eval cost.

So the honest surface is: **stable vs unstable** as the primary choice, with
"custom" reframed as a **nixpkgs revision/date pin** rather than a per-package
version string.

## Proposed v1

- At install time the surface shows Stable (default, current version
  highlighted) and a "bleeding edge" option marked clearly as unstable.
  Selecting unstable resolves the app against the unstable catalogue/channel.
- Custom is deferred: no text field for a made-up version number. If it
  returns, it is a nixpkgs-revision pin with filtered suggestions sourced from
  real available revisions, never a free-form "1.2.3" that doesn't exist.

## Dependencies and gating

- An unstable choice needs a second catalogue index built from nixpkgs-unstable
  and its own distribution (binary cache) — tightly coupled to the catalogue
  service body (`7wo.4`) and the binary-cache evaluation.
- Until that exists, the dropdown cannot be offered honestly.

## Open questions

- Whether per-app "one unstable app on a stable system" is in scope at all
  (an override) or out of scope for v1 (whole-catalogue channel switch only).
- Where the choice is stored/surfaced in the desired-state model (a per-app
  `channel` field) and how rollback treats an unstable-app install.