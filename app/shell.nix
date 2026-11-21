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

  # GTK4's gsettings schemas: without a resolvable schema dir, GTK's settings
  # backend fails to look up keys and floods stderr with a GLib-GIO-CRITICAL
  # on every launch. Point glib at the gtk4 + desktop-schemas schema dirs.
  GSETTINGS_SCHEMA_DIR = pkgs.lib.concatStringsSep ":" [
    "${pkgs.gtk4}/share/gsettings-schemas/${pkgs.gtk4.name}/glib-2.0/schemas"
    "${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}/glib-2.0/schemas"
  ];
}
