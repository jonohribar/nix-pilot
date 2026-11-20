# Global Search Surface (design draft)

Status: decided 2026-11-18, revised 2026-11-20 — v1 implements the global search (apps + settings, D16/D18 grouped, opens the store detail or the owning Settings category); the help / update-action domains join the same broadcast when those surfaces land (tracked nix-pilot-q1w). The product has four surfaces (Apps, Settings, Updates, global search); app-wide search sits atop the sidebar, and store-local search lives inside the Apps page (see [navigation map](../../../app/wireframes/navigation.md); layout decision recorded as [D28](../../research/04-recommendations.md#decision-log)).

## Scope and placement

A search box at the top of the sidebar searches the whole application: settings, applications, command-line tools, help content, and update actions. A search box inside the Store/Apps page searches only the catalogue. Placement matches scope — the global box sits where navigation lives (ensuring it always searches the same thing regardless of the surface you are on, the KDE Discover lesson), the store-local box stays inside the Apps page. This is the single app-wide search of constraint [C4](../../requirements/requirements.md#product-constraints); a query is broadcast to all indexed document types (search requirements [S1, S2](../../requirements/requirements.md#search)).

The global search is a separate surface reachable from anywhere. It does not replace the store-local search entry. A query that matches an application appears in the global results' Application group, and opening it jumps to the same detail/install page the Store search uses; one that matches a setting appears in a Settings group and opens the owning Settings category (S2 — "ssh" surfaces both the Remote Access setting and the OpenSSH application).

## Result presentation

Results are grouped and labelled by document type, reusing the catalogue class labels: Application, Unverified application, Command-line tool, Terminal app (decisions [D16](../../research/04-recommendations.md#decision-log) and [D18](../../research/04-recommendations.md#decision-log)). Command-line tools and unverified applications are labelled groups, never app tiles. Each result row shows the display name, group and version, a truncated description where present, and the evidence tier label. A query that matches several types shows one group per type (S2), e.g. "ssh" surfaces both a setting and an application.

## Interaction

Enter-to-navigate per result type: an application opens its detail/install page; a command-line tool offers install or run (per-user scope, [D9](../../research/04-recommendations.md#decision-log)); a setting navigates to its row in Settings; an update action opens the Updates transaction surface; help opens the relevant guide section. Keyboard: Ctrl+K focuses the global box; Esc clears the query and returns focus to the current surface. Ranking is the same scorer the Store uses (exact/prefix/contains on name plus description bonus), so results read consistently.

## Out of scope for this draft

Full-text search inside settings rows or help documents, ranking signals beyond the existing scorer, typo correction and fuzzy matching, search history and suggestions, and searching NixOS configuration outside the catalogue index.

## Open questions

The architecture's user-interface section lists global search as spanning settings, applications and help (C4); this spec adds update actions to the broadcast, which needs confirming in the architecture wording when the Updates surface lands. Decided 2026-11-20 (D28): the global box does not stay in the window header — it moves to the top of the sidebar, and Updates is a top-level sidebar surface only (the Settings home's "Updates" category row was removed; upkeep settings continue to render on the Updates page).
