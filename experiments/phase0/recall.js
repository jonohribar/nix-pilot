#!/usr/bin/env node
// Known-apps recall sample: how many well-known GUI applications resolve to a
// top-level Nixpkgs attribute in the pinned set, and how complete is their
// metadata. Automated proxy for the hand-labelled QA sample: this measures
// join recall for the common-name -> attribute-path direction.
//
// Usage: node recall.js
const fs = require('fs');
const path = require('path');

const recs = JSON.parse(fs.readFileSync(path.join(__dirname, 'results/records.json'), 'utf8'));
const drv = recs.filter((r) => r.isDerivation && !r.evalError);
const byAttr = new Map(drv.map((r) => [r.name, r]));

// <app name>, <expected top-level attribute path>
const KNOWN = [
  // browsers
  ['Firefox', 'firefox'], ['Chromium', 'chromium'], ['Epiphany', 'epiphany'],
  ['qutebrowser', 'qutebrowser'], ['Tor Browser', 'tor-browser-bundle-bin'],
  // editors / IDEs
  ['VS Code', 'vscode'], ['VSCodium', 'vscodium'], ['gedit', 'gedit'], ['Geany', 'geany'],
  ['Neovide', 'neovide'], ['Emacs (GUI)', 'emacs'], ['Mousepad', 'mousepad'], ['Sublime Text', 'sublime4'],
  // terminals
  ['kitty', 'kitty'], ['Alacritty', 'alacritty'], ['foot', 'foot'], ['Tilix', 'tilix'],
  ['xterm', 'xterm'], ['rxvt-unicode', 'rxvt-unicode'], ['WezTerm', 'wezterm'], ['st', 'st'],
  // file managers
  ['Nautilus', 'nautilus'], ['Nemo', 'nemo'], ['Thunar', 'thunar'], ['PCManFM', 'pcmanfm'],
  ['Double Commander', 'doublecmd'], ['Krusader', 'krusader'],
  // media
  ['VLC', 'vlc'], ['mpv', 'mpv'], ['Totem', 'totem'], ['Kdenlive', 'kdenlive'], ['Audacity', 'audacity'],
  ['Blender', 'blender'], ['OBS Studio', 'obs-studio'], ['GIMP', 'gimp'], ['Inkscape', 'inkscape'],
  ['Krita', 'krita'], ['Darktable', 'darktable'], ['RawTherapee', 'rawtherapee'], ['Kodi', 'kodi'],
  // office / documents / email
  ['LibreOffice', 'libreoffice'], ['Calibre', 'calibre'], ['Evince', 'evince'], ['Zathura', 'zathura'],
  ['PDF Arranger', 'pdfarranger'], ['Thunderbird', 'thunderbird'], ['Evolution', 'evolution'],
  // graphics / viewers
  ['eog', 'eog'], ['feh', 'feh'], ['Shotwell', 'shotwell'], ['Flameshot', 'flameshot'],
  ['Spectacle', 'spectacle'], ['GNOME Font Viewer', 'gnome-font-viewer'],
  // messaging
  ['Signal', 'signal-desktop'], ['Discord', 'discord'], ['Telegram', 'telegram-desktop'],
  ['Element', 'element-desktop'], ['Slack', 'slack'], ['Rocket.Chat', 'rocketchat-desktop'],
  // utilities / system
  ['GParted', 'gparted'], ['GNOME Disk Utility', 'gnome-disk-utility'],
  ['GNOME System Monitor', 'gnome-system-monitor'], ['htop', 'htop'], ['btop', 'btop'],
  ['KeePassXC', 'keepassxc'], ['Remmina', 'remmina'], ['Pavucontrol', 'pavucontrol'],
  ['Blueman', 'blueman'], ['gThumb', 'gthumb'], ['GNOME Calculator', 'gnome-calculator'],
  ['Baobab', 'baobab'], ['GNOME Chess', 'gnome-chess'], ['GNOME Mines', 'gnome-mines'],
  ['Foliate', 'foliate'], ['Qalculate', 'qalculate-gtk'], ['FreeCAD', 'freecad'],
  ['Godot', 'godot'], ['GnuCash', 'gnucash'], ['Krita (dup-guard)', 'krita'],
];

const results = KNOWN.map(([name, attr]) => {
  const rec = byAttr.get(attr);
  return {
    name, attr,
    found: !!rec,
    descriptionPresent: !!rec?.description,
    linuxOK: !!(rec && (rec.platforms || []).some((p) => /(x86_64|aarch64|i686)-linux/.test(p))),
  };
});

const found = results.filter((r) => r.found);
const missing = results.filter((r) => !r.found);
console.log(JSON.stringify(
  {
    knownApps: results.length,
    resolvableToAttr: found.length,
    recall: +(found.length / results.length).toFixed(2),
    unresolvable: missing.map((m) => m.name + ' (' + m.attr + ')'),
    ofResolvable: {
      descriptionPresent: found.filter((r) => r.descriptionPresent).length,
      installableOnLinux: found.filter((r) => r.linuxOK).length,
    },
  },
  null, 2,
));