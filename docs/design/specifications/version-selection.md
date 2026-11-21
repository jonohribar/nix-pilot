# Package Version Selection

Status: decided (2026-11-21), revised from the D27 (2026-11-19) wording. The
feature is a per-app **input choice** — the system channel or the newest
channel — not a version picker. Feasibility history is in the decision log
(`04-recommendations.md`, D27) and tracked as beads `9f3.9`.

## The underlying want

A user installing an app can legitimately want a *newer* version than the one
their system ships — the current one is buggy, or a fix/feature just landed.
The want is per-app and immediate: "the newest of *this one app*, now", without
changing the rest of the system.

## Reality of nixpkgs (validity)

- nixpkgs ships **one** version of most packages per snapshot. There is no
  per-package version matrix to pick from; a free-form "1.2.3" mostly does not
  exist as an installable choice, and offering it would mean building and
  maintaining a catalog of every past version of every package.
- The real versions come from nixpkgs **channels/revisions**: the system's
  stable channel and the unstable branch. "Newer" is a channel question, not an
  app query.
- The honest lever is therefore the **input/channel** a package comes from, not
  a version number. We do not let the user pick a version; we let them pick an
  input.

## What we ship (decision)

A per-app choice of which input supplies that one app, made at install and
changeable later:

- **Stable — the system version (default):** the app comes from the system's
  own nixpkgs channel — no pin, identical to today's behaviour.
- **Newest available (unstable):** the app comes from the newest (unstable)
  channel — a scoped, controlled exception to the never-impose-a-pin rule
  (D23), for that one app only.

Labels use "stable"/"unstable" because that is the recognisable benefit pair
for consumers (tested/mainline vs cutting-edge). "Stable" describes the
*choice* (the reliable, mainline path), not a claim that the whole system runs
the stable channel — a system already on the newest channel still offers the
same two options relative to itself, so the label is never false. The option's
subtitle ties it to the system: "the version your system uses".

That is the whole choice. There is no exact-version picker and no per-app
"older than the system channel" option in this release (see Future directions).

## What this does and does not give (user perspective)

Gives:
- The newest version of a named app without waiting for the whole stable
  channel to roll forward, and without flipping the whole system to unstable.
- Per-app, independent control — each app can opt in separately.

Expectations to set in the UI, in plain words:
- "Newest available" is a **moving target**: the app re-follows the newest
  channel on each system update, so its version can change again later. It is
  not pinned to today's revision.
- Newest builds are the least-tested ones; the option says so in one line at
  opt-in.
- There is no "older than my system" and no "exact version" choice yet.

## Plain-language user interface

- The choice lives on the app's detail page as a collapsed, advanced opt-in —
  never a dropdown on every install row. The default pass ("install this") is
  frictionless.
- Wording is consumer language, never Nix (REQ-6). "Stable" and "unstable"
  are recognised consumer words (Chrome/Firefox channels) and are fine;
  internal Nix terms stay invisible — nixpkgs, input, rev, overlay, channel.
- Keep the benefit pair obvious at a glance: stable = reliable, tested;
  newest = cutting-edge but less tested, and it may change on each update.
- The current (system) version is always shown and highlighted; when the index
  knows a newer version exists, that hint is honest and actionable.
- A one-line note at opt-in: newest builds are less tested, and the app may
  change again on a later update.

## Resilience linkage

Per-app "newest" is a user choice; update reliability is separate and
transparent. If an update fails because a package regressed upstream, the
update recovery holds that package (in whichever channel it lives) at its
last-good revision so the rest of the update applies. For a user-chosen
"newest" app, recovery pins within that channel — it does not silently move
the app back to the system channel. Reusing one mechanism (a per-package
overlay pin) serves both per-app sourcing and update recovery.

## Gating

Requires the per-channel catalogue infrastructure (`7wo.4`) so the app knows a
newer version exists and can source the newest channel, and the documented
scoped D23 exception for that channel. Free-form custom version input stays
out.

## Mechanism plumbing (decided)

The per-app overlay is app-side, through the generated config the app owns
(decision D23 scoped exception), with no change to the user's flake module:
`render_module` emits `nixpkgs.overlays = [ (import ./per-app-overlays.nix) ]`
into the generated module only when per-app sources exist, the overlay file is
written beside it, and the switch/activation archive carries the overlay file
alongside the module so the relative import resolves from the extracted config.
The no-sources path is byte-identical and stays untouched.

## Future directions (deferred, not scheduled)

Possibilities for exact or older per-app versions, and for choosing how widely
a version applies:

- **Exact version, or older than the system channel,** via one of:
  - an advanced setting for a specific git hash/rev,
  - a self-maintained version catalog, proven to users (models like
    nixhub.io or lazamar.co.uk/nix-versions),
  - a live algorithm that walks nixpkgs git history to build a version history
    for the package while the user looks at the field.
- **Granularity choice** — let the user trade "set all packages to the same
  channel" (simplest, fastest) against per-channel against per-package
  fidelity. For now we decide the default ourselves (open per-package opt-in)
  and do not expose the toggle.

Both are explicitly out of scope for the first implementation.