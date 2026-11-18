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
const EV_CLI = path.join(root, 'results/evidence-cli.jsonl'); // nus.5 recovery source (optional)
const write = process.argv.includes('--write');

// known-apps recovery source: recall.js KNOWN is the manually-validated list
// of well-known GUI applications (single source of truth, no list duplication).
const recallSrc = fs.readFileSync(path.join(root, 'recall.js'), 'utf8');
const KNOWN_ATTRS = new Set();
for (const m of recallSrc.matchAll(/\[['"]([^'"]+)['"],\s*['"]([^'"]+)['"]\]/g)) KNOWN_ATTRS.add(m[2]);
// recall-list entries that are deliberately not tile-able apps (TUIs)
const KNOWN_DENY = new Set(['htop', 'btop']);

const idx = JSON.parse(fs.readFileSync(IDX, 'utf8'));
const load = (file) => {
  const m = new Map();
  if (!fs.existsSync(file)) return m;
  for (const l of fs.readFileSync(file, 'utf8').split('\n').filter(Boolean)) {
    let j;
    try { j = JSON.parse(l); } catch { continue; } // ignore a torn/partial line
    m.set(j.attr, j);
  }
  return m;
};
const ev = load(EV);
const evCli = load(EV_CLI);

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
  // known-apps (recall.js KNOWN = manual QA basis) are dominant: promote
  // mislabels out of cli/term-app/unverified, and never let a weak evidence
  // row demote an already-app entry back to unverified (idempotent).
  if (KNOWN_ATTRS.has(e.attr) && !KNOWN_DENY.has(e.attr)) {
    if (e.class !== 'app') {
      changed++;
      out.push({
        ...e,
        class: 'app',
        tier: 'unverified',
        note: 'known GUI app (recall list); no cache evidence yet',
      });
    } else {
      out.push(e);
    }
    continue;
  }
  // nus.5: recover real apps that carry fetched desktop evidence in the cli-recovery
  // pool, regardless of their heuristic class (rich/basic -> app)
  const cliRow = evCli.get(e.attr);
  if (cliRow) {
    if (cliRow.tier === 'rich') { changed++; out.push({ ...e, class: 'app', tier: 'rich' }); continue; }
    if (cliRow.tier === 'basic') { changed++; out.push({ ...e, class: 'app', tier: 'basic' }); continue; }
    // D17: an unfree app in the recovery pool is listed, not tile-hidden
    if (cliRow.tier === 'unfree-deferred') { changed++; out.push({ ...e, class: 'app', tier: 'unfree-deferred' }); continue; }
  }
  if (e.class === 'cli') { out.push(e); continue; }
  if (e.class === 'term-app') { out.push(e); continue; }
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