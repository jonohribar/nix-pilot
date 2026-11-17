#!/usr/bin/env node
// Evidence-driven reclassification (nus.4 final step): overlays the fetched
// output-evidence tiers from results/evidence.jsonl onto the heuristic index
// and emits an evidence-tiered catalogue-index.json.
//
// App-candidate rules:
//   rich / basic        -> class 'app',   tier rich | basic       (tile-able)
//   unfree-deferred     -> class 'app',   tier unfree-deferred    (D17: listed, no cache route)
//   uncertain / too-large / build-needed / error / fetch-failed
//                       -> class 'unverified', tier 'unverified'  (D-N1: labelled group, never a tile)
//   alias-missing       -> dropped                               (not installable via the flake; D10)
// An app-candidate with no evidence row yet -> 'unverified' (cannot be trusted as a tile).
// term-app and cli entries are unchanged (no evidence pass targets them).
//
// Usage: node classify-evidence.js [--write]   (prints plan; --write overwrites the index)
const fs = require('fs');
const path = require('path');

const root = path.join(__dirname);
const IDX = path.join(root, 'results/catalogue-index.json');
const EV = path.join(root, 'results/evidence.jsonl');
const write = process.argv.includes('--write');

const idx = JSON.parse(fs.readFileSync(IDX, 'utf8'));
const ev = new Map();
for (const l of fs.readFileSync(EV, 'utf8').split('\n').filter(Boolean)) {
  let j;
  try { j = JSON.parse(l); } catch { continue; } // ignore a torn/partial concurrent line
  ev.set(j.attr, j);
}

// tier -> (class, tier, note)
const MAP = {
  rich: ['app', 'rich', null],
  basic: ['app', 'basic', null],
  'unfree-deferred': ['app', 'unfree-deferred', null],
  'uncertain-no-evidence': ['unverified', 'unverified', null],
  'too-large': ['unverified', 'unverified', 'too-large'],
  'build-needed': ['unverified', 'unverified', 'build-needed'],
  error: ['unverified', 'unverified', 'error'],
  'fetch-failed': ['unverified', 'unverified', 'fetch-failed'],
  'alias-missing': ['DROP', null, null],
};

let dropped = 0;
let changed = 0;
const out = [];
for (const e of idx.entries) {
  if (e.class === 'term-app' || e.class === 'cli') { out.push(e); continue; }
  const row = ev.get(e.attr);
  let cls = e.class, tier = e.tier, note = e.note;
  if (e.class === 'app') {
    if (!row) { cls = 'unverified'; tier = 'unverified'; note = 'unevidenced'; }
    else {
      const [c, t, n] = MAP[row.tier] || ['unverified', 'unverified', row.tier];
      if (c === 'DROP') { dropped++; continue; }
      cls = c; tier = t; note = n;
    }
    if (cls !== e.class || tier !== e.tier) changed++;
    out.push({ ...e, class: cls, tier, ...(note ? { note } : {}) });
  } else {
    out.push(e);
  }
}

const unverified = out.filter((e) => e.class === 'unverified').length;
const next = {
  ...idx,
  generated: { ...idx.generated, date: new Date().toISOString(), note: 'evidence-tiered (nus.4): rich/basic from fetched .desktop/metainfo; uncertain/unevidenced -> unverified group; unfree -> unfree-deferred (D17); alias-missing dropped (D10)' },
  counts: {
    appCandidates: out.filter((e) => e.class === 'app').length,
    unverifiedApps: unverified,
    terminalApps: out.filter((e) => e.class === 'term-app').length,
    cliTools: out.filter((e) => e.class === 'cli').length,
    excludedLibraries: idx.counts.excludedLibraries,
    installableOnLinux: out.filter((e) => e.linuxOK).length,
    unfreeShipped: out.filter((e) => e.unfree).length,
  },
  entries: out,
};

if (write) {
  fs.writeFileSync(IDX, JSON.stringify(next));
}

console.log(JSON.stringify({
  schemaVersion: idx.schemaVersion,
  entries: out.length,
  droppedAlias: dropped,
  changedAppTier: changed,
  byTier: out.reduce((a, e) => { a[e.tier] = (a[e.tier] || 0) + 1; return a; }, {}),
  appCandidates: out.filter((e) => e.class === 'app').length,
  unverifiedApps: unverified,
  terminalApps: out.filter((e) => e.class === 'term-app').length,
  cliTools: out.filter((e) => e.class === 'cli').length,
  wrote: write,
}, null, 2));