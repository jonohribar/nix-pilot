# Settings Schema Specification

How the Settings surface is defined, versioned and shipped. The surface is product-authored: Nixpkgs cannot generate a consumer settings interface, so designing the workflows is product work (requirement [S3](../../requirements/requirements.md)). This spec pins the schema shape and the boundary that keeps the surface tractable.

## Boundary

The surface is bounded to system-owned settings. A setting is in scope only when all three hold (decision [D14](../../research/04-recommendations.md#decision-log)):

- It maps to a stable NixOS option, not a per-desktop config-file backend.
- It means the same thing across the desktops the product runs on.
- The product can own its versioning and migration.

Desktop-environment chrome (panels, themes, wallpapers, "show battery percent"), and per-application preferences, are out of scope. The product does not author a mapping for them; it links to the tool that owns them (the desktop's settings app, the application's own settings) or states "this setting lives in your desktop's settings". Nothing in the surface reimplements another tool's preferences.

## Schema shape

Each setting workflow is versioned record with the following fields:

| Field | Contents |
|---|---|
| Identity | A stable, versioned key that does not change on cosmetic renames |
| Presentation | Friendly name, plain-language behaviour, and the outcome statement shown to the user before the change is applied (REQ-3) |
| Mapping | The single owned mapping to the backing NixOS option(s); one option per setting, never a per-desktop fork |
| Supported actions | The operations the workflow supports (enable, disable, configure a value) and their valid ranges |
| Control | The user-facing control the workflow renders (toggle, text field, or — for options with a small fixed value set — a plain-language dropdown). An option whose values are product-authored choices (an enum, like update timing "apply right away vs at next restart") must not be a free-text field exposing raw option values (C2); it is authored as a dropdown whose choices map to the backing values. |
| Presentation rule | Blurbs, placeholders and examples must be valid input: an example shown to the user must pass the workflow's own validator, and no Nix concept (flags, flake paths, rebuild verbs, hashes) may appear in user-facing strings (C2, REQ-6). Internal metadata (evidence tiers, cache state) is never shown in the normal experience (C8). |
| Scope | System-wide; the value is applied through the backend's generation transaction |
| Search | Human-readable name, synonyms and supported actions, so the setting is findable and grouped by type (S1, S2) |
| Currency | How the setting tracks the backing option across Nixpkgs revisions |

## Currency and migration

- The schema is versioned and ships a documented migration path, so a Nixpkgs option rename never silently breaks a user's setting (decision [D6](../../research/04-recommendations.md#decision-log); risk [R04](../risks.md#r04)).
- When a setting's backing option no longer applies (renamed, removed or moved), the UI tells the user "this setting no longer applies" instead of generating stale configuration ([architecture.md, Settings surface](../architecture.md#settings-surface)).  *(Shipped 2026-11-20: the current-value read-out classifies a missing-attribute eval failure as NoSuchOption and the Settings page shows this exact message.)*
- A version change is additive and migration-bearing; old versions of a setting record remain readable so installed state can be upgraded rather than dropped.  *(The versioned migration *map* itself is added when a backing option actually renames; none of the shipped workflows has renamed in the pinned snapshot yet.)*

## Control layout

Each setting renders as one card with three stacked rows:

- A title row: the setting's plain-language name with a small info icon; hovering the icon shows the detailed description (`details`), keeping the card compact.
- A short summary line above the control (the `blurb`).
- One horizontal control row: the control (switch, dropdown or text entry), an inline capitalised status for switches (`On`/`Off`, mirror-reality: the effective value, stored choice else the live system value), and a `Reset to default` button to its right.

`Reset to default` is always visible and only greyed when no change is stored, so the control never pops in or out; it points the setting back at the system value (removing the stored choice). Text and choice rows preselect the live system value when nothing is stored, under a guard so the prefill is never recorded as the user's choice.

## Initial surface

The first-release surface covers the areas in REQ-5: networking, display, sound, printers, users, power and remote access. Each area is a set of setting workflows meeting the boundary above; anything in those areas that fails the boundary (for example display appearance, which is desktop chrome) is exposed as a pointer, not a workflow.

### First-release workflows

Shipped in `app/src/settings.rs` (single-value overrides, verified against the pinned nixpkgs; each renders as an app-wins `lib.mkOverride 10` line and is stored in `DesiredDoc.settings`):

| Setting | NixOS option | Type | Plain label |
|---|---|---|---|
| Device name | `networking.hostName` | string | device name on the network |
| Network connections | `networking.networkmanager.enable` | boolean | manage Wi-Fi/wired via NetworkManager |
| Bluetooth | `hardware.bluetooth.enable` | boolean | use Bluetooth devices (headphones, mice, keyboards) |
| Bluetooth at startup | `hardware.bluetooth.powerOnBoot` | boolean | turn Bluetooth on at start (needs it enabled) |
| Time zone | `time.timeZone` | string | the time zone used for times and dates |
| Language and region | `i18n.defaultLocale` | string | language for messages, dates and numbers |
| Printing | `services.printing.enable` | boolean | start the printing service |
| Power management | `powerManagement.enable` | boolean | suspend and power-saving features |
| Sound | `services.pipewire.enable` | boolean | play and record sound through the desktop (PipeWire) |
| Remote access (SSH) | `services.openssh.enable` | boolean | allow connecting to this computer remotely |
| Remote access keys (SSH) | `users.users.<name>.openssh.authorizedKeys.keys` | list of strings (per user) | the public keys allowed to log into this computer, one per line (add/remove) |

Software (category `software`, rendered on the Settings page):

| Setting | NixOS option | Type | Plain label |
|---|---|---|---|
| Restricted-licence software | `nixpkgs.config.allowUnfree` | boolean | allow installing apps whose licence restricts redistribution (for example proprietary apps) |

This closes the gap where an unfree install was refused with no owned
switch; when it is off, an unfree install is refused and the guidance sentence
points here instead of a dead end.

Automatic upkeep (category `updates`, rendered on the Updates page as the
set-and-forget "Automatic" card; verified against the pinned nixpkgs):

| Setting | NixOS option | Type | Plain label |
|---|---|---|---|
| Automatic cleanup | `nix.gc.automatic` + `nix.gc.options` | boolean + string | regularly remove what nothing needs |
| Reclaim storage | `nix.optimise.automatic` | boolean | hard-link duplicates to save space |
| Automatic system updates | `system.autoUpgrade.enable` | boolean | check and switch on a schedule |
| Update schedule / delay / action / config | `system.autoUpgrade.dates`, `.randomizedDelaySec`, `.operation`, `.flake` | strings | when and how updates run |

All four upkeep-card presentation debts raised in the 2026-11-20 visual review were resolved with the authored-controls work: the update action, update delay, and cleanup retention now render as plain-language Choice dropdowns (`system.autoUpgrade.operation`, `system.autoUpgrade.randomizedDelaySec`, `nix.gc.options`), and the flake setting's blurb was reworded to plain language (`system.autoUpgrade.flake`).

Candidates explicitly excluded after verification: `programs.nix-index-database.comma.enable` does not exist in the pinned nixpkgs snapshot; home-manager auto-update options exist only where the home-manager module is enabled (D23 gating, pending detection decision).

Pending first-release candidates that need a per-workflow design (structural / safety-sensitive, not a plain value): user accounts (`users.users.<name>`, add/edit/remove workflow), printer queues (runtime CUPS state, a pointer not an option), idle/sleep timeouts (live in the desktop, no cross-desktop option).

### User-accounts workflow (design, 2026-11-19)

The module the app contributes is **additive** (D21 pointer import): it merges
`users.users.<name>` definitions over the live system config. That decides
what "create / edit / remove an account" can honestly mean:

- **The app owns the accounts it creates.** Add = emit
  `users.users.<name> = { isNormalUser = true; description = ...; }` in the
  desired-state module; remove = drop that definition. Because the app owns
  the definition, removing it cleanly un-creates the account on the next
  generation.
- **The app does not own live-config accounts.** Removing or re-flavouring a
  user the user's `configuration.nix` defines is not expressible additively
  (a removed option the live config also sets is a conflict or a no-op). Such
  accounts are displayed read-only, never edited or removed.
- **Creation must refuse collisions.** Before creating a name, check the
  account does not already exist on the system (`id <name>` / `getent passwd`
  — read-only); a same-named live account cannot be cleanly owned.
- **Passwords are not held by the app.** The app never stores or renders
  passwords. New accounts are created without a usable password and the flow
  prompts the user (or the first login) to set one via the desktop's own
  account tooling; a forced-change one-time password is the only escalation
  considered, and it is out of the first slice.
- **Removal is a confirmed, plain-worded destructive action.** "This removes
  the account and its files" with a confirmation dialog (REQ-3); the boot-menu
  recovery card stays as the undo path for a mistaken system change.

The surface is unbounded by nature; carrying and reviewing it is risk [R04](../risks.md#r04) and scope risk [R09](../risks.md#r09). The boundary above is what keeps it finite: only Nix-owned, cross-desktop, migrate-able settings are authored.

## Relationship to the product model and search

- Settings workflows are product data, not catalogue data: Nixpkgs produces the application catalogue (decision [D4](../../research/04-recommendations.md#decision-log)), while the settings surface is authored.
- The desired-state model stores the user's chosen setting values; the schema above defines the shape each value takes and how it maps to generated config.
- The settings index feeds the same search as applications and help, grouped and labelled by type (S1, S2), so a term can surface both a setting and a relevant application.

## Non-goals

- Not a generic editor for arbitrary NixOS options or hand-written configuration (requirements non-goals).
- Not a replacement for any desktop's appearance preferences (decision [D14](../../research/04-recommendations.md#decision-log)).
