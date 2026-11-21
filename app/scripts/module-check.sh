#!/usr/bin/env bash
# Packaging check: the nix-pilot NixOS module must evaluate cleanly when
# enabled. Uses nixos-rebuild dry-build of a scratch, VM-shaped config that
# imports the module (evaluation only — nothing builds, touches the host, or
# activates). Run via: just app module-check
set -euo pipefail
MOD="$(cd "$(dirname "$0")/../nixos-module" && pwd)/nix-pilot.nix"
TMP=$(mktemp -d /tmp/nix-pilot-modcheck-XXXX)
trap 'rm -rf "$TMP"' EXIT
cat > "$TMP/config.nix" <<EOF
{ config, pkgs, lib, ... }:
{
  imports = [ <nixpkgs/nixos/modules/virtualisation/qemu-vm.nix> $MOD ];
  system.stateVersion = "26.05";
  services.nix-pilot.enable = true;
}
EOF
timeout 900 nixos-rebuild dry-build -I "nixos-config=$TMP/config.nix" 2>&1 | tail -3
echo "module-check ok"