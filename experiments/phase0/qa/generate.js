#!/usr/bin/env node
// QA sample generator. Resolves a seed list of consumer-facing app names
// against the exported metadata and emits qa/entries.json (name, proposed
// match, join tier, description hint). The human (or a labeller subagent)
// scores each entry; skip means "agree with the machine's self-score".
//
// Usage: node qa/generate.js  ->  writes qa/entries.json
const fs = require('fs');
const path = require('path');

const root = path.join(__dirname, '..');
const recs = JSON.parse(fs.readFileSync(path.join(root, 'results/records.json'), 'utf8'));
const byAttr = new Map(recs.filter((r) => r.isDerivation && !r.evalError).map((r) => [r.name, r]));

// [consumer-facing name, expected top-level attribute path ('' = expect none)]
const SEED = [
  // --- known GUI apps -------------------------------------------------
  ['Firefox', 'firefox'], ['Chromium', 'chromium'], ['Epiphany', 'epiphany'], ['qutebrowser', 'qutebrowser'],
  ['Tor Browser', 'tor-browser'], ['VS Code', 'vscode'], ['VSCodium', 'vscodium'], ['gedit', 'gedit'],
  ['Geany', 'geany'], ['Neovide', 'neovide'], ['Emacs (GUI)', 'emacs'], ['Mousepad', 'mousepad'],
  ['Sublime Text', 'sublime4'], ['kitty', 'kitty'], ['Alacritty', 'alacritty'], ['foot', 'foot'],
  ['Tilix', 'tilix'], ['xterm', 'xterm'], ['rxvt-unicode', 'rxvt-unicode'], ['WezTerm', 'wezterm'],
  ['st terminal', 'st'], ['Nautilus', 'nautilus'], ['Nemo', 'nemo'], ['Thunar', 'thunar'],
  ['PCManFM', 'pcmanfm'], ['Double Commander', 'doublecmd'], ['Krusader', 'krusader'],
  ['VLC', 'vlc'], ['mpv', 'mpv'], ['Totem', 'totem'], ['Kdenlive', 'kdenlive'], ['Audacity', 'audacity'],
  ['Blender', 'blender'], ['OBS Studio', 'obs-studio'], ['GIMP', 'gimp'], ['Inkscape', 'inkscape'],
  ['Krita', 'krita'], ['Darktable', 'darktable'], ['RawTherapee', 'rawtherapee'], ['Kodi', 'kodi'],
  ['LibreOffice', 'libreoffice'], ['Calibre', 'calibre'], ['Evince', 'evince'], ['Zathura', 'zathura'],
  ['PDF Arranger', 'pdfarranger'], ['Thunderbird', 'thunderbird'], ['Evolution', 'evolution'],
  ['eog', 'eog'], ['feh', 'feh'], ['Shotwell', 'shotwell'], ['Flameshot', 'flameshot'],
  ['Spectacle', 'spectacle'], ['GNOME Font Viewer', 'gnome-font-viewer'], ['Signal', 'signal-desktop'],
  ['Discord', 'discord'], ['Telegram', 'telegram-desktop'], ['Element', 'element-desktop'],
  ['Slack', 'slack'], ['Rocket.Chat', 'rocketchat-desktop'], ['GParted', 'gparted'],
  ['GNOME Disk Utility', 'gnome-disk-utility'], ['GNOME System Monitor', 'gnome-system-monitor'],
  ['htop', 'htop'], ['btop', 'btop'], ['KeePassXC', 'keepassxc'], ['Remmina', 'remmina'],
  ['Pavucontrol', 'pavucontrol'], ['Blueman', 'blueman'], ['gThumb', 'gthumb'],
  ['GNOME Calculator', 'gnome-calculator'], ['Baobab', 'baobab'], ['GNOME Chess', 'gnome-chess'],
  ['GNOME Mines', 'gnome-mines'], ['Foliate', 'foliate'], ['Qalculate', 'qalculate-gtk'],
  ['FreeCAD', 'freecad'], ['Godot', 'godot'], ['GnuCash', 'gnucash'],
  // --- unfree (must be listable per D1) ------------------------------
  ['Spotify', 'spotify'], ['Google Chrome', 'google-chrome'], ['Steam', 'steam'], ['Brave', 'brave'],
  // --- absent apps (expect NO match) ----------------------------------
  ['Adobe Photoshop', ''], ['Figma', ''], ['Sketch', ''], ['GarageBand', ''],
  ['Microsoft Office', ''], ['Adobe Illustrator', ''], ['Arc browser', ''], ['Notion', ''],
  ['Cursor AI editor', ''], ['Docker Desktop', ''], ['Dropbox', ''],
  // --- libs / CLIs / runtimes with app-adjacent names (must NOT be sold as apps)
  ['GTK library', 'gtk'], ['GLib', 'glib'], ['SQLite', 'sqlite'], ['OpenSSL', 'openssl'],
  ['curl', 'curl'], ['jq', 'jq'], ['ripgrep', 'ripgrep'], ['fd', 'fd'], ['bat', 'bat'],
  ['tmux', 'tmux'], ['zsh', 'zsh'], ['Git CLI', 'git'], ['FFmpeg', 'ffmpeg'], ['GCC', 'gcc'],
  ['Python runtime', 'python3'], ['Node runtime', 'nodejs'], ['Docker engine', 'docker'], ['QEMU', 'qemu'],
  ['Mesa graphics', 'mesa'], ['Wine', 'wine'],
  // --- ambiguous names (machine should flag uncertainty) --------------
  ['code', 'vscode'], ['vim', 'vim'], ['ssh', 'openssh'],
];

const entries = SEED.map(([name, hint], i) => {
  const rec = hint ? byAttr.get(hint) : undefined;
  const found = !!rec;
  return {
    id: i + 1,
    name,
    hint: hint || null,
    found,
    attr: found ? hint : null,
    tier: found ? (hint === rec.name ? 'exact' : 'differs') : 'no-match',
    desc: found ? (rec.description || '').slice(0, 70) : null,
  };
});

fs.writeFileSync(path.join(__dirname, 'entries.json'), JSON.stringify(entries, null, 2));
console.log(`wrote qa/entries.json: ${entries.length} entries; found=${entries.filter((e) => e.found).length}, no-match=${entries.filter((e) => !e.found).length}`);
const exact = entries.filter((e) => e.found && e.tier === 'exact').length;
console.log(`exact joins=${exact}, no-match=${entries.filter((e) => !e.found).length}`);