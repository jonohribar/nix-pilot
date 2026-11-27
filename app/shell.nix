# nix-pilot desktop client — development shell.
# Provides the Rust toolchain + GTK4 + libadwaita for gtk4-rs/libadwaita-rs.
# Uses the same pinned Nixpkgs snapshot as the rest of the project
# (`flake:nixpkgs` on this machine); a production flake pin comes later.
#
# Usage:  nix-shell app/shell.nix          (from the repo root)
#         then: cd app && cargo build && cargo run
{
  pkgs ? (builtins.getFlake "nixpkgs").legacyPackages.x86_64-linux,
}: with pkgs.lib;
let
  # GSettings schema dirs for every GUI package in this shell. Mirrors
  # test-atspi.nix; keep the two in sync. Without libadwaita's own schema
  # (org.gnome.Adw), adw_init fails and the app exits before its window,
  # which the smoke test only catches on machines without a desktop
  # libadwaita hiding the gap.
  schemas-dir = p:
    let dir = "${p}/share/gsettings-schemas";
    in if builtins.pathExists dir then
      let dirs = builtins.attrNames (builtins.readDir dir);
      in "${dir}/${builtins.head dirs}"
    else null;
  GSETTINGS_SCHEMAS_PATH = builtins.concatStringsSep ":"
    (builtins.filter (x: x != null)
      (builtins.map schemas-dir
        [ pkgs.gsettings-desktop-schemas pkgs.gtk4 pkgs.libadwaita pkgs.gtk3 ]));
in
pkgs.mkShell {
  name = "nix-pilot-app-dev";

  packages = with pkgs; [
    cargo
    rustc
    rustfmt
    clippy
    rust-analyzer
    pkg-config
    gtk4
    libadwaita
    glib
    gdk-pixbuf
    xvfb-run # headless smoke tests (no display needed)
    openbox # minimal WM so GTK4 frame clock ticks under Xvfb (wireframe captures)
    blueprint-compiler # wireframe source (.blp) -> GTK .ui
    imagemagick # capture Xvfb screenshots of wireframes
    xwininfo # diagnose which X windows are mapped
    xwd # raster dump of an X window
  ];

  # gtk4-rs builds need to find the C libraries via pkg-config; nixpkgs'
  # dev outputs put the .pc files on PKG_CONFIG_PATH through propagation.
  RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
}
