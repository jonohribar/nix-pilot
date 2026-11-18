# nix-pilot — command hub, organised by folder.
#
#   just app                   # list the app/Justfile recipes
#   just app proto             # run the clickable wireframe prototype
#   just app build|run|wireframes|smoke …
#
#   just phase0                # list the experiments/phase0/Justfile recipes
#   just phase0 stats          # catalogue index stats
#   just phase0 search …       # search the catalogue
#   just phase0 measure 300    # full-set export (needs nix)
#
# Requires: just (`nix profile install nixpkgs#just`) and node.
# App recipes are wrapped in the nix dev shell (app/shell.nix).

# list all root recipes
default:
    just --list

# enter the app dev shell (cargo + GTK4 + libadwaita)
app-shell:
    nix-shell app/shell.nix

# app recipes — `just app` lists them (no shell needed); actual recipe runs
# are wrapped in the nix dev shell so cargo/GTK4 are available
app *args:
    @if [ -n "{{args}}" ]; then nix-shell app/shell.nix --run 'cd app && just {{args}}'; else cd app && just; fi

# catalogue recipes — `just phase0` to list; node only (measure needs nix)
phase0 *args:
    cd experiments/phase0 && just {{args}}