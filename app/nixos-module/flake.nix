# nix-pilot NixOS module — flake delivery surface (decision D23).
#
# Use from any NixOS flake:
#   inputs.nix-pilot.url = "github:<owner>/nix-pilot";
#   inputs.nix-pilot.nixpkgs.follows = "nixpkgs";   # the app never imposes a pin
#   ... modules = [ nix-pilot.nixosModules.default ];
#
# The module (./nix-pilot.nix) evaluates against the consuming system's own
# nixpkgs via `config`/`lib`/`pkgs`; it adds no inputs of its own beyond the
# helper + polkit action. `nix-pilot.nixpkgs.follows` keeps it on the user's
# nixpkgs so there is no second universe to drift (D21/D23).
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
  };
  outputs =
    { self }: {
      nixosModules.default = import ./nix-pilot.nix;
    };
}
