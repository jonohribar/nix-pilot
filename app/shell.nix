# nix-pilot desktop client — development shell.
# Provides the Rust toolchain + GTK4 + libadwaita for gtk4-rs/libadwaita-rs.
# Uses the same pinned Nixpkgs snapshot as the rest of the project
# (`flake:nixpkgs` on this machine); a production flake pin comes later.
#
# Usage:  nix-shell app/shell.nix          (from the repo root)
#         then: cd app && cargo build && cargo run
{
  pkgs ? (builtins.getFlake "nixpkgs").legacyPackages.x86_64-linux,
}:
pkgs.mkShell {
  name = "nix-pilot-app-dev";

  packages = with pkgs; [
    cargo
    rustc
    rust-analyzer
    pkg-config
    gtk4
    libadwaita
    glib
    gdk-pixbuf
    xvfb-run # headless smoke tests (no display needed)
  ];

  # gtk4-rs builds need to find the C libraries via pkg-config; nixpkgs'
  # dev outputs put the .pc files on PKG_CONFIG_PATH through propagation.
  RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
}
