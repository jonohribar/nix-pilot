# Package Version Selection

Design draft — not yet decided. Idea raised 2026-11-19: let a non-technical
user choose the stable, the bleeding-edge (unstable), or a specific version of
an app at install time. Reviewed 2026-11-19 for usability, validity and
feasibility; this version reflects that review, which found the idea mostly
good but the per-app "unstable / custom version" shapes in tension with the
decided architecture.

## The underlying want

A user installing an app can legitimately want a *newer* version than the one
their system ships (the current one is buggy, a fix/feature just landed) or,
much rarer, an *older* one (a regression). The current-version display is
real: the catalogue index already carries `version` per package, so showing
"version X (current)" is trivial and cheap.

## Reality of nixpkgs (validity)

- nixpkgs ships **one** version of most packages per snapshot. There is no
  per-package version matrix to pick from; a free-form "1.2.3" mostly does not
  exist as an installable choice. A custom-version text field is not
  implementable as described and should not be promised.
- The real versions come from nixpkgs **channels/revisions**: a stable channel
  (e.g. 26.05) and the unstable branch. "Newer" is a channel question, not an
  app query.

## The central tension (feasibility)

Per-app "unstable" requires the app to pin its **own** unstable nixpkgs for
that one app. That collides with decision D23: in the flake integration the
app's module "follows the user's nixpkgs input" and deliberately never
imposes its own pin over the user's eval (that was the earlier finding — the
only way the app fights a user's config is imposing its own nixpkgs pins).
So per-app bleeding-edge, in its naive form, is a decision reversal, not a
small feature.

It also has a hard infra cost: an unstable choice needs a second catalogue
index built from nixpkgs-unstable plus its own distribution (binary cache) —
the catalogue-service body (`7wo.4`).

## Options

- **A. Whole-system channel switch (recommended).** One setting, plain words:
  "System packages: stay on the stable channel / switch to the newest
  available." Lives in settings; a whole-system switch keeps one nixpkgs, one
  eval, no mixed closures, matches the "one nixpkgs, one system" model, and
  needs no per-app pin. Costs: it is not per-app.
- **B. Per-app unstable override.** The app pins its own unstable nixpkgs for
  a single named app (via the D23 module). Gives the requested per-app UX but
  reverses the no-imposed-pin principle and adds a channel + catalogue +
  mixed-closure surface — the most complex option.
- **C. Pointer only.** Skip the mechanism; surface a plain-language pointer
  ("to get the newest version, your system must run the newer channel") that
  links to option A.

## Proposed ideal UX (whichever option)

- Always an opt-in, advanced/overflow surface — never a dropdown on every
  install. The default pass ("install this") is frictionless.
- Plain words, never Nix vocabulary (REQ-6): "Version shown in the store"
  instead of "stable", "newest available test build" instead of "unstable",
  "choose a specific release" instead of "pin a revision".
- Current version is always highlighted; a newer-available hint (when the
  index knows a newer version exists) is honest and actionable.

## Gating

- Options A and B both need catalogue-channel infra (`7wo.4`) to know whether
  a newer version exists; B also needs the D23 principle revisited. Until
  that decision is made, nothing is shipped.

## Decision (D27, 2026-11-19)

**Per-app override (option B)**, chosen: at install the user picks stable
(default, current version highlighted) or bleeding-edge for that app; the app
resolves the app against a per-app channel (unstable). This is a scoped,
controlled exception to the "never impose a pin" reading of D23 — the app
imposes its own nixpkgs pin *for one named app only*, never the whole system.

Implementation is gated on per-channel catalogue infra (`7wo.4`) so the app
knows whether a newer version exists and can source the unstable channel.
Free-form custom version strings stay out (no per-package version matrix in
nixpkgs). UX stays as described above: opt-in advanced surface, current
version always highlighted, plain words. Tracked as beads `9f3.9`.