#!/usr/bin/env bash
# 7wo.3 activation harness: proves switch/rollback in an isolated QEMU VM.
#
# The VM is a MINIMAL TEST SYSTEM built from the same pinned nixpkgs — it is
# never the host's real config and never touches /etc/nixos or the live
# system. The app's candidate generation (built on the host by apply.rs) is
# activated inside the VM through the shared host store, so the real
# nixos-rebuild switch/rollback invocation is exercised without any host
# risk (the same "explicit scratch dir / never switch a live system" rule).
#
# Current scope (first iteration): build + boot the minimal VM under KVM and
# prove SSH reachability. Switch/rollback assertions land next within 7wo.3.
#
# Usage: ./vm-activation-test.sh
set -euo pipefail
cd "$(dirname "$0")/.."
TMP=$(mktemp -d /tmp/nix-pilot-vm-XXXX)
trap 'rm -rf "$TMP"' EXIT
cat > "$TMP/config.nix" <<EOF
{ config, pkgs, lib, ... }:
{
  system.stateVersion = "26.05";
  users.users.test = {
    isNormalUser = true;
    openssh.authorizedKeys.keys = [ "$(cat ~/.ssh/id_ed25519.pub 2>/dev/null || cat ~/.ssh/id_rsa.pub 2>/dev/null || echo '')" ];
  };
  users.users.root.openssh.authorizedKeys.keys = [ "$(cat ~/.ssh/id_ed25519.pub 2>/dev/null || cat ~/.ssh/id_rsa.pub 2>/dev/null || echo '')" ];
  services.openssh.enable = true;
  services.openssh.settings.PermitRootLogin = "yes";
  # VM-only options live under vmVariant (applied only to the build-vm eval)
  virtualisation.vmVariant = {
    virtualisation.forwardPorts = [ { host.port = 2222; guest.port = 22; } ];
    virtualisation.memorySize = 1536;
  };
}
EOF
echo "== building minimal test VM (this can take a few minutes)"
timeout 1800 nixos-rebuild build-vm -I "nixos-config=$TMP/config.nix" --no-link 2>&1 | tail -3 || { echo "VM build failed"; exit 1; }
# build-vm --no-link prints the result dir on the last line when -v; find it
RESULT=$(ls -td /nix/store/*-nixos-vm 2>/dev/null | head -1)
[ -n "$RESULT" ] || { echo "no vm result found"; exit 1; }
RUNNER=$(ls "$RESULT"/bin/run-*-vm 2>/dev/null | head -1)
echo "VM built: $RESULT"
echo "runner: $RUNNER"
echo "== booting VM under KVM (headless, ssh on :2222)"
# forwardPorts in the config already wires hostfwd :2222->22; passing any
# QEMU_NET_OPTS here would add a second -netdev and qemu would refuse it
QEMU_OPTS="-enable-kvm -m 1536" \
  timeout 300 "$RUNNER" -nographic >"$TMP/vm.log" 2>&1 &
VM_PID=$!
echo "== waiting for ssh (up to 5 min)"
ok=0
for i in $(seq 1 60); do
  sleep 5
  if timeout 5 ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o ConnectTimeout=3 -p 2222 root@localhost 'echo vm-ssh-ok; nixos-version' 2>/dev/null | grep -q vm-ssh-ok; then
    ok=1
    echo "VM reachable via ssh (attempt $i)"
    break
  fi
  kill -0 "$VM_PID" 2>/dev/null || { echo "VM process died early"; break; }
done
kill "$VM_PID" 2>/dev/null || true
wait "$VM_PID" 2>/dev/null || true
if [ "$ok" = 1 ]; then
  echo "HARNESS OK: minimal test VM booted, ssh reached, KVM working"
  exit 0
else
  echo "HARNESS FAILED: VM did not become ssh-reachable"
  tail -40 "$TMP/vm.log" || true
  exit 1
fi