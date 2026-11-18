#!/usr/bin/env node
// Join prototype (decision D13): how well does a desktop entry's Exec
// basename match a Nixpkgs attribute path or pname? Resolves the exact output
// store paths of the sample packages (as discover.sh does) and scores the
// match of each .desktop file found in those outputs.
//
// Usage: join.js   (requires results/records.json from measure.sh)
const fs = require('fs');
const cp = require('child_process');
const path = require('path');

const root = path.join(__dirname);
const recs = JSON.parse(fs.readFileSync(path.join(root, 'results/records.json'), 'utf8'));
const drv = recs.filter((r) => r.isDerivation);
const attrs = new Set(drv.map((r) => r.name.toLowerCase()));
const pnames = new Set(drv.filter((r) => r.pname).map((r) => r.pname.toLowerCase()));

const pkgs = [
  'feh', 'mpv', 'kitty', 'rofi', 'xterm', 'meld', 'pcmanfm', 'htop',
  'gedit', 'evince', 'gnome-calculator', 'baobab', 'eog',
];

const outs = new Set();
for (const p of pkgs) {
  try {
    const out = cp.execSync(`nix build --no-link --print-out-paths nixpkgs#${p} 2>/dev/null`, { encoding: 'utf8' });
    out.split('\n').filter(Boolean).forEach((l) => outs.add(l.trim()));
  } catch (e) { /* skip pkg */ }
}

const deskFiles = [];
for (const o of outs) {
  const found = cp.execSync(`find ${o} -name '*.desktop' 2>/dev/null`, { encoding: 'utf8' });
  found.split('\n').filter(Boolean).forEach((f) => deskFiles.push(f));
}

function execName(file) {
  const txt = fs.readFileSync(file, 'utf8');
  const m = txt.match(/^Exec\s*=\s*([^\s]+)/m);
  if (!m) return null;
  return path.basename(m[1]).toLowerCase().replace(/\.(sh|bin)$/, '');
}

const exact = [];
const pnameMatch = [];
const none = [];
for (const f of deskFiles) {
  const e = execName(f);
  if (!e) continue;
  if (attrs.has(e)) exact.push([e, path.basename(path.dirname(path.dirname(f))) + '/' + path.basename(f)]);
  else if (pnames.has(e)) pnameMatch.push([e, path.basename(f)]);
  else none.push([e, path.basename(f)]);
}

console.log(JSON.stringify(
  {
    outputDirsResolved: outs.size,
    desktopFilesFound: deskFiles.length,
    execExtracted: exact.length + pnameMatch.length + none.length,
    matchedExactAttr: exact.length,
    matchedPname: pnameMatch.length,
    unmatched: none.length,
    matchedExamples: exact.concat(pnameMatch).slice(0, 12).map((x) => x[0]),
    unmatchedExamples: none.slice(0, 8).map((x) => x[0]),
  },
  null, 2,
));