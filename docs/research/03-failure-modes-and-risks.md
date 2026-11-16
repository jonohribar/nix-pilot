# Failure Modes and Risks

> Research note · 2026-11-16 · Status: current Analysis of why prior attempts failed, and where this project is most likely to fail. Facts in `01-prior-art.md` and `02-catalogue-feasibility.md`; this file is analysis.

## In one paragraph

History shows five recurring failure patterns, and this project is immune to only one of them. The generic option-tree editor approach is structurally dead and the brief correctly avoids it. The remaining four — maintenance collapse, the abstraction/knowledge gap at failure time, the catalogue metadata gap, and scope — are live risks. Maintenance collapse is the most likely cause of death; every predecessor, including the most popular one, died there. The knowledge gap is the deepest design risk: the brief's own "hide Nix" design is what amplifies it at the exact moment of failure.

## Failure patterns from prior art (in order of lethality)

1. **Single-maintainer burnout with no institutional home.** nix-gui and nix-software-center both attracted community sympathy and stars, neither attracted reliable co-maintainers, neither was adopted upstream. NixOS core keeps a polite distance. This project must answer "who maintains this in three years?" before Phase 2.
2. **The generic option-tree editor fails.** Rendering the full NixOS option tree (nix-gui) collapses because: the UI becomes an information dump; the language's expressiveness (functions, `lib.mkMerge`, submodules, aliases) cannot be round-tripped — even Nix itself cannot say where a merged option value came from; unrenderable types fall back to raw Nix editing, which defeats the audience. The brief avoids this by owning the config and curating settings. Correct call.
3. **Mixing handwritten and generated config reverts to config-as-state.** rnhmjoj's argument, with /etc/resolv.conf as the canonical example: when multiple frontends touch one backend, they fight; "it works well enough only if you always stick to just one front-end". The brief's answer — application owns the config it generates, existing configs out of scope — is coherent, but it is a wall, not a bridge (see knowledge gap below).
4. **The metadata gap is structural, not incidental.** Detail in `02-catalogue-feasibility.md`. No predecessor solved it; the nixpkgs RFC has been open a decade.
5. **Abstraction leaks are inevitable.** Even trivial NixOS state (a firewall port list defined across modules) cannot be faithfully edited by a GUI. Community consensus: "there will come a point where you outgrow higher-level tooling" (https://discourse.nixos.org/t/nixos-is-the-best-linux-back-end-ever-designed-but-it-cant-become-mainstream-without-a-front-end/46526/32). The product question is not whether the leak exists but how users cross it.

## Risk A — Language and abstraction mismatch

- **Reality:** the mismatch is broader than "options are hard to render". NixOS concepts are *dependencies*: installing a media player drags in a sound server; enabling SSH implies a socket, firewall rules, a user; a desktop environment implies drivers (nouveau vs nvidia) and hundreds of packages. A GUI's flat "installed = yes" model diverges from Nix's graph model exactly where real systems get complicated.
- **The settings surface is a second product.** Networking (networkmanager vs wpa_supplicant vs systemd-networkd), sound (pipewire migration), drivers, printers — each is a product-authored workflow with a long tail. No predecessor attempted this (nix-gui chose the option tree because curating was *more* work). The brief acknowledges it (product-authored settings) but does not carry the cost in its risk table.
- **Mitigation already in the brief:** curated settings instead of option trees; outcome language; "application needs additional setup" as a link, not a guess. Good. What is missing is treating the settings schema as a maintained product surface with its own lifecycle.

## Risk B — Maintenance burden

- The project is four moving surfaces, each tracking Nixpkgs drift forever:
  1. the catalogue generator and its pinned-revision lifecycle;
  2. the settings workflow schema (option renames in Nixpkgs are constant);
  3. the desired-state-to-NixOS backend;
  4. the UI.
- The brief's own risk row "catalogue and system revisions differ" is not an edge case; over time it is the default state. Renames, removals and behavioural changes in Nixpkgs will break joins and settings silently.
- **No predecessor survived one maintainer for three years.** Nothing in the phased plan changes that. Phase 0/1/2 shorten the path to a demo, not the sustain horizon.

## Risk C — Knowledge gap in the target audience

- The brief's design — hide Nix syntax, concepts and error surfaces — works until the inevitable failure, and then it becomes the failure: recovery in NixOS *is* a Nix concept (rebuild, generations, boot selection, /nix/store).
- Two concrete failure classes the brief under-specifies:
  1. **Build failure at apply time.** The app promises a plain-language transaction summary and rollback controls, but a failed `nixos-rebuild` produces errors in Nix/dependency/linker language. Who translates them? Phase 1's criterion "the GUI can explain each operation without exposing Nix concepts" is too weak; the test should be "a user who hits a build failure can recover without a terminal".
  2. **The machine does not boot.** Recovery must happen before the GUI runs; in-app rollback controls cannot cover it. This needs a designed boot-time story (older-generation selection, shipped documentation) that is *not* part of the app.
- Branding matters: "hides Nix" creates a wall the user hits at the worst moment. "The details are one click away and always shown on error" turns the knowledge gap into an on-ramp. This is a product decision, not a technical one, and it should be made explicitly.

## Risk D — Catalogue reality (Phase 0)

- The classification policy and confidence tiers are sensible but unmeasured. Honest prior is pessimism (ten-year-old RFC, unfree coverage, join complexity — see `02`).
- The "no manual curation" rule has a UX cost: generated-only stores look empty next to curated ones (GNOME Software, Discover). The temptation will be to let "safe product rules" quietly become a de facto allowlist. The rule needs a precise statement (see `04-recommendations.md` item 4).
- The plan must treat "coverage too low" as a legitimate research outcome that changes scope, not a bug to be fixed by loosening the curation rule.

## Risk E — Scope (three products in one)

- The brief bundles: a data platform (catalogue generator), an OS-level transaction engine (desired state → evaluate → build → apply → rollback), and a consumer UX (Settings + Software + Updates). Each is a multi-year project alone; history shows teams of one stalling on just one.
- The phased plan is the correct mitigation, and the brief's scope boundaries (no existing configs, no raw Nix editing in normal mode) are the right cuts. The remaining scope risk is the *settings* surface, which is unbounded and has no precedent to lean on.

## Bottom line

The brief fixes the failure mode that killed nix-gui (generic editors), has a credible plan for the metadata gap, and has no answer yet for maintenance sustainability or the failure-time knowledge gap. Both gaps are addressable on paper before any UI work begins; recommendations in `04-recommendations.md`.
