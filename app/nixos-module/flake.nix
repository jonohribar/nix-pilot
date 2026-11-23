# nix-pilot NixOS module — flake delivery surface (decision D23).
#
# Use from any NixOS flake:
#   inputs.nix-pilot.url = "github:jonohribar/nix-pilot?dir=app/nixos-module";
#   ... modules = [ nix-pilot.nixosModules.default ];
#
# The module (./nix-pilot.nix) evaluates against the consuming system's own
# nixpkgs via `config`/`lib`/`pkgs`; it adds no inputs of its own beyond the
# helper + polkit action (D21/D23). The flake therefore declares no inputs and
# its `outputs` takes only `{ self }`, so no `follows` is needed and no second
# nixpkgs universe can drift.
{
  outputs =
    { self }: {
      nixosModules.default = import ./nix-pilot.nix;
    };
}
