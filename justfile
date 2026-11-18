# nix-pilot command hub.

default:
    @just --list

mod app 'app/Justfile'
mod experiments 'experiments/catalogue/Justfile'

app-shell:
    nix-shell app/shell.nix