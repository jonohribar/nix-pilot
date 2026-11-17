#!/usr/bin/env node
// Generator v1: builds the classified, versioned catalogue index.
//
// Reads the full-set export (results/records.json) and produces a versioned
// index of GUI-app candidates + command-line tools (decision D16), excluding
// libraries/system components, each with a heuristic confidence tier.
// Desktop-evidence tiering (rich/basic from .desktop/metainfo) is the upgrade
// path once the coverage pipeline runs at scale; until then every entry is
// tier 'heuristic' with a confidence score, per the classification table.
//
// Usage: node generator.js   -> writes results/catalogue-index.json
const fs = require('fs');
const path = require('path');

const root = path.join(__dirname);
const recs = JSON.parse(fs.readFileSync(path.join(root, 'results/records.json'), 'utf8'));
const drv = recs.filter((r) => r.isDerivation && !r.evalError);

const APP_HINTS = [
  'browser', 'editor', 'viewer', 'player', 'manager', 'client', 'suite', 'desktop',
  'graphical', 'graphics', 'gui', 'environment', 'shell', 'game', 'studio',
  'terminal emulator', 'image viewer', 'media', 'office', 'chat', 'messenger',
  'note', 'reader', 'image editor', 'video', 'icon', 'wallpaper', 'font',
];
const CLI_HINTS = [
  'library', 'development', 'dev tool', 'command-line', 'command line', 'cli ',
  'header', 'runtime', 'daemon', 'server', 'sdk', 'binding api', 'api', 'module',
  'tool for developers', 'compiler', 'kernel', 'framework', 'protocol', 'driver',
  'library for', 'utilities', 'script', 'plugin', 'agreement',
];
const LIB_NAME = /^(lib|lib[0-9]|perl|python-?|node-?|ruby|haskell|php|guile|ocaml|dotnet|julia)/i;

function classify(r) {
  const d = (r.description || '').toLowerCase();
  const hasMain = !!r.mainProgram;
  let app = 0;
  let cli = 0;
  for (const h of APP_HINTS) if (d.includes(h)) app += 2;
  for (const h of CLI_HINTS) if (d.includes(h)) cli += 2;
  if (hasMain) app += 2;
  if (LIB_NAME.test(r.name)) cli += 4;
  // strong lib sentence markers
  if (/\b(a )?library\b|development library|(python|node|perl|ruby) (module|package)/.test(d)) cli += 5;
  // Calibrated so the app-candidate count lands in the measured coverage band
  // (13.0% ± 3.8% of derivations ship a desktop entry; threshold 3 reproduces it).
  if (cli >= app + 3) return { cls: 'excluded-lib', conf: 0.6, tier: 'heuristic' };
  if (app - cli >= 3) {
    const conf = Math.min(0.95, 0.5 + (app - cli) / (app + cli + 6));
    return { cls: 'app', conf, tier: 'heuristic' };
  }
  return { cls: 'cli', conf: 0.45, tier: 'heuristic' };
}

const linuxOK = (r) => (r.platforms || []).some((p) => /(x86_64|aarch64|i686)-linux/.test(p));

const entries = [];
let excludedLibs = 0;
for (const r of drv) {
  const { cls, conf, tier } = classify(r);
  if (cls === 'excluded-lib') { excludedLibs++; continue; }
  entries.push({
    id: r.name,
    attr: r.name,
    pname: r.pname,
    version: r.version,
    description: r.description,
    homePage: r.homePage,
    licenseOk: r.licenseOk,
    linuxOK: linuxOK(r),
    mainProgram: r.mainProgram,
    class: cls,          // 'app' | 'cli'
    tier,                // 'heuristic' (desktop-evidence tiering pending)
    confidence: +conf.toFixed(2),
    install: { attr: r.name, scopeDefault: 'user' },
  });
}

const index = {
  schemaVersion: '0.1',
  generated: {
    pin: 'sha256-qXFIqA6FCcx7XeDzw0ZNlkDT9UiCzrNDwK8RuU8RcHM=',
    date: new Date().toISOString(),
    note: 'classification is heuristic-tier (decision D16 CLI class applied); desktop-evidence (rich/basic) tiering is the coverage-pipeline upgrade path',
  },
  counts: {
    appCandidates: entries.filter((e) => e.class === 'app').length,
    cliTools: entries.filter((e) => e.class === 'cli').length,
    excludedLibraries: excludedLibs,
    installableOnLinux: entries.filter((e) => e.linuxOK).length,
  },
  entries,
};

fs.writeFileSync(path.join(root, 'results/catalogue-index.json'), JSON.stringify(index));
console.log(JSON.stringify(index.counts, null, 2));
console.log(`index: results/catalogue-index.json (${(fs.statSync(path.join(root, 'results/catalogue-index.json')).size / 1048576).toFixed(1)} MB)`);