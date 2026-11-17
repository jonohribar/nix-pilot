#!/usr/bin/env node
// Statistical coverage sample: random derivations, fetchable from cache only.
// For each: dry-run first (skip anything that would build locally or exceeds
// the size cap), then fetch and scan for .desktop / metainfo. Gives a
// defensible GUI-candidate rate estimate on modest hardware.
//
// Usage: node sample-coverage.js [N=250] [seed=1]
const fs = require('fs');
const path = require('path');
const cp = require('child_process');

const N = Number(process.argv[2] || 250);
const SEED = Number(process.argv[3] || 1);
const SIZE_CAP_MB = Number(process.argv[4] || 150);
const DRY_TIMEOUT_MS = 120000;
const FETCH_TIMEOUT_MS = 300000;

const root = path.join(__dirname);
const recs = JSON.parse(fs.readFileSync(path.join(root, 'results/records.json'), 'utf8'));
const pool = recs.filter((r) => r.isDerivation && !r.evalError).map((r) => r.name);
const OUT = path.join(root, 'results/coverage-sample.json');
const OUTL = path.join(root, 'results/coverage-sample.jsonl');

// deterministic PRNG (mulberry32)
function rng(seed) {
  let a = seed >>> 0;
  return () => {
    a |= 0; a = (a + 0x6D2B79F5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / Math.pow(2, 32);
  };
}
const rand = rng(SEED);
const sample = [];
const seen = new Set();
while (sample.length < N) {
  const name = pool[Math.floor(rand() * pool.length)];
  if (!seen.has(name)) { seen.add(name); sample.push(name); }
}

const run = (cmd, tmo) => {
  try {
    return cp.execSync(cmd, { encoding: 'utf8', timeout: tmo, maxBuffer: 128 * 1024 * 1024, stdio: ['ignore', 'pipe', 'pipe'] });
  } catch (e) { return null; }
};

const parseDry = (txt) => {
  // Empty output = nothing to fetch or build (already in the local store).
  if (!txt || !txt.trim()) return { kind: 'local', mb: 0 };
  const m = txt.match(/these \d+ paths will be fetched \(([\d.]+) (?:MiB|KiB) download/);
  if (m) {
    const v = parseFloat(m[1]);
    const mb = m[2] === 'KiB' ? v / 1024 : v;
    return { kind: 'fetch', mb };
  }
  if (/will be built/.test(txt)) return { kind: 'build' };
  // nothing to fetch/build: outputs already in the local store
  if (/\/nix\/store\//.test(txt)) return { kind: 'local', mb: 0 };
  return { kind: 'error' };
};

const lines = [];
for (const name of sample) {
  let item = { attr: name, ts: new Date().toISOString() };
  const dry = run(`nix build --dry-run nixpkgs#${name} 2>&1`, DRY_TIMEOUT_MS);
  const d = parseDry(dry);
  if (d.kind === 'build') item.status = 'build-needed';
  else if ((d.kind === 'fetch' || d.kind === 'local') && d.mb > SIZE_CAP_MB) item.status = 'too-large';
  else if (d.kind !== 'fetch' && d.kind !== 'local') item.status = 'error';
  else {
    item.status = 'fetched';
    item.mb = d.mb; item.local = d.kind === 'local';
    const outs = run(`nix build --no-link --print-out-paths nixpkgs#${name} 2>&1`, FETCH_TIMEOUT_MS);
    if (!outs) { item.status = 'fetch-failed'; }
    else {
      let desks = 0, metainfo = 0;
      for (const o of outs.trim().split('\n').filter(Boolean)) {
        const df = run(`find "${o}" -name '*.desktop' 2>/dev/null | wc -l`, 30000);
        const mf = run(`find "${o}" -path '*/share/metainfo/*' \\( -name '*.xml' -o -name '*.metainfo' \\) 2>/dev/null | wc -l`, 30000);
        desks += parseInt(df || '0', 10) || 0;
        metainfo += parseInt(mf || '0', 10) || 0;
      }
      item.desktops = desks;
      item.metainfo = metainfo;
    }
  }
  lines.push(item);
  fs.appendFileSync(OUTL, JSON.stringify(item) + '\n');
  process.stdout.write(`  ${item.attr.padEnd(30)} -> ${item.status}${item.mb ? ' (' + item.mb.toFixed(1) + ' MiB)' : ''}${item.desktops ? ' desks=' + item.desktops : ''}\n`);
}
fs.writeFileSync(OUT, JSON.stringify(lines, null, 2));
console.log('\nwrote', OUT, `(${lines.length} items)`);