# User Needs

Who the product is for and the outcomes it must deliver. This document does not describe how the product is built.

## Terms used here

- **Persona** — a user type, described by context and goals.
- **User story** — one need, stated as "As a <persona>, I want <capability>, so that <benefit>." Each story carries a stable id used to trace requirements.
- **Journey** — the end-to-end sequence a persona goes through to reach an outcome, including what happens when something goes wrong. A journey can pass through several user stories.

## Personas

### Primary — the daily user (non-technical)

A person who does not program and does not want to learn Nix. They are comfortable with an ordinary desktop operating system and expect it to work without reading documentation or touching configuration files. They want to find applications by browsing or searching, install them with a button, configure the computer through ordinary settings screens, update it, and recover if something goes wrong. The product must never require them to understand Nix, its language, its configuration files or its error output.

### Secondary — first time on Linux (from Windows or macOS)

Accustomed to a store where installing is one click, a control panel for settings, and automatic updates. Unlikely to look outside the product for common tasks. They expect familiar names and a familiar mental model and have a low tolerance for jargon and for unexplained differences. They care that their documents and settings survive updates and that the system will not surprise them.

### Secondary — Linux user new to NixOS (distro hopper)

Comfortable with Linux and with package managers in general (apt, dnf, pacman, Flatpak). May already use GNOME Software or KDE Discover. Values speed, control, and not fighting the system. They accept more technical detail than the primary user but still have no interest in the Nix language. They want fast install and removal, transparency into what a change touches, the option of more advanced settings without them cluttering the basics, and a safe rollback path so experiments are low-risk.

### Adjacent — experienced NixOS user (writes Nix)

Not the product's target. Already manages their own Nix configuration and files. They may use the product to browse the generated catalogue as a reference, to drive updates and recovery, to help a non-technical relative, or to inspect what the product generates. Their needs are limited and must never degrade the primary user's experience.

## What the product must do

1. **Settings.** Approachable workflows for common system tasks: networking, Bluetooth, display, sound, printers, users, power and remote access.
2. **Software.** Discover, search, view details, install, uninstall and update applications.
3. **Updates and recovery.** Show what updates are available, explain what they change in ordinary language, apply them, and provide a clear rollback and recovery path.
4. **Search.** One search across settings, applications and help, with results grouped by type.

## User stories

### Primary — the daily user

- **P1.** As a non-technical user, I can install an application with one button, so that I never have to do anything technical.
- **P2.** As a non-technical user, I can search in my own words and get the setting, application or help I need, so that I do not need to know technical names.
- **P3.** As a non-technical user, I can understand what a change will do before I apply it, so that I am not surprised by its effect.
- **P4.** As a non-technical user, I can undo or recover from a bad change without a terminal, so that a mistake does not leave me stuck.
- **P5.** As a non-technical user, I can update the system and know what changed, so that my software stays current and safe.
- **P6.** As a non-technical user, I can configure common things such as networking, display, sound, printers and users in settings screens, so that I never touch a configuration file.
- **P7.** As a non-technical user, I am never shown Nix concepts, so that I am not confused by implementation details.

### Secondary — first time on Linux

- **M1.** As a Windows or macOS user, I find the settings and the store instantly familiar, so that I do not feel lost in a new operating system.
- **M2.** As a Windows or macOS user, I get plain-language explanations instead of jargon, so that I do not have to learn a new vocabulary.
- **M3.** As a Windows or macOS user, I know my documents and settings are safe and survive updates, so that I trust the system.
- **M4.** As a Windows or macOS user, I can take updates automatically or on my own schedule, so that I stay in control.

### Secondary — Linux user new to NixOS

- **L1.** As a Linux user, I can install and remove software quickly, so that I am not blocked by slow or unfamiliar steps.
- **L2.** As a Linux user, I can see what a change touches (files, services, packages), so that I trust what the system is doing.
- **L3.** As a Linux user, I can reach more technical settings when I want them, without them cluttering the basics.
- **L4.** As a Linux user, I can roll back to a previous working state, so that experiments are safe.

### Adjacent — experienced NixOS user

- **N1.** As a NixOS user who writes Nix, I can use the product read-only to browse the generated catalogue, so that it never changes my own configuration.
- **N2.** As a NixOS user who writes Nix, I can see a plain-language summary of what an update or change would do, so that I can decide whether to do it in the product or myself.
- **N3.** As a NixOS user who writes Nix, I can expose raw configuration and diagnostics in a developer mode, so that I can trace what the product generates.

Stories N1–N3 must never compromise the primary experience; developer mode is hidden by default.

## Journeys

Journeys are narratives; each can pass through several of the stories above.

### Journey A — first boot for a migrating user

A user on Windows switches their machine to NixOS. The product is the first thing they meet. They search for an application they used before, install it in one step, and open settings to turn on a printer and connect to their Wi-Fi, finding each where they expected from the old operating system. They start an update, see in plain language what it changes, and apply it. The key test is that nothing in the product references Nix or asks them to do anything they did not do on their previous computer. (Stories P1, P5, M1, M2, M3, L2.)

### Journey B — finding "ssh" by ordinary words

A user wants to let another computer connect to theirs. They type "ssh" into the single search box. Results come back grouped by type: a setting "Remote access"; a program "OpenSSH"; and a short explanation of what remote access is. They read the explanation, enable the setting, and the product states clearly what has been turned on. (Stories P2, P3.)

### Journey C — an application that needs extra setup

A user installs an application that needs one more step, such as a scanner interacting with the printing service. Instead of a blank install, the product tells the user a follow-up step is needed and links directly to the relevant setting. The link comes from machine-readable integration data or an explicit product rule; it is never guessed from a name. (Stories P1, P6.)

### Journey D — something went wrong

A user applies a change that fails. The product explains, in plain language, what went wrong and what they can do, and offers to return to the previous working state. The user accepts, the system is restored, and the user did not open a terminal or read Nix output at any point. (Stories P4, L4; see "Recovery as a need" below.)

## Derived requirements

The derived user requirements REQ-1…REQ-8 from these stories are maintained in [requirements.md](requirements.md), the single home for the requirement set. The story ids P, M, L and N above trace to them.

## What the user must never have to do

- Learn Nix, Nix expressions, flakes or the module system.
- Edit configuration files or read Nix output.
- Understand why an application is a library rather than a program they can install, or why two similarly named things differ.

## Scope of the first version

The first version targets a clean, application-managed installation. Supporting arbitrary existing configurations, importing user configurations, and direct Nix editing are out of scope for this version. Users who rely on them are not the initial audience.

## Recovery as a need

The normal experience hides Nix by design, but a technical failure must not strand the user. When a change fails to apply, or the system will not start, the user still needs a plain-language explanation and a path back to a working system. Recovery is not part of the everyday workflow, but it is a user need: the product must not depend on the user understanding Nix to get out of trouble.