# Wireframe navigation map

The wireframes are a single clickable prototype (`just app proto`): one window,
sidebar + stack pages. This map is the at-a-glance route diagram; the PNGs are
the rendered states.

**Real-app parity (2026-11-20):** the app's live window now matches this layout
— a sidebar (Packages / Settings / Updates / Your system) over a stack. The
sidebar carries the app-wide search at its top (D28, fyq): it searches packages +
settings and always keeps that scope regardless of surface. The store's
scoped search + Hide non-free filter live inside the Packages page. Updates is a
top-level surface only (the Settings home no longer duplicates it). The
Updates page's Prepare-and-build runs the real candidate build (build-only,
store never activated); updates/settings rollback state pages are capture-only
until 7wo.3.

Note: `just app` lists the app recipes; `just app proto` opens the prototype.

## Routes

```mermaid
flowchart LR
    START([Sidebar]) --> HOME[Packages · home]
    HOME -->|"click an app card"| DETAIL[Application detail]
    DETAIL -->|"Back"| HOME
    HOME -->|"type in the Packages search"| RESULTS[App results · grouped]
    RESULTS -->|"clear search / Back"| HOME
    START -->|"type in the sidebar search"| GSTART[Global search · packages + settings]
    GSTART -->|"an app result"| DETAIL
    GSTART -->|"a setting result"| SETCAT[Settings category]
    START -->|"Settings"| SET[Settings · home]
    SET -->|"open a category"| SETCAT
    SETCAT -->|"back"| SET
    START -->|"Updates"| UPD[Updates · ready]
    UPD -->|"Prepare and install"| SUM[Update summary]
    SUM -->|"Apply changes"| APP[Applying]
    APP -->|"finish"| DONE[Done]
    DONE -->|"Go back to yesterday's system"| RB[Rollback]
    RB -->|"finish"| REST[System restored]
    UPD -.->|"build failure"| BF[Build failure · nothing changed]
    APP -.->|"apply failure"| AF[Apply failure · nothing half-applied]
    UPD -.->|"system will not boot"| BOOT[Boot recovery · "Start with yesterday's system"]
    BOOT -->|"I'm back on an earlier system"| REST
```

## States (rendered)

![Store home](proto-home.png)

![Search results](proto-search.png)

![Application detail](proto-detail.png)

![Settings](proto-settings.png)

![Updates — ready](proto-updates.png)

![Updates — summary](proto-updates-summary.png)

![Updates — applying](proto-updates-applying.png)

![Updates — done](proto-updates-done.png)

![Updates — build failure](proto-updates-build-fail.png)

![Updates — apply failure](proto-updates-apply-fail.png)

![Updates — rollback](proto-updates-rollback.png)

![Updates — restored](proto-updates-restored.png)

![Updates — boot recovery](proto-updates-boot.png)

## Decisions the prototype encodes

- Command-line tools appear only as a labelled group in search, never as app tiles (D16).
- Unconfirmed candidates sit in their own "Unverified" group, never as tiles (D18).
- Install is per-user by default, system-wide only when chosen (D9).
- Settings surface is system-only with pointers to the desktop's own tools (D14); it carries the plain-language recovery card that ships before any failure (failure-and-recovery.md).
- Updates is one transaction: prepare → review summary → apply (atomic) → done, with explicit build-failure ("nothing changed"), apply-failure ("nothing half-applied"), rollback ("go back to yesterday's system") and boot-recovery states (failure-and-recovery.md).

## Regenerating

```sh
just app build
just app wireframes            # all states, or named ones: `just app wireframes detail updates-summary`
```
