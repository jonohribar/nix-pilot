# Requirements

The requirements the product must meet. This document owns the requirement set. It does not describe how the product is built; architecture and specifications live in `../design/`.

## Reading notes

- User context, personas, stories and journeys live in [user-needs.md](user-needs.md).
- The derived user requirements REQ-1…REQ-8 below are the canonical set; the story ids after each entry show their origin in user-needs.
- Provisions that are set by a recorded decision are marked with a link to the [decision log](../research/04-recommendations.md#decision-log) rather than restated here, so decisions are kept in one place.

## Product constraints

- **C1. Audience.** The target user is a non-technical desktop user, as defined in [user-needs.md](user-needs.md).
- **C2. Nix exposure.** The normal mode must not expose Nix syntax, Nix concepts or configuration files.
- **C3. Product shape.** The product is one integrated application with Settings, Software, and Updates and Recovery areas.
- **C4. Search.** A single search spans settings, applications and help.
- **C5. Catalogue.** The software catalogue is generated automatically from Nixpkgs and upstream metadata; it is not a hand-maintained list of packages (decision [D4](../research/04-recommendations.md#decision-log)).
- **C6. Configuration ownership.** The application owns the system configuration it generates.
- **C7. Existing configurations.** Importing or editing existing Nix configurations is not required for the initial release.
- **C8. Developer mode.** Developer tools are optional and hidden by default and are not part of the normal experience.
- **C9. Package universe.** User-relevant applications are exposed by default; libraries and low-level tools stay out of ordinary browsing.
- **C10. Unfree applications.** Unfree applications are listed by default, with an optional user filter to hide them (decision [D1](../research/04-recommendations.md#decision-log)).

## Non-goals for the initial product

- Teaching users Nix, Nix expressions, flakes or the module system.
- Editing arbitrary hand-written Nix configurations.
- Supporting every existing NixOS configuration or third-party module combination.
- Presenting every Nixpkgs derivation as an ordinary desktop application.
- Maintaining a hand-curated catalogue of individual packages.

Importing existing configurations, editing raw Nix and advanced diagnostics may be considered later, but must not compromise the simple default experience.

## Search

- **S1.** The search index combines four document types: applications (names, aliases, package names, descriptions, upstream IDs, executable names, categories), settings (human-readable names, descriptions, synonyms, supported actions), command-line tools (names, descriptions, package names) and help content (explanations and task guidance).
- **S2.** Results are grouped and labelled by type. A common term such as "ssh" shows both the Remote Access setting and the OpenSSH application where relevant (journey B in user-needs).
- **S3.** Settings workflows are product-authored. Nixpkgs cannot automatically generate a good consumer settings interface for every system option; designing those workflows is product work, while the application catalogue is generated (decision [D4](../research/04-recommendations.md#decision-log)).

## Ranking and trust

- **R1.** A ranking must not be labelled "most installed on NixOS" without real usage data.
- **R2.** Ranking signals are evidence-based and tracked separately: upstream project activity, public download statistics where available, cross-distribution availability, AppStream or ODRS reviews, maintenance signals and local compatibility. Each signal has bias and coverage limitations.
- **R3.** First iteration: rank exact name matches and compatibility highly for search.
- **R4.** "Popular" or "recommended" labels appear only when supported by evidence and are transparently explained.
- **R5.** Popularity, maintenance and security are treated as separate signals.
- **R6.** No telemetry is collected by default merely to create popularity rankings.

## Derived user requirements

These are the canonical requirement statements; each carries the origin story id from [user-needs.md](user-needs.md).

- **REQ-1** (stories P1, L1). A user can install and remove an application with a single action and no technical steps.
- **REQ-2** (story P2). A user can run one search across settings, applications and help, in ordinary words, with results grouped by type.
- **REQ-3** (story P3). Every system change is explained in plain language before it is applied.
- **REQ-4** (stories P4, L4). A user has a plain-language recovery path when a change fails to apply or the system will not start; at minimum, return to the previous working state. This is the deepest requirement and is tracked as risk [R03](../design/risks.md).
- **REQ-5** (story P6). Common system configuration is presented as settings screens covering networking, display, sound, printers, users, power and remote access.
- **REQ-6** (story P7; constraint C2). Nix concepts, configuration files and error output stay out of the normal experience.
- **REQ-7** (stories P1, L1; constraint C9). The set of installable applications is clear, and libraries and system components are excluded from ordinary browsing; command-line tools are searchable as a labelled "Command-line tool" group, never as application tiles (decision [D16](../research/04-recommendations.md#decision-log)).
- **REQ-8** (story P5). Updates are reported in plain language and applied as a confirmable, recoverable transaction.