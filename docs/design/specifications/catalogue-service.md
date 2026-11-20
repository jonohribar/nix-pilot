# Catalogue Service Plan (7wo.4)

Status: plan — deliberately not started until 7wo.3 real-host activation is
verified. The app today consumes a locally generated
`catalogue-index.json` (+ sha256 sidecar, env/URL loading already shipped);
this plan makes the catalogue a published, versioned, honest service.

## Goal

A normal user's fresh install gets a current catalogue from a published
source: versioned manifest (nixpkgs revision, schema version, sha256), served
and cached, with honest "not available on your system" states (R07) and a
newer-version signal.

## Slices (planned order)

1. **Publication pipeline** — the generator runs on a schedule against a
   pinned nixpkgs and publishes `catalogue-index.json` + `manifest.json`
   (nixpkgs rev, schema version, sha256, built-at). Reuses the existing
   generator + sidecar emission; adds the manifest and versioning rules.
2. **Distribution + binary-cache evaluation** — hosting/cache design; the
   unfree-metadata sub-question (decision D2/D17: Hydra does not build unfree
   apps, so their metadata/icons need a second source — arbitrary binary-cache
   inspection evaluation, cost/coverage).
3. **Freshness + version signal** — the app knows when a newer catalogue (or a
   newer version of an app) exists; this is the feeder for the version-selection
   feature (9f3.9, decided D27).

## Gating

- Feeders: 9f3.9 per-app version channels sit on slice 3.
- Not blocked by 7wo.3; slices 1–2 are independent of activation.
- Non-goals: no hand curation (D4); no telemetry (R6).

## Acceptance

Fresh machine → app fetches catalogue by URL, verifies sha256, shows honest
availability; store shows current versions with a "newer available" signal
where the channel index proves it.