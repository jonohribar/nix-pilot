# nix-pilot NixOS module — the app's system integration.
#
# Installs the sanctioned apply helper and a polkit action so a *future* app
# codepath can have the user write the app-owned configuration area and
# switch/rollback generations through one controlled, argument-free
# interface. Nothing here parses or edits the user's configuration; the
# generated config area is the app's own and the switch always targets
# exactly /etc/nix-pilot/generated/configuration.nix (decision D21 pointer
# import; see apply-activation.md). Per decision D20, no product code calls
# these yet — the helper is the spec + test surface (VM harness) for activation.
#
# Enable with: services.nix-pilot.enable = true;  (in the user's flake/modules)
{ config, lib, pkgs, ... }:
let
  cfg = config.services.nix-pilot;

  # The root-only operations the desktop app is allowed to request.
  nix-pilot-apply = pkgs.writeShellApplication {
    name = "nix-pilot-apply";
    runtimeInputs = [ pkgs.gnutar pkgs.nixos-rebuild pkgs.nix ];
    text = ''
      set -euo pipefail
      area=/etc/nix-pilot/generated
      config=$area/configuration.nix
      ensure_area() { mkdir -p "$area"; }
      # Self-preserve (nix-pilot-r3j): keep a working copy of this helper in
      # /etc/nix-pilot so it survives switches that drop the module, letting
      # the app still roll back. Never fails the switch if it cannot copy.
      self_preserve() {
        mkdir -p /etc/nix-pilot 2>/dev/null || return 0
        cp -f "$0" /etc/nix-pilot/nix-pilot-apply 2>/dev/null || return 0
        chmod +x /etc/nix-pilot/nix-pilot-apply 2>/dev/null || true
      }
      self_preserve || true
      # In-band publish-before-switch (D26): the generated area (a tar of
      # configuration.nix + generated-desired.nix) arrives on stdin and is
      # written into the app-owned area before anything builds/switches. Empty
      # stdin fails closed — there is never a stale or missing config to
      # switch by accident.
      publish() {
        ensure_area
        tar -xzf - -C "$area"
      }
      case "$1" in
        build)
          publish
          nixos-rebuild build -I nixos-config="$config"
          ;;
        switch)
          publish
          nixos-rebuild switch -I nixos-config="$config"
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
(build|switch|rollback|gc|optimise|gc-estimate)" >&2
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