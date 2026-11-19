# nix-pilot NixOS module — the app's system integration (7wo.3 packaging).
#
# Installs the sanctioned apply helper and a polkit action so the user can
# write the app-owned configuration area and switch/rollback generations,
# all through one controlled, argument-free interface. Nothing here parses or
# edits the user's configuration: the generated config area is the app's own
# and the switch always targets exactly /etc/nix-pilot/generated/configuration.nix
# (decision D21 pointer import; apply-activation.md).
#
# Enable with: services.nix-pilot.enable = true;  (in the user's flake/modules)
{ config, lib, pkgs, ... }:
let
  cfg = config.services.nix-pilot;

  # The root-only operations the desktop app is allowed to request.
  nix-pilot-apply = pkgs.writeShellApplication {
    name = "nix-pilot-apply";
    runtimeInputs = [ pkgs.nixos-rebuild ];
    text = ''
      set -euo pipefail
      area=/etc/nix-pilot/generated
      config=$area/configuration.nix
      ensure_area() {
        # nothing user-owned is created by the app here; the outer config
        # imports the live /etc/nixos/configuration.nix by pointer, never a
        # copy (D21), so this directory is purely app-owned generated output
        mkdir -p "$area"
      }
      case "$1" in
        build)
          ensure_area
          nixos-rebuild build -I nixos-config="$config"
          ;;
        switch)
          ensure_area
          nixos-rebuild switch -I nixos-config="$config"
          ;;
        rollback)
          ensure_area
          nixos-rebuild --rollback
          ;;
        *)
          echo "nix-pilot-apply: unknown subcommand '$1' (build|switch|rollback)" >&2
          exit 2
          ;;
      esac
    '';
  };

  apply-action = pkgs.stdenv.mkDerivation {
    name = "nix-pilot-polkit-action";
    src = ./.; # carries org.nix-pilot.apply.policy
    installPhase = ''
      mkdir -p "$out/share/polkit-1/actions"
      cp org.nix-pilot.apply.policy "$out/share/polkit-1/actions/"
    '';
  };
in
{
  options.services.nix-pilot = {
    enable = lib.mkEnableOption "the nix-pilot system module";
  };

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ nix-pilot-apply apply-action ];

    security.polkit.extraConfig = ''
      polkit.addRule(function(action, subject) {
        if (action.id == "org.nix-pilot.apply" &&
            subject.isInGroup("wheel")) {
          return polkit.Result.YES;
        }
      });
    '';
  };
}