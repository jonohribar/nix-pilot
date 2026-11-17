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
- "MISSING" rows flag real coverage gaps (Kdenlive, Spectacle, Cursor).

---

## Table

| # | App (user searches) | Proposed match | Tier | Short description | My score | Your verdict |
|---|---|---|---|---|---|---|
| 1 | Firefox | `firefox` | exact | Web browser built from Firefox source tree | GOOD (web browser) |  |
| 2 | Chromium | `chromium` | exact | Open source web browser from Google | GOOD (web browser) |  |
| 3 | Epiphany | `epiphany` | exact | WebKit based web browser for GNOME | GOOD (GNOME web browser) |  |
| 4 | qutebrowser | `qutebrowser` | exact | Keyboard-focused browser with a minimal GUI | GOOD (keyboard-driven browser) |  |
| 5 | Tor Browser | `tor-browser` | exact | Privacy-focused browser routing traffic through the Tor netw | GOOD (privacy browser) |  |
| 6 | VS Code | `vscode` | exact | Code editor developed by Microsoft | GOOD (code editor) |  |
| 7 | VSCodium | `vscodium` | exact | Open source source code editor developed by Microsoft for Wi | GOOD (open-source code editor) |  |
| 8 | gedit | `gedit` | exact | Former GNOME text editor | GOOD (simple text editor) |  |
| 9 | Geany | `geany` | exact | Small and lightweight IDE | GOOD (lightweight IDE) |  |
| 10 | Neovide | `neovide` | exact | Simple, no-nonsense, cross-platform graphical user interface | GOOD (neovim GUI editor) |  |
| 11 | Emacs (GUI) | `emacs` | exact | Extensible, customizable GNU text editor | GOOD (GUI text editor) |  |
| 12 | Mousepad | `mousepad` | exact | Simple text editor for Xfce | GOOD (simple text editor) |  |
| 13 | Sublime Text | `sublime4` | exact | Sophisticated text editor for code, markup and prose | GOOD (text editor) |  |
| 14 | kitty | `kitty` | exact | Fast, feature-rich, GPU based terminal emulator | GOOD (terminal emulator) |  |
| 15 | Alacritty | `alacritty` | exact | Cross-platform, GPU-accelerated terminal emulator | GOOD (GPU terminal emulator) |  |
| 16 | foot | `foot` | exact | Fast, lightweight and minimalistic Wayland terminal emulator | GOOD (Wayland terminal emulator) |  |
| 17 | Tilix | `tilix` | exact | Tiling terminal emulator following the Gnome Human Interface | GOOD (tiling terminal emulator) |  |
| 18 | xterm | `xterm` | exact | Terminal emulator for the X Window System | GOOD (terminal emulator) |  |
| 19 | rxvt-unicode | `rxvt-unicode` | exact | Clone of the well-known terminal emulator rxvt | GOOD (terminal emulator) |  |
| 20 | WezTerm | `wezterm` | exact | GPU-accelerated cross-platform terminal emulator and multipl | GOOD (terminal emulator) |  |
| 21 | st terminal | `st` | exact | Simple Terminal for X from Suckless.org Community | GOOD (suckless terminal emulator) |  |
| 22 | Nautilus | `nautilus` | exact | File manager for GNOME | GOOD (file manager) |  |
| 23 | Nemo | `nemo` | exact | File browser for Cinnamon | GOOD (Cinnamon file browser) |  |
| 24 | Thunar | `thunar` | exact | Xfce file manager | GOOD (Xfce file manager) |  |
| 25 | PCManFM | `pcmanfm` | exact | File manager with GTK interface | GOOD (file manager) |  |
| 26 | Double Commander | `doublecmd` | exact | Two-panel graphical file manager written in Pascal | GOOD (dual-panel file manager) |  |
| 27 | Krusader | `krusader` | exact | Norton/Total Commander clone for KDE | GOOD (KDE file manager) |  |
| 28 | VLC | `vlc` | exact | Cross-platform media player and streaming server | GOOD (media player) |  |
| 29 | mpv | `mpv` | exact | General-purpose media player, fork of MPlayer and mplayer2 | GOOD (media player) |  |
| 30 | Totem | `totem` | exact | Movie player for the GNOME desktop based on GStreamer | GOOD (movie player) |  |
| 31 | Kdenlive | `— no match —` | absent |  | MISSING (real video editor) |  |
| 32 | Audacity | `audacity` | exact | Sound editor with graphical UI | GOOD (audio editor) |  |
| 33 | Blender | `blender` | exact | 3D Creation/Animation/Publishing System | GOOD (3D creation app) |  |
| 34 | OBS Studio | `obs-studio` | exact | Free and open source software for video recording and live s | GOOD (streaming/recording app) |  |
| 35 | GIMP | `gimp` | exact | GNU Image Manipulation Program | GOOD (image editor) |  |
| 36 | Inkscape | `inkscape` | exact | Vector graphics editor | GOOD (vector graphics editor) |  |
| 37 | Krita | `krita` | exact | Free and open source painting application | GOOD (painting app) |  |
| 38 | Darktable | `darktable` | exact | Virtual lighttable and darkroom for photographers | GOOD (photo editor) |  |
| 39 | RawTherapee | `rawtherapee` | exact | RAW converter and digital photo processing software | GOOD (RAW photo editor) |  |
| 40 | Kodi | `kodi` | exact | Media center | GOOD (media center) |  |
| 41 | LibreOffice | `libreoffice` | exact | Comprehensive, professional-quality productivity suite, a va | GOOD (office suite) |  |
| 42 | Calibre | `calibre` | exact | Comprehensive e-book software | GOOD (e-book manager) |  |
| 43 | Evince | `evince` | exact | GNOME's document viewer | GOOD (document viewer) |  |
| 44 | Zathura | `zathura` | exact | Highly customizable and functional PDF viewer | GOOD (PDF viewer) |  |
| 45 | PDF Arranger | `pdfarranger` | exact | Merge or split pdf documents and rotate, crop and rearrange  | GOOD (PDF editor) |  |
| 46 | Thunderbird | `thunderbird` | exact | Full-featured e-mail client | GOOD (mail client) |  |
| 47 | Evolution | `evolution` | exact | Personal information management application that provides in | GOOD (mail/calendar app) |  |
| 48 | eog | `eog` | exact | GNOME image viewer | GOOD (image viewer) |  |
| 49 | feh | `feh` | exact | Light-weight image viewer | UNSURE (command-line image viewer) |  |
| 50 | Shotwell | `shotwell` | exact | Popular photo organizer for the GNOME desktop | GOOD (photo organizer) |  |
| 51 | Flameshot | `flameshot` | exact | Powerful yet simple to use screenshot software | GOOD (screenshot tool) |  |
| 52 | Spectacle | `— no match —` | absent |  | MISSING (KDE screenshot tool) |  |
| 53 | GNOME Font Viewer | `gnome-font-viewer` | exact | Program that can preview fonts and create thumbnails for fon | GOOD (font previewer) |  |
| 54 | Signal | `signal-desktop` | exact | Private, simple, and secure messenger | GOOD (secure messenger) |  |
| 55 | Discord | `discord` | exact | All-in-one cross-platform voice and text chat for gamers | GOOD (chat app) |  |
| 56 | Telegram | `telegram-desktop` | exact | Telegram Desktop messaging app | GOOD (messaging app) |  |
| 57 | Element | `element-desktop` | exact | Matrix client for desktop | GOOD (matrix client) |  |
| 58 | Slack | `slack` | exact | Desktop client for Slack | GOOD (team chat) |  |
| 59 | Rocket.Chat | `rocketchat-desktop` | exact | Official Desktop client for Rocket.Chat | GOOD (chat client) |  |
| 60 | GParted | `gparted` | exact | Graphical disk partitioning tool | GOOD (disk partition tool) |  |
| 61 | GNOME Disk Utility | `gnome-disk-utility` | exact | Udisks graphical front-end | GOOD (disk utility) |  |
| 62 | GNOME System Monitor | `gnome-system-monitor` | exact | System Monitor shows you what programs are running and how m | GOOD (system monitor) |  |
| 63 | htop | `htop` | exact | Interactive process viewer | NOT-APP (terminal process viewer) |  |
| 64 | btop | `btop` | exact | Monitor of resources | NOT-APP (terminal resource monitor) |  |
| 65 | KeePassXC | `keepassxc` | exact | Offline password manager with many features | GOOD (password manager) |  |
| 66 | Remmina | `remmina` | exact | Remote desktop client written in GTK | GOOD (remote desktop client) |  |
| 67 | Pavucontrol | `pavucontrol` | exact | PulseAudio Volume Control | GOOD (audio volume control) |  |
| 68 | Blueman | `blueman` | exact | GTK-based Bluetooth Manager | GOOD (bluetooth manager) |  |
| 69 | gThumb | `gthumb` | exact | Image browser and viewer for GNOME | GOOD (image viewer) |  |
| 70 | GNOME Calculator | `gnome-calculator` | exact | Application that solves mathematical equations and is suitab | GOOD (calculator) |  |
| 71 | Baobab | `baobab` | exact | Graphical application to analyse disk usage in any GNOME env | GOOD (disk usage analyzer) |  |
| 72 | GNOME Chess | `gnome-chess` | exact | Play the classic two-player boardgame of chess | GOOD (chess game) |  |
| 73 | GNOME Mines | `gnome-mines` | exact | Clear hidden mines from a minefield | GOOD (minesweeper game) |  |
| 74 | Foliate | `foliate` | exact | Simple and modern GTK eBook reader | GOOD (ebook reader) |  |
| 75 | Qalculate | `qalculate-gtk` | exact | Ultimate desktop calculator | GOOD (calculator) |  |
| 76 | FreeCAD | `freecad` | exact | General purpose Open Source 3D CAD/MCAD/CAx/CAE/PLM modeler | GOOD (CAD modeler) |  |
| 77 | Godot | `godot` | exact | Free and Open Source 2D and 3D game engine | GOOD (game engine editor) |  |
| 78 | GnuCash | `gnucash` | exact | Free software for double entry accounting | GOOD (accounting app) |  |
| 79 | Spotify | `spotify` | exact | Play music from the Spotify music service | GOOD (music player) |  |
| 80 | Google Chrome | `google-chrome` | exact | Freeware web browser developed by Google | GOOD (web browser) |  |
| 81 | Steam | `steam` | exact | Digital distribution platform | GOOD (game platform) |  |
| 82 | Brave | `brave` | exact | Privacy-oriented browser for Desktop and Laptop computers | GOOD (privacy browser) |  |
| 83 | Adobe Photoshop | `— no match —` | absent |  | CORRECT-ABSENT (no Linux version) |  |
| 84 | Sketch | `— no match —` | absent |  | CORRECT-ABSENT (mac-only app) |  |
| 85 | GarageBand | `— no match —` | absent |  | CORRECT-ABSENT (mac-only app) |  |
| 86 | Microsoft Office | `— no match —` | absent |  | CORRECT-ABSENT (no Linux native app) |  |
| 87 | Adobe Illustrator | `— no match —` | absent |  | CORRECT-ABSENT (no Linux version) |  |
| 88 | Arc browser | `— no match —` | absent |  | CORRECT-ABSENT (no Linux version) |  |
| 89 | Cursor AI editor | `— no match —` | absent |  | MISSING (Cursor exists on Linux) |  |
| 90 | Docker Desktop | `— no match —` | absent |  | MISSING (Docker Desktop exists) |  |
| 91 | Dropbox | `dropbox` | exact | Online stored folders (daemon version) | GOOD (dropbox tray GUI client exists) |  |
| 92 | Figma (community client) | `figma-linux` | exact | Unofficial Electron-based Figma desktop app for Linux | GOOD (community figma-linux client exists) |  |
| 93 | Notion (notes app) | `notion-app` | exact | App to write, plan, collaborate, and get organised | GOOD (notion-app is the notes app) |  |
| 94 | GTK library | `— no match —` | absent |  | CORRECT-ABSENT (library not app) |  |
| 95 | GLib | `glib` | exact | C library of programming buildings blocks | NOT-APP (C library) |  |
| 96 | SQLite | `sqlite` | exact | Self-contained, serverless, zero-configuration, transactiona | NOT-APP (database library) |  |
| 97 | OpenSSL | `openssl` | exact | Cryptographic library that implements the SSL and TLS protoc | NOT-APP (crypto library) |  |
| 98 | curl | `curl` | exact | Command line tool for transferring files with URL syntax | NOT-APP (CLI tool) |  |
| 99 | jq | `jq` | exact | Lightweight and flexible command-line JSON processor | NOT-APP (JSON CLI) |  |
| 100 | ripgrep | `ripgrep` | exact | Utility that combines the usability of The Silver Searcher w | NOT-APP (search CLI) |  |
| 101 | fd | `fd` | exact | Simple, fast and user-friendly alternative to find | NOT-APP (find CLI) |  |
| 102 | bat | `bat` | exact | Cat(1) clone with syntax highlighting and Git integration | NOT-APP (cat CLI) |  |
| 103 | tmux | `tmux` | exact | Terminal multiplexer | NOT-APP (terminal multiplexer) |  |
| 104 | zsh | `zsh` | exact | Z shell | NOT-APP (shell) |  |
| 105 | Git CLI | `git` | exact | Distributed version control system | NOT-APP (version control CLI) |  |
| 106 | FFmpeg | `ffmpeg` | exact | Complete, cross-platform solution to record, convert and str | NOT-APP (media CLI/library) |  |
| 107 | GCC | `gcc` | exact | GNU Compiler Collection, version 15.3.0 (wrapper script) | NOT-APP (compiler CLI) |  |
| 108 | Python runtime | `python3` | exact | High-level dynamically-typed programming language | NOT-APP (programming runtime) |  |
| 109 | Node runtime | `nodejs` | exact | Event-driven I/O framework for the V8 JavaScript engine | NOT-APP (runtime engine) |  |
| 110 | Docker engine | `docker` | exact | Open source project to pack, ship and run any application as | NOT-APP (container daemon) |  |
| 111 | QEMU | `qemu` | exact | Generic and open source machine emulator and virtualizer | NOT-APP (emulator CLI) |  |
| 112 | Mesa graphics | `mesa` | exact | Open source 3D graphics library | NOT-APP (graphics library) |  |
| 113 | Wine | `wine` | exact | Open Source implementation of the Windows API on top of X, O | NOT-APP (compatibility layer) |  |
| 114 | code | `vscode` | exact | Code editor developed by Microsoft | GOOD (maps to VS Code) |  |
| 115 | vim | `vim` | exact | Most popular clone of the VI editor | NOT-APP (terminal editor) |  |
| 116 | ssh | `openssh` | exact | Implementation of the SSH protocol | NOT-APP (ssh CLI tool) |  |
| 117 | notion (ambiguous) | `notion` | exact | Tiling tabbed window manager | WRONG (matches tiling WM, not the notes app) |  |
