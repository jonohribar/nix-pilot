# Application internals refactor

This plan tightens the boundaries inside `app/src/` without changing the product architecture or adding features. The aim is to make ownership, dependencies, and operation lifecycles apparent from the module structure, so a change can usually be understood and tested within the subsystem that owns it.

## Boundary rules

- A piece of mutable state has one authoritative owner. Other structures are either derived views or explicitly documented staging state with a tested synchronization invariant.
- Dependencies point inward: GTK pages invoke application-facing operations and render typed outcomes; domain and application logic do not depend on GTK; system and Nix adapters own external I/O and do not reach into UI code.
- A function should have one coherent reason to change. Composition functions may coordinate multiple steps when that coordination is their purpose; they should delegate implementation details rather than absorb each subsystem's policy.
- A module name and public API describe the responsibility actually implemented there. Helpers shared across modules have a clear owner rather than being copied or hidden in an unrelated module.
- Refactoring preserves user-visible behavior, desired-state file compatibility, transaction safety gates, and existing system-operation boundaries. Behavior changes need separate justification and tests.

## Desired-state and setting ownership

The package `ChangeQueue` and persisted `DesiredDoc` both carry package intent, while settings metadata and values currently cross between `settings.rs` and `desired_state.rs`. Define which structure owns package intent and whether the queue is a separate staging concept or a view of that state. Establish one explicit update path for package add/remove/scope changes, load hydration, and persistence; callers should not need to remember an undocumented second write.

Separate the setting value/schema contract from persistence and Nix rendering so the settings surface and desired-state model do not form a dependency cycle. Keep the product-authored settings specifications, validation rules, and deterministic generated-module output testable without GTK. Preserve the existing serialized schema and migration behavior.

### Investigation and candidate direction

The current UX and persistence behavior point toward `DesiredDoc` as the authoritative package-intent model, with the on-disk JSON as its durable snapshot. Store add/remove, Settings edits, and Updates automatic-upkeep edits all mutate the in-memory document and invoke a save hook; they do not wait in a separate unsaved staging area. `ChangeQueue` therefore appears to be a second in-memory representation used for display, availability checks, and package scope, rather than an independent transaction boundary.

There is a concrete integrity edge case to resolve: `DesiredDoc` allows one package attribute in both system and per-user sets, but `ChangeQueue::add` de-duplicates only by attribute. On launch, `seed_queue` adds system packages before user packages, so an attribute present in both scopes is represented only as system-wide. The next persistence callback rebuilds both package sets from that queue and can silently erase the user's per-user selection. The task must establish whether same-attribute cross-scope entries are valid; whichever policy is chosen must be enforced at the authoritative model boundary and covered by a regression test.

Candidate direction for review: make the in-memory desired-state model the sole owner of package scope, remove the separately mutable queue, and derive the Updates package list and checks from a stable package-intent iterator. Derive presentation names from the catalogue where available, with an attribute-name fallback; do not persist display labels merely to support the queue. Route package add/remove/scope changes through desired-state operations and save the resulting document snapshot. Preserve the current JSON shape unless a tested migration is explicitly justified. If an independent staging queue is retained instead, define a real commit/cancel boundary and make its relationship to persisted desired state explicit; the current auto-persist behavior does not provide that distinction.

### Dependency direction for settings values

The direct cycle is small: `settings.rs` imports `SettingValue` from `desired_state.rs`, while `desired_state.rs` calls settings validation and rendering. A low-disruption option is a neutral settings-value module that owns `SettingValue`; both the settings schema and persisted document depend on that type, while desired-state code may depend on settings policy in one direction. An alternative is to make the settings domain own the value type and schema together, with desired-state depending on it. Choose based on whether settings policy should be a dependency of persistence, not simply on minimizing file count. Either option must avoid moving GTK controls or persistence responsibilities into the settings schema.

### Planned work and acceptance

1. Decide and document the valid package identity/scope invariant, including whether the same attribute may appear once per scope; add a regression test for load/hydrate/save so the current potential data-loss case cannot recur.
2. Select one owner for mutable package intent and remove the implicit queue-to-document copy during persistence; all UI entry points must update the same authoritative model, and Updates verification/build must consume a consistent snapshot.
3. Break the settings-value dependency cycle with a one-way module dependency while keeping validation and Nix rendering pure and preserving the serialized representation.
4. Audit desired-state validation at load, save, and render as separate trust boundaries; retain defense-in-depth where each boundary needs it, and remove duplication only when it has no distinct contract.
5. Verify existing JSON files still load and round-trip, app and settings edits still persist, save failures remain visible without corrupting the in-memory model, and the complete app check passes.

## System and account query boundary

Account identity and collision checks currently run from the Settings page, and live Nix configuration discovery is shared through the broad apply module. Put each external query behind the narrowest appropriate account/system adapter. The UI should receive a result it can present, not construct commands or interpret operating-system output. Keep query parsing and unavailable/error cases testable independently of GTK.

### Investigation and candidate direction

`SettingsPage::new` calls `getent passwd` to obtain a display name, and the Add account callback calls `id -u` synchronously to refuse names already present on the host. These are account-directory queries, not GTK responsibilities; the latter also runs directly in a click callback. Move command execution and output interpretation into a small account adapter with typed lookup/collision results. Keep formatting and rendering in the page, and define whether account existence lookup is synchronous or delivered asynchronously without blocking GTK.

Live NixOS configuration discovery is also owned by the wrong adapter: `config_value.rs` imports `discover_nixos_sources` from `apply.rs`, even though the helper only inspects the conventional `/etc/nixos` layout and is needed for read-only settings evaluation as well as builds. Move that discovery contract to a narrowly named configuration-source adapter and return a named result rather than an order-sensitive tuple. Candidate-build and settings-value adapters can depend on that shared read-only discovery API without either depending on the other's operation module.

Keep the current policy explicit: only the conventional configuration layout is recognized; flake-only configurations remain unsupported rather than being guessed at. Preserve the existing distinctions between missing live configuration, missing executable, invalid option path, removed option, and evaluation failure. The UI should receive typed outcomes and map them to user-facing text; command strings and raw stderr remain in the adapter/technical detail path.

### Planned work and acceptance

1. Extract account lookup and collision checks from the Settings page; unit-test parsing and typed not-found/error outcomes, and test that the page handles those outcomes without embedding subprocess rules.
2. Move conventional NixOS source discovery out of the candidate-build module into a shared, read-only configuration-source boundary with named fields and a test seam for path layouts.
3. Keep `config_value` responsible for option evaluation/parsing and the build adapter responsible for generation operations; neither should import the other's operation APIs.
4. Ensure host queries that may take noticeable time do not run in GTK callbacks; preserve stale-result protection and main-thread-only widget updates for asynchronous reads.
5. Test standard config present/missing, hardware file optionality, flake-only rejection, account exists/missing, and Nix reader availability/error mapping; run app checks and relevant UI tests.

## UI transaction and background-work lifecycle

The Updates page owns several workflows, but the transaction state should remain authoritative in the pure transaction model. UI rendering should map a state and operation result to the displayed status and enabled actions; it should not independently invent transitions. The detail-page availability/install flow should make its state transitions similarly visible where that improves testability.

Keep one stable action handler for Apply and rollback rather than accumulating callbacks as the button changes meaning. Represent the current action explicitly and dispatch from that handler, or use another lifecycle with explicit disconnection and tests. Background operations must receive plain `Send` inputs and return typed results; GTK widget updates stay on the main context. A shared worker/polling abstraction is worthwhile only if it reduces lifecycle duplication without obscuring cancellation, failures, or operation-specific UI behavior.

### Investigation and candidate direction

`updates::advance` is a pure transition table, but `UpdatesPage` writes several transaction states directly and only uses `advance` for some prepare outcomes. The intended state machine and the UI's operational state can therefore diverge. First make the event table the single authority for legal state changes; keep a single reducer/dispatch point that accepts operation events and returns the next state or rejects an invalid event. Render the step label, button text, sensitivity, and available action from that state instead of scattering those decisions among completion callbacks.

The Apply callback issue needs an early correctness fix within this epic: the original Switch click handler remains connected when `rewire_apply_for_rollback` connects a second handler to the same button. The current mode flag only disables the newly added rollback closure; it does not remove or suppress the original callback. The safer candidate is to connect once and let an explicit UI action mode select Apply versus confirmed rollback. Do not rely on button sensitivity as the only guard against stale operations.

Background operation setup and result polling are repeated across detail availability, Updates preparation, activation, rollback, and storage. Extract only the mechanical thread/channel/main-context handoff if a helper can preserve each operation's typed result and explicit success, failure, disconnect, and button-restoration behavior. Keep action-specific user wording and state transitions with the owning page/application flow. Test pure dispatch/render mapping without a display; reserve GTK interaction tests for the handler lifecycle and page behavior.

The detail page has a separate `InstallState` enum whose transitions are embedded in `start_availability_check` and button-click closures. Treat the UI state machine separately from the Updates transaction machine: add a pure transition function only if it reduces duplicated state mutation and makes stale-selection behavior testable, rather than combining unrelated states into one global transaction model.

### Planned work and acceptance

1. Replace dynamic Apply callback rewiring with one stable action dispatch path; test Apply success/failure, rollback confirmation/cancel/success/failure, and repeated cycles so one click produces exactly one operation.
2. Route transaction events through the pure transition model, including prepare start, prepare result, apply start/result, and rollback result; define how unavailable system configuration and worker failure map to the state model.
3. Extract a pure view-state mapping for status labels and action availability, and test that invalid transitions cannot enable Apply or rollback.
4. Evaluate the detail install-state transition extraction independently; preserve the rule that a late availability result for a different selected package cannot update the current card.
5. Consolidate only the repeated worker completion mechanism where it clarifies thread ownership; test worker success, operation error, channel disconnect/panic, and restoration of disabled controls.
6. Verify all widget access remains on the GTK main context, all worker payloads are plain `Send` data, Apply stays gated by the existing activation policy, and display-dependent tests cover the stable-handler path without invoking real system operations.

## System backend cohesion

The current apply module includes configuration generation and candidate builds, activation, storage upkeep, and generation listing. Group these by cohesive adapter responsibility while retaining explicit safety gates, fixed helper actions, timeouts, and exact generated-config paths. Keep subprocess lifecycle behavior centralized, including stdin streaming and concurrent output collection, rather than maintaining parallel process runners.

Classify system-operation failures at the adapter boundary into typed outcomes; UI pages should map those outcomes to plain-language feedback and optional technical detail. Keep filesystem parsing and formatting pure where practical so generation listing and storage summaries remain easy to test.

### Investigation and candidate direction

Before the split, `apply.rs` was over 1,000 lines and combined candidate generation/build, privileged activation/helper dispatch, storage operations, and read-only generation listing/parsing. `process.rs` already owned concurrent stdout/stderr draining and deadlines, but activation had a second stdin runner and archive creation used unbounded `Command::output()`. These were cohesive extraction opportunities, not a reason to create a module per function.

The resulting `system/` modules separate candidate generation/build (`build.rs`), archive preparation and activation/rollback (`activation.rs`), fixed privileged-helper policy and invocation (`helper.rs`), storage measurement and upkeep (`storage.rs`), and generation listing/parsing (`generation.rs`). The shared process adapter now owns stdin streaming together with bounded concurrent output collection. UI call sites depend on the operation-specific modules rather than a broad `apply` facade.

Preserve security boundaries: helper actions remain fixed enums/subcommands, activation and cleanup continue to fail closed unless the helper plus explicit enablement policy is present, and generated configuration is still packed from the exact expected files. Preserve current plain-language result notes and technical log separation. Typed result structs should remain operation-specific unless there is genuinely shared semantics, not just matching `ok/note/log` fields.

### Planned work and acceptance

1. Define the system adapter dependency map after the shared configuration-source work: candidate generation/build, privileged helper, storage, and generations; ensure UI depends on operation contracts rather than module internals.
2. Move candidate generation/build into its own cohesive module and keep scratch-only config paths and build-only guarantees explicit.
3. Move activation/helper policy and storage commands into their respective owners; retain fixed action enums, authorization gates, bounded operation timeouts, and output-tail behavior.
4. Move generation listing and pure parsers/formatters out of the candidate-build concern; test profile parsing, current-generation detection, ordering, and dates without requiring a live host where practical.
5. Add stdin support to the shared process runner and route activation packaging through bounded process handling; handle spawn, write, wait, timeout, reader, and cleanup failures consistently.
6. Preserve unit and ignored live tests, prove no privileged command can be reached through caller-supplied free-form arguments, and run the complete app check plus the existing opt-in NixOS tests on a suitable host.

## Shell and search presentation

Keep the shell as the window/surface composition and cross-surface navigation point. Isolate store-local and global-search row construction, grouping, and selection mapping into appropriately named UI components if that reduces shell-level policy and callback complexity. Preserve the existing separation between pure ranking and GTK rendering.

Make the lowercased search data visibly a derived cache of catalogue entries, with a clear rebuild relationship whenever the filtered catalogue changes. Search implementation names should distinguish cached/prepared text from source catalogue fields; do not broaden this refactor into ranking or product behavior changes.

### Investigation and candidate direction

`shell.rs` owns both independent search surfaces: a store-local list and a global list spanning packages and settings. The pure scorer is already in `search.rs`, and shared row styling is already in `widgets.rs`; the remaining concentration is GTK-side grouping, rendering, row-to-target mapping, and navigation callbacks. Keep those existing boundaries rather than moving scoring policy into widgets or making the shell own rendering details.

The source entries and their lowered text are currently maintained in parallel `all` and `texts` vectors. Filtering rebuilds both in one callback, but correctness depends on their indices remaining aligned at every call site. A typed search corpus/index that owns entries together with their derived fields is a candidate way to make that invariant structural. Keep this as a focused cache-ownership task: do not change ranking weights, grouping order, result caps, filter defaults, or browse behavior.

The resulting split puts store-local query/list/summary/filter rendering in `ui/store_search.rs` and global query rendering, group expansion, and typed package/setting selection in `ui/global_search.rs`. The shell composes both components and owns cross-surface navigation; the global component owns its sidebar search entry because it shares lifecycle and selection state with the results page.

`search::SearchCorpus` owns each filtered catalogue entry beside its derived lowercased fields, and ranks directly against those cached fields. Replacing the unfree filter constructs and swaps a complete corpus before the store list is rendered again, so entry-to-text index alignment is structural rather than maintained by parallel vectors. Ranking weights and limits, group order and caps, duplicate suppression, filter default, empty-query browse, Escape behavior, Ctrl+K, and package/setting routing remain unchanged. Unit tests cover corpus/filter alignment and typed row-selection mapping.

### Planned work and acceptance

1. Define component contracts and row payloads before moving code; keep navigation decisions in the shell and ranking in the pure search module.
2. Extract store-local and global search rendering into focused UI modules/components, using shared widget builders for consistent row construction.
3. Couple filtered entries and cached search fields in one typed owner, or prove another representation makes alignment impossible to violate; rebuild that owner atomically when the unfree filter changes.
4. Preserve grouped order/caps/expansion, store-local empty-query browse, duplicate suppression, unfree filtering, keyboard shortcut, Escape behavior, and setting/package activation routing.
5. Run pure ranking tests and GUI tree/accessibility tests; add focused view tests for selection payload mapping and filter/cache rebuild consistency.

## Implementation order

1. Resolve state ownership and the settings/desired-state dependency direction first; the other UI flows need a stable contract to depend on.
2. Isolate account and system queries and settle the shared process-adapter boundary before extracting UI worker patterns.
3. Fix the Apply/rollback handler lifecycle, then centralize transaction transitions and background completion behavior.
4. Split backend responsibilities after its public outcomes and process boundary are clear.
5. Extract search presentation from shell composition and clarify derived search-cache ownership.

Each step should preserve behavior and include focused tests at the layer that owns the rule. Run the existing application check and relevant GUI/system integration checks after each coherent extraction; live NixOS and display-dependent tests remain opt-in where their environment is required.
