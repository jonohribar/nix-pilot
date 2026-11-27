# nix-pilot NixOS module — the app's system integration.
#
# Enable with: services.nix-pilot.enable = true;  (in the user's flake/modules)
#
# D33 model (config-authority.md): the app writes its own module copy and a
# generated state module into the user's config tree (`nix-pilot/module.nix`
# and `nix-pilot/state.nix`, carrying packages, settings, accounts, keys),
# and the user's config imports them — one config, one switch, fully pure:
# the state travels with the repo, so the config stays reproducible and
# portable. This module provides only the sanctioned apply helper (which
# switches the user's own config) and the polkit action.
{ config, lib, pkgs, ... }:
let
  cfg = config.services.nix-pilot;

  # The root-only operations the desktop app is allowed to request.
  nix-pilot-apply = pkgs.writeShellApplication {
    name = "nix-pilot-apply";
    runtimeInputs = [ pkgs.nixos-rebuild pkgs.nix ];
    text = ''
      set -euo pipefail
      # Self-preserve (nix-pilot-r3j): keep a working copy of this helper in
      # /etc/nix-pilot so it survives switches that drop the module, letting
      # the app still roll back. Never fails the switch if it cannot copy.
      self_preserve() {
        mkdir -p /etc/nix-pilot 2>/dev/null || return 0
        cp -f "$0" /etc/nix-pilot/nix-pilot-apply 2>/dev/null || return 0
        chmod +x /etc/nix-pilot/nix-pilot-apply 2>/dev/null || true
      }
      self_preserve || true
      # D33: the app writes its nix-pilot/state.nix module into the user's
      # config tree and this helper switches that same config — one config,
      # one pure rebuild (no generated area, no stdin archive). The config
      # directory is the only accepted argument.
      require_config_dir() {
        [ $# -ge 2 ] || { echo "nix-pilot-apply: '$1' needs a config directory" >&2; exit 2; }
        [ -d "$2" ] || { echo "nix-pilot-apply: not a directory: $2" >&2; exit 2; }
      }
      case "$1" in
        switch)
          require_config_dir "$@"
          if [ -f "$2/flake.nix" ]; then
            nixos-rebuild switch --flake "$2"
          else
            nixos-rebuild switch -I "nixos-config=$2/configuration.nix"
          fi
          ;;
        rollback)
          ensure_area
          nixos-rebuild --rollback
          ;;
        # Storage upkeep: fixed, argument-free actions only. Estimates
        # and actual cleanup never take parameters from the caller; the app
        # asks for a fixed action and the helper decides the exact command.
        gc)
          nix-collect-garbage --delete-old
          ;;
        optimise)
          nix store optimise
          ;;
        gc-estimate)
          # Count of paths that --delete-old would remove (old generations only).
          nix-collect-garbage --delete-old --dry-run
          # Approximate reclaim size: total size of all dead store paths.
          # Scope differs from --delete-old; label honestly as approximate.
          dead_paths=$(nix-store --gc --print-dead 2>/dev/null || true)
          if [ -n "$dead_paths" ]; then
            approx_bytes=$(echo "$dead_paths" | xargs -r nix path-info --closure-size 2>/dev/null | awk '{sum+=$1} END {if (sum>0) printf "%.0f", sum; else print 0}')
            echo "~$approx_bytes bytes would be freed (approximate, all dead paths)"
          else
            echo "~0 bytes would be freed (approximate, all dead paths)"
          fi
          ;;
        *)
          echo "nix-pilot-apply: unknown subcommand '$1' \
(switch|rollback|gc|optimise|gc-estimate)" >&2
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

  appBinaries = [ nix-pilot-apply apply-action ];
in
{
  options.services.nix-pilot = {
    enable = lib.mkEnableOption "the nix-pilot system module";
  };

  # The managed surface (packages, settings, accounts, keys) is not processed
  # here: the app renders it into a generated `nix-pilot/state.nix` module the
  # user's config imports (D33), keeping this module pure and static.
  config = lib.mkIf cfg.enable {
    environment.systemPackages = appBinaries;

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