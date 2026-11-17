# QA Sample v1 — app-to-package join review

This sample checks whether the catalogue's automatic join between a consumer-facing
app name and a Nixpkgs package is correct and appropriate for a normal user. It was
generated from the Phase 0 export (see [../README.md](../README.md)); 116 entries.

## What I need from you (about 20–30 minutes, skimmable)

- Read the table below — each row is one app name a user might search for.
- Column "My score" is an independent labeller's verdict (a subagent role-playing a
  non-technical consumer).
- **If you agree with "My score", leave the row untouched (skip = agree).** If you
  disagree, overwrite the **"Your verdict"** cell with one of the tokens below.
- Verdict tokens:
  - `GOOD` — right app, belongs in a consumer store.
  - `WRONG` — same name, wrong package.
  - `NOT-APP` — it's a library/CLI/runtime/daemon; must not be shown as an app.
  - `UNSURE` — can't tell.
  - `CORRECT-ABSENT` — no match found and that's fine (made-up, wrong-platform).
  - `MISSING` — a real consumer app a normal user would expect; its absence is a
    coverage gap.
- When done, commit the edited file or reply with just the ids you disagree with.
  I compute the precision/recall numbers from your version.

## What this scores

- "My score = GOOD" rows measure recall (are real apps found?).
- "My score = NOT-APP" rows measure precision (do we wrongly present libs/CLIs?).
- "MISSING" rows flag real coverage gaps (Kdenlive, Spectacle, Cursor, Dropbox).

---

## Table

| # | App (user searches) | Proposed match | Tier | Short description | My score | Your verdict |
|---|---|---|---|---|---|---|
| 1 | Firefox | `firefox` | exact | Web browser built from Firefox source tree | GOOD (Firefox browser correct) |  |
| 2 | Chromium | `chromium` | exact | Open source web browser from Google | GOOD (Chromium browser correct) |  |
| 3 | Epiphany | `epiphany` | exact | WebKit based web browser for GNOME | GOOD (Epiphany GNOME browser correct) |  |
| 4 | qutebrowser | `qutebrowser` | exact | Keyboard-focused browser with a minimal GUI | GOOD (qutebrowser GUI browser correct) |  |
| 5 | Tor Browser | `tor-browser` | exact | Privacy-focused browser routing traffic through the Tor netw | GOOD (Tor Browser privacy browser correct) |  |
| 6 | VS Code | `vscode` | exact | Code editor developed by Microsoft | GOOD (VS Code editor correct) |  |
| 7 | VSCodium | `vscodium` | exact | Open source source code editor developed by Microsoft for Wi | GOOD (VSCodium editor correct) |  |
| 8 | gedit | `gedit` | exact | Former GNOME text editor | GOOD (gedit text editor correct) |  |
| 9 | Geany | `geany` | exact | Small and lightweight IDE | GOOD (Geany lightweight IDE correct) |  |
| 10 | Neovide | `neovide` | exact | Simple, no-nonsense, cross-platform graphical user interface | GOOD (Neovide GUI Neovim frontend) |  |
| 11 | Emacs (GUI) | `emacs` | exact | Extensible, customizable GNU text editor | GOOD (Emacs GUI editor correct) |  |
| 12 | Mousepad | `mousepad` | exact | Simple text editor for Xfce | GOOD (Mousepad text editor correct) |  |
| 13 | Sublime Text | `sublime4` | exact | Sophisticated text editor for code, markup and prose | GOOD (Sublime Text editor correct) |  |
| 14 | kitty | `kitty` | exact | Fast, feature-rich, GPU based terminal emulator | GOOD (kitty terminal emulator correct) |  |
| 15 | Alacritty | `alacritty` | exact | Cross-platform, GPU-accelerated terminal emulator | GOOD (Alacritty terminal emulator correct) |  |
| 16 | foot | `foot` | exact | Fast, lightweight and minimalistic Wayland terminal emulator | GOOD (foot Wayland terminal correct) |  |
| 17 | Tilix | `tilix` | exact | Tiling terminal emulator following the Gnome Human Interface | GOOD (Tilix terminal emulator correct) |  |
| 18 | xterm | `xterm` | exact | Terminal emulator for the X Window System | GOOD (xterm terminal emulator correct) |  |
| 19 | rxvt-unicode | `rxvt-unicode` | exact | Clone of the well-known terminal emulator rxvt | GOOD (rxvt-unicode terminal emulator correct) |  |
| 20 | WezTerm | `wezterm` | exact | GPU-accelerated cross-platform terminal emulator and multipl | GOOD (WezTerm terminal emulator correct) |  |
| 21 | st terminal | `st` | exact | Simple Terminal for X from Suckless.org Community | GOOD (st terminal emulator correct) |  |
| 22 | Nautilus | `nautilus` | exact | File manager for GNOME | GOOD (Nautilus file manager correct) |  |
| 23 | Nemo | `nemo` | exact | File browser for Cinnamon | GOOD (Nemo file manager correct) |  |
| 24 | Thunar | `thunar` | exact | Xfce file manager | GOOD (Thunar file manager correct) |  |
| 25 | PCManFM | `pcmanfm` | exact | File manager with GTK interface | GOOD (PCManFM file manager correct) |  |
| 26 | Double Commander | `doublecmd` | exact | Two-panel graphical file manager written in Pascal | GOOD (Double Commander file manager correct) |  |
| 27 | Krusader | `krusader` | exact | Norton/Total Commander clone for KDE | GOOD (Krusader file manager correct) |  |
| 28 | VLC | `vlc` | exact | Cross-platform media player and streaming server | GOOD (VLC media player correct) |  |
| 29 | mpv | `mpv` | exact | General-purpose media player, fork of MPlayer and mplayer2 | GOOD (mpv media player correct) |  |
| 30 | Totem | `totem` | exact | Movie player for the GNOME desktop based on GStreamer | GOOD (Totem video player correct) |  |
| 31 | Kdenlive | `— no match —` | absent |  | MISSING (Kdenlive video editor in catalogue) |  |
| 32 | Audacity | `audacity` | exact | Sound editor with graphical UI | GOOD (Audacity audio editor correct) |  |
| 33 | Blender | `blender` | exact | 3D Creation/Animation/Publishing System | GOOD (Blender 3D suite correct) |  |
| 34 | OBS Studio | `obs-studio` | exact | Free and open source software for video recording and live s | GOOD (OBS Studio recorder correct) |  |
| 35 | GIMP | `gimp` | exact | GNU Image Manipulation Program | GOOD (GIMP image editor correct) |  |
| 36 | Inkscape | `inkscape` | exact | Vector graphics editor | GOOD (Inkscape vector editor correct) |  |
| 37 | Krita | `krita` | exact | Free and open source painting application | GOOD (Krita painting app correct) |  |
| 38 | Darktable | `darktable` | exact | Virtual lighttable and darkroom for photographers | GOOD (Darktable photo editor correct) |  |
| 39 | RawTherapee | `rawtherapee` | exact | RAW converter and digital photo processing software | GOOD (RawTherapee photo processor correct) |  |
| 40 | Kodi | `kodi` | exact | Media center | GOOD (Kodi media center correct) |  |
| 41 | LibreOffice | `libreoffice` | exact | Comprehensive, professional-quality productivity suite, a va | GOOD (LibreOffice office suite correct) |  |
| 42 | Calibre | `calibre` | exact | Comprehensive e-book software | GOOD (Calibre ebook manager correct) |  |
| 43 | Evince | `evince` | exact | GNOME's document viewer | GOOD (Evince document viewer correct) |  |
| 44 | Zathura | `zathura` | exact | Highly customizable and functional PDF viewer | GOOD (Zathura PDF viewer correct) |  |
| 45 | PDF Arranger | `pdfarranger` | exact | Merge or split pdf documents and rotate, crop and rearrange  | GOOD (PDF Arranger utility correct) |  |
| 46 | Thunderbird | `thunderbird` | exact | Full-featured e-mail client | GOOD (Thunderbird email client correct) |  |
| 47 | Evolution | `evolution` | exact | Personal information management application that provides in | GOOD (Evolution mail client correct) |  |
| 48 | eog | `eog` | exact | GNOME image viewer | GOOD (eog image viewer correct) |  |
| 49 | feh | `feh` | exact | Light-weight image viewer | GOOD (feh image viewer correct) |  |
| 50 | Shotwell | `shotwell` | exact | Popular photo organizer for the GNOME desktop | GOOD (Shotwell photo manager correct) |  |
| 51 | Flameshot | `flameshot` | exact | Powerful yet simple to use screenshot software | GOOD (Flameshot screenshot tool correct) |  |
| 52 | Spectacle | `— no match —` | absent |  | MISSING (Spectacle KDE screenshot tool) |  |
| 53 | GNOME Font Viewer | `gnome-font-viewer` | exact | Program that can preview fonts and create thumbnails for fon | GOOD (GNOME Font Viewer correct) |  |
| 54 | Signal | `signal-desktop` | exact | Private, simple, and secure messenger | GOOD (Signal messenger correct) |  |
| 55 | Discord | `discord` | exact | All-in-one cross-platform voice and text chat for gamers | GOOD (Discord chat correct) |  |
| 56 | Telegram | `telegram-desktop` | exact | Telegram Desktop messaging app | GOOD (Telegram messenger correct) |  |
| 57 | Element | `element-desktop` | exact | Matrix client for desktop | GOOD (Element Matrix client correct) |  |
| 58 | Slack | `slack` | exact | Desktop client for Slack | GOOD (Slack chat client correct) |  |
| 59 | Rocket.Chat | `rocketchat-desktop` | exact | Official Desktop client for Rocket.Chat | GOOD (Rocket.Chat desktop client correct) |  |
| 60 | GParted | `gparted` | exact | Graphical disk partitioning tool | GOOD (GParted disk tool correct) |  |
| 61 | GNOME Disk Utility | `gnome-disk-utility` | exact | Udisks graphical front-end | GOOD (GNOME Disk Utility correct) |  |
| 62 | GNOME System Monitor | `gnome-system-monitor` | exact | System Monitor shows you what programs are running and how m | GOOD (GNOME System Monitor correct) |  |
| 63 | htop | `htop` | exact | Interactive process viewer | NOT-APP (htop CLI process viewer) |  |
| 64 | btop | `btop` | exact | Monitor of resources | NOT-APP (btop CLI monitor) |  |
| 65 | KeePassXC | `keepassxc` | exact | Offline password manager with many features | GOOD (KeePassXC password manager correct) |  |
| 66 | Remmina | `remmina` | exact | Remote desktop client written in GTK | GOOD (Remmina remote desktop correct) |  |
| 67 | Pavucontrol | `pavucontrol` | exact | PulseAudio Volume Control | GOOD (Pavucontrol volume GUI correct) |  |
| 68 | Blueman | `blueman` | exact | GTK-based Bluetooth Manager | GOOD (Blueman bluetooth manager correct) |  |
| 69 | gThumb | `gthumb` | exact | Image browser and viewer for GNOME | GOOD (gThumb image viewer correct) |  |
| 70 | GNOME Calculator | `gnome-calculator` | exact | Application that solves mathematical equations and is suitab | GOOD (GNOME Calculator correct) |  |
| 71 | Baobab | `baobab` | exact | Graphical application to analyse disk usage in any GNOME env | GOOD (Baobab disk usage GUI) |  |
| 72 | GNOME Chess | `gnome-chess` | exact | Play the classic two-player boardgame of chess | GOOD (GNOME Chess game correct) |  |
| 73 | GNOME Mines | `gnome-mines` | exact | Clear hidden mines from a minefield | GOOD (GNOME Mines game correct) |  |
| 74 | Foliate | `foliate` | exact | Simple and modern GTK eBook reader | GOOD (Foliate ebook reader correct) |  |
| 75 | Qalculate | `qalculate-gtk` | exact | Ultimate desktop calculator | GOOD (Qalculate calculator correct) |  |
| 76 | FreeCAD | `freecad` | exact | General purpose Open Source 3D CAD/MCAD/CAx/CAE/PLM modeler | GOOD (FreeCAD CAD modeler correct) |  |
| 77 | Godot | `godot` | exact | Free and Open Source 2D and 3D game engine | GOOD (Godot engine GUI editor) |  |
| 78 | GnuCash | `gnucash` | exact | Free software for double entry accounting | GOOD (GnuCash accounting app correct) |  |
| 79 | Spotify | `spotify` | exact | Play music from the Spotify music service | GOOD (Spotify player correct) |  |
| 80 | Google Chrome | `google-chrome` | exact | Freeware web browser developed by Google | GOOD (Google Chrome browser correct) |  |
| 81 | Steam | `steam` | exact | Digital distribution platform | GOOD (Steam platform correct) |  |
| 82 | Brave | `brave` | exact | Privacy-oriented browser for Desktop and Laptop computers | GOOD (Brave browser correct) |  |
| 83 | Adobe Photoshop | `— no match —` | absent |  | CORRECT-ABSENT (Photoshop proprietary no Linux) |  |
| 84 | Figma | `— no match —` | absent |  | CORRECT-ABSENT (Figma web-only no package) |  |
| 85 | Sketch | `— no match —` | absent |  | CORRECT-ABSENT (Sketch macOS-only) |  |
| 86 | GarageBand | `— no match —` | absent |  | CORRECT-ABSENT (GarageBand macOS-only) |  |
| 87 | Microsoft Office | `— no match —` | absent |  | CORRECT-ABSENT (Office proprietary no Linux) |  |
| 88 | Adobe Illustrator | `— no match —` | absent |  | CORRECT-ABSENT (Illustrator no Linux build) |  |
| 89 | Arc browser | `— no match —` | absent |  | CORRECT-ABSENT (Arc no Linux support) |  |
| 90 | Notion | `— no match —` | absent |  | CORRECT-ABSENT (Notion no native package) |  |
| 91 | Cursor AI editor | `— no match —` | absent |  | MISSING (Cursor popular native editor absent) |  |
| 92 | Docker Desktop | `— no match —` | absent |  | CORRECT-ABSENT (Docker Desktop dev tool absent) |  |
| 93 | Dropbox | `— no match —` | absent |  | MISSING (Dropbox popular consumer app absent) |  |
| 94 | GTK library | `— no match —` | absent |  | CORRECT-ABSENT (GTK library not store app) |  |
| 95 | GLib | `glib` | exact | C library of programming buildings blocks | NOT-APP (GLib library only) |  |
| 96 | SQLite | `sqlite` | exact | Self-contained, serverless, zero-configuration, transactiona | NOT-APP (SQLite embedded library) |  |
| 97 | OpenSSL | `openssl` | exact | Cryptographic library that implements the SSL and TLS protoc | NOT-APP (OpenSSL crypto library) |  |
| 98 | curl | `curl` | exact | Command line tool for transferring files with URL syntax | NOT-APP (curl CLI transfer tool) |  |
| 99 | jq | `jq` | exact | Lightweight and flexible command-line JSON processor | NOT-APP (jq CLI JSON tool) |  |
| 100 | ripgrep | `ripgrep` | exact | Utility that combines the usability of The Silver Searcher w | NOT-APP (ripgrep CLI search tool) |  |
| 101 | fd | `fd` | exact | Simple, fast and user-friendly alternative to find | NOT-APP (fd CLI find tool) |  |
| 102 | bat | `bat` | exact | Cat(1) clone with syntax highlighting and Git integration | NOT-APP (bat CLI viewer) |  |
| 103 | tmux | `tmux` | exact | Terminal multiplexer | NOT-APP (tmux terminal multiplexer CLI) |  |
| 104 | zsh | `zsh` | exact | Z shell | NOT-APP (zsh shell runtime) |  |
| 105 | Git CLI | `git` | exact | Distributed version control system | NOT-APP (Git CLI not GUI app) |  |
| 106 | FFmpeg | `ffmpeg` | exact | Complete, cross-platform solution to record, convert and str | NOT-APP (FFmpeg CLI media tool) |  |
| 107 | GCC | `gcc` | exact | GNU Compiler Collection, version 15.3.0 (wrapper script) | NOT-APP (GCC compiler toolchain) |  |
| 108 | Python runtime | `python3` | exact | High-level dynamically-typed programming language | NOT-APP (Python runtime not app) |  |
| 109 | Node runtime | `nodejs` | exact | Event-driven I/O framework for the V8 JavaScript engine | NOT-APP (Node runtime not app) |  |
| 110 | Docker engine | `docker` | exact | Open source project to pack, ship and run any application as | NOT-APP (Docker engine daemon not app) |  |
| 111 | QEMU | `qemu` | exact | Generic and open source machine emulator and virtualizer | NOT-APP (QEMU emulator CLI) |  |
| 112 | Mesa graphics | `mesa` | exact | Open source 3D graphics library | NOT-APP (Mesa graphics library) |  |
| 113 | Wine | `wine` | exact | Open Source implementation of the Windows API on top of X, O | NOT-APP (Wine compatibility layer) |  |
| 114 | code | `vscode` | exact | Code editor developed by Microsoft | GOOD (code resolves to VS Code) |  |
| 115 | vim | `vim` | exact | Most popular clone of the VI editor | NOT-APP (vim CLI editor) |  |
| 116 | ssh | `openssh` | exact | Implementation of the SSH protocol | NOT-APP (ssh CLI client) |  |
