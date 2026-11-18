# nix-pilot — project command hub
# Requires: `just` (installed via `nix profile install nixpkgs#just`) and node.
# App recipes wrap the nix dev shell (app/shell.nix); catalogue recipes run node.

# list all recipes
default:
    just --list

# enter the app dev shell (cargo + GTK4 + libadwaita)
app-shell:
    nix-shell app/shell.nix

# build the desktop client
app-build:
    nix-shell app/shell.nix --run 'cd app && just build'

# run the Software Store app against the generated catalogue index
app-run:
    nix-shell app/shell.nix --run 'cd app && just run'

# run the clickable wireframe prototype (test navigation)
app-proto:
    nix-shell app/shell.nix --run 'cd app && just proto'

# render wireframe states to PNGs (all, or named: detail search …)
app-wireframes *states:
    nix-shell app/shell.nix --run 'cd app && just wireframes {{states}}'

# headless smoke test of the app window
app-smoke:
    nix-shell app/shell.nix --run 'cd app && just smoke'

# --- catalogue helpers (node) ---

# search the catalogue index
search *terms:
    cd experiments/phase0 && just search {{terms}}

# index stats
stats:
    cd experiments/phase0 && just stats

# rebuild the heuristic catalogue index from records.json
generator:
    cd experiments/phase0 && just generator

# overlay evidence tiers onto the index (reads results/evidence*.jsonl)
reclassify:
    cd experiments/phase0 && just reclassify

# post-evidence known-apps sanity check
validate:
    cd experiments/phase0 && just validate

# full-set metadata export (measure.sh, needs nix)
measure n:
    cd experiments/phase0 && just measure {{n}}

# serve the browsable catalogue viewer (http://localhost:8137/view.html)
viewer:
    cd experiments/phase0 && just viewer
