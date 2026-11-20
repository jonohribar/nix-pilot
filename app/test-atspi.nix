# nix-pilot AT-SPI integration test shell.
# Provides Xvfb, dbus, at-spi2-core, python3 + pygobject3, dogtail for
# driving the GTK4 app via its accessibility tree.
# Usage: nix-shell app/test-atspi.nix
{
  pkgs ? (builtins.getFlake "nixpkgs").legacyPackages.x86_64-linux,
}: with pkgs.lib;
let
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
  name = "nix-pilot-atspi-test";

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
    xvfb-run
    openbox
    blueprint-compiler
    imagemagick
    xorg.xwininfo
    xorg.xwd
    xdotool
    # AT-SPI test dependencies
    python3
    python3Packages.pygobject3
    python3Packages.dogtail
    at-spi2-core
    dbus
    gsettings-desktop-schemas
    gtk3
    adwaita-icon-theme
    hicolor-icon-theme
  ];

  # Ensure GTK4's GSettings (accessibility control) resolves against the
  # real package schema dirs.
  inherit GSETTINGS_SCHEMAS_PATH;

  # at-spi-bus-launcher installs to libexec, not bin, so it is not on PATH
  # by default; without it the a11y registry never starts and the app never
  # registers with AT-SPI. Export the libexec dir.
  shellHook = ''
    export PATH="${pkgs.at-spi2-core}/libexec:$PATH"
  '';

  # gtk4-rs builds need to find the C libraries via pkg-config
  RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
}