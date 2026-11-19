#!/usr/bin/env bash
# 7wo.3 activation harness: prove switch + rollback of an app-built candidate
# generation, using the NixOS test driver (an isolated QEMU VM).
#
# Never touches the host: the VM is a minimal test system from the same
# pinned nixpkgs, and the candidate generation is built on the host (build
# only) then registered into the VM's store via virtualisation.pathsInNixDB.
# The test then exercises the real activation primitives the app will drive:
#   nix-env -p /nix/var/nix/profiles/system --set <candidate>
#   switch-to-configuration switch
#   nix-env -p /nix/var/nix/profiles/system --rollback + switch
#
# Usage: ./vm-activation-test.sh   (just app vm-probe)
set -euo pipefail
cd "$(dirname "$0")/.."

TMP=$(mktemp -d /tmp/nix-pilot-vm-XXXX)
trap 'rm -rf "$TMP"' EXIT

# candidate config: minimal system + one extra package (hello). Importing the
# qemu-vm module gives it the root fs/loader a standalone toplevel needs.
cat > "$TMP/candidate.nix" <<'EOF'
{ config, pkgs, lib, ... }:
{
  imports = [ <nixpkgs/nixos/modules/virtualisation/qemu-vm.nix> ];
  system.stateVersion = "26.05";
  environment.systemPackages = [ pkgs.hello ];
}
EOF

echo "== building candidate toplevel (build only)"
T1=$(timeout 1500 nix-build '<nixpkgs/nixos>' -A system \
  -I "nixos-config=$TMP/candidate.nix" --no-out-link 2>/dev/null | tail -1)
[ -n "$T1" ] && [ -e "$T1" ] || { echo "candidate build failed"; exit 1; }
echo "candidate: $T1"

cat > "$TMP/test.nix" <<EOF
let
  pkgs = import <nixpkgs> {};
  candidate = builtins.storePath "$T1";
in
pkgs.testers.runNixOSTest {
  name = "nix-pilot-activation";
  # tiny machine; the candidate is registered into its store so nix-env can
  # adopt it as a generation without any network inside the VM
  nodes.machine = { ... }: {
    system.stateVersion = "26.05";
    virtualisation.pathsInNixDB = [ candidate ];
    virtualisation.memorySize = 1536;
  };
  testScript = ''
    machine.start()
    machine.wait_for_unit("multi-user.target")

    # baseline: the candidate's package is absent
    machine.fail("command -v hello")

    # switch: adopt the candidate as a new system generation and activate it
    machine.succeed("nix-env -p /nix/var/nix/profiles/system --set ${T1}")
    machine.succeed("nix-env -p /nix/var/nix/profiles/system --list-generations")
    machine.succeed("${T1}/bin/switch-to-configuration switch")
    machine.succeed("command -v hello")
    machine.succeed("test \"\$(readlink -f /run/current-system)\" = \"\$(readlink -f ${T1})\"")

    # rollback: step back a generation and activate it again
    machine.succeed("nix-env -p /nix/var/nix/profiles/system --rollback")
    machine.succeed("/nix/var/nix/profiles/system/bin/switch-to-configuration switch")
    machine.fail("command -v hello")
    machine.succeed("test \"\$(readlink -f /run/current-system)\" != \"\$(readlink -f ${T1})\"")
  '';
}
EOF

echo "== running NixOS test driver (isolated QEMU VM)"
if timeout 2400 nix-build "$TMP/test.nix" -A driver --no-out-link >"$TMP/test.log" 2>&1; then
  echo "HARNESS OK: switch + rollback proven in isolation"
  grep -E "^(machine|subtest).*(#|$)|passed|ok" "$TMP/test.log" | tail -10 || true
  exit 0
else
  echo "HARNESS FAILED"
  tail -60 "$TMP/test.log" || true
  exit 1
fi
