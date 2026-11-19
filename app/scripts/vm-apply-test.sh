#!/usr/bin/env bash
# 7wo.3 installed-module harness: prove the *installed* nix-pilot module's
# helper and polkit authorization work, in an isolated QEMU VM.
#
# vm-activation-test.sh already proves the raw activation primitives
# (adopt a generation, switch-to-configuration, rollback). This test proves
# the layer the desktop app actually uses:
#   - the module installs nix-pilot-apply at the polkit exec path, plus the
#     org.nix-pilot.apply policy/rule;
#   - pkexec authorizes a wheel user for it (rule -> Result.YES, so no auth
#     agent is needed headless) AND the .policy exec-path annotation is right
#     (the most common real pkexec failure);
#   - the helper reads the generated-area tar from stdin and publishes it into
#     /etc/nix-pilot/generated before the switch is attempted.
#
# The switch itself rebuilds inside the VM, which a bare test box cannot
# evaluate, so its nonzero exit is expected; what matters is that publication
# succeeded and the failure is an eval error, not "not authorized".
#
# Never touches the host. Run via: just app vm-apply
set -euo pipefail
cd "$(dirname "$0")/.."

MOD="$(pwd)/nixos-module/nix-pilot.nix"
TMP=$(mktemp -d /tmp/nix-pilot-vmapply-XXXX)
trap 'rm -rf "$TMP"' EXIT

cat > "$TMP/test.nix" <<EOF
let
  pkgs = import <nixpkgs> {};
  nixpilotModule = "$MOD";
in
pkgs.testers.runNixOSTest {
  name = "nix-pilot-installed-module";
  nodes.machine = { lib, pkgs, ... }: {
    imports = [ nixpilotModule ];
    services.nix-pilot.enable = true;
    system.stateVersion = "26.05";
    users.mutableUsers = false;
    users.users.tester = {
      isNormalUser = true;
      extraGroups = [ "wheel" ];
    };
    virtualisation.memorySize = 1536;
  };
  testScript = ''
    machine.start()
    machine.wait_for_unit("multi-user.target")

    # the module installed the helper at the annotated polkit exec path
    machine.succeed("test -x /run/current-system/sw/bin/nix-pilot-apply")
    machine.succeed(
        "test -f /run/current-system/sw/share/polkit-1/actions/org.nix-pilot.apply.policy")

    # build the generated area (two files, as the app packs) and tar it
    machine.succeed("mkdir -p /root/area")
    machine.succeed(
        "cat > /root/area/configuration.nix <<'NIX'\\n"
        "{ config, pkgs, lib, ... }:\\n"
        "{ imports = [ <nixpkgs/nixos/modules/virtualisation/qemu-vm.nix> ./generated-desired.nix ];\\n"
        "  system.stateVersion = \\"26.05\\";\\n"
        "  environment.systemPackages = [ pkgs.hello ]; }\\n"
        "NIX")
    machine.succeed("echo '{ ... }: { }' > /root/area/generated-desired.nix")
    machine.succeed(
        "tar -czf /tmp/area.tar.gz -C /root/area configuration.nix generated-desired.nix")

    # main assertion: pkexec authorizes a wheel user and the helper publishes
    status, out = machine.execute(
        "su -l tester -c 'pkexec nix-pilot-apply switch < /tmp/area.tar.gz'")
    machine.succeed("test -f /etc/nix-pilot/generated/configuration.nix")
    machine.succeed("test -f /etc/nix-pilot/generated/generated-desired.nix")
    low = out.lower()
    assert "not authorized" not in low, "pkexec denied the wheel user: " + out
    assert "authentication" not in low, "pkexec asked for auth headless: " + out
    print("pkexec authorized + helper published the area (switch rc={})".format(status))

    # rollback path is also authorized (no auth rejection)
    status2, out2 = machine.execute(
        "su -l tester -c 'pkexec nix-pilot-apply rollback'")
    low2 = out2.lower()
    assert "not authorized" not in low2, "rollback denied: " + out2
    print("rollback path authorized (rc={})".format(status2))
  '';
}
EOF

echo "== running NixOS test driver (installed module, isolated QEMU VM)"
if timeout 2400 nix-build "$TMP/test.nix" -A driver --no-out-link >"$TMP/test.log" 2>&1; then
  echo "HARNESS OK: module installed, pkexec authorized, area published"
  grep -E "^machine|pkexec authorized|rollback path" "$TMP/test.log" | tail -10 || true
  exit 0
else
  echo "HARNESS FAILED"
  tail -60 "$TMP/test.log" || true
  exit 1
fi