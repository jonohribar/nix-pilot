# nix-pilot — command hub, organised with just modules (`mod`).
#
#   just app build           # module "app" -> app/Justfile (auto-cwd = app/)
#   just app proto
#   just app wireframes
#   just experiments stats   # module "experiments" -> experiments/catalogue/Justfile
#   just experiments search docker
#   just app-shell           # drop into the dev shell
#   just                     # list everything
#
# App recipes run from app/ and self-wrap in the nix dev shell unless you are
# already inside it. Requires: just (`nix profile install nixpkgs#just`) + node.

# app desktop client recipes (app/Justfile)
mod app 'app/Justfile'

# catalogue experiments recipes (experiments/catalogue/Justfile)
mod experiments 'experiments/catalogue/Justfile'

# enter the app dev shell (cargo + GTK4 + libadwaita)
app-shell:
    nix-shell app/shell.nix