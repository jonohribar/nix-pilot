# nix-pilot command hub.

# List all recipes with their descriptions
help:
    @just --list --unsorted 

# Desktop client (app/justfile)
mod app 'app/justfile'

# Catalogue experiments (experiments/catalogue/justfile)
mod experiments 'experiments/catalogue/justfile'