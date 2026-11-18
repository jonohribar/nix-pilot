# Global Search Surface (design draft)

Status: decided 2026-11-18 — v1 implements the catalogue-only global search (header search, D16/D18 grouped, opens the store detail page); the settings / help / update-action domains join the same broadcast when those surfaces land in the app (tracked nix-pilot-q1w). The product has four surfaces (Software Store, Settings, Updates, global search); store-local search lives inside the Store page (see [navigation map](../../../app/wireframes/navigation.md)).

## Scope and placement

A search box in the global header searches the whole application: settings, applications, command-line tools, help content, and update actions. A search box inside the Store page searches only the Store. Placement must match scope — the global box belongs in the header, the store-local box stays inside the Store page. This is the single app-wide search of constraint [C4](../../requirements/requirements.md#product-constraints); a query is broadcast to all indexed document types (search requirements [S1, S2](../../requirements/requirements.md#search)).

The global header search is a fourth, separate surface. It does not replace the store-local search entry. A query that matches an application appears in the global results' Application group, and opening it jumps to the same detail/install page the Store search uses.

## Result presentation

Results are grouped and labelled by document type, reusing the catalogue class labels: Application, Unverified application, Command-line tool, Terminal app (decisions [D16](../../research/04-recommendations.md#decision-log) and [D18](../../research/04-recommendations.md#decision-log)). Command-line tools and unverified applications are labelled groups, never app tiles. Each result row shows the display name, group and version, a truncated description where present, and the evidence tier label. A query that matches several types shows one group per type (S2), e.g. "ssh" surfaces both a setting and an application.

## Interaction

Enter-to-navigate per result type: an application opens its detail/install page; a command-line tool offers install or run (per-user scope, [D9](../../research/04-recommendations.md#decision-log)); a setting navigates to its row in Settings; an update action opens the Updates transaction surface; help opens the relevant guide section. Keyboard: Ctrl+K focuses the global box; Esc clears the query and returns focus to the current surface. Ranking is the same scorer the Store uses (exact/prefix/contains on name plus description bonus), so results read consistently.

## Out of scope for this draft

Full-text search inside settings rows or help documents, ranking signals beyond the existing scorer, typo correction and fuzzy matching, search history and suggestions, and searching NixOS configuration outside the catalogue index.

## Open questions

The architecture's user-interface section lists global search as spanning settings, applications and help (C4); this spec adds update actions to the broadcast, which needs confirming in the architecture wording when the Updates surface lands. Whether the global box stays in the header when the Store window grows Settings/Updates is a layout decision for the msn.2 pass.
