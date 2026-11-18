#!/usr/bin/env node
// Tier-split sample: fetch desktop-entry/metainfo evidence for a random sample
// of app candidates from the generated index, and classify each as rich
// (metainfo), basic (.desktop) or uncertain (no output evidence). This is the
// upgrade path for the v1 heuristic classification: it measures how the app
// universe splits into tiers. Resumable (results/tier-sample.jsonl); error,
// fetch-failed and unscanned-local items are re-queued on rerun.
//
// Usage: node tier-sample.js [N=40] [seed=3]
const fs = require('fs');
const path = require('path');
const cp = require('child_process');

const N = Number(process.argv[2] || 40);
const SEED = Number(process.argv[3] || 3);
const SIZE_CAP_MB = 500;
const DRY_TIMEOUT = 120000;
const FETCH_TIMEOUT = 300000;
const SCAN_TIMEOUT = 30000;

const root = path.join(__dirname);
const idx = JSON.parse(fs.readFileSync(path.join(root, 'results/catalogue-index.json'), 'utf8'));
const pool = idx.entries.filter((e) => e.class === 'app').map((e) => e.attr);
const OUTL = path.join(root, 'results/tier-sample.jsonl');

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
const seen = new Set();
const retry = [];
if (fs.existsSync(OUTL)) {
  for (const l of fs.readFileSync(OUTL, 'utf8').split('\n').filter(Boolean)) {
    const j = JSON.parse(l);
    seen.add(j.attr);
    if (['error', 'fetch-failed', 'local', 'uncertain-no-evidence'].includes(j.tier)) retry.push(j.attr);
  }
}
const sample = [...retry];
while (sample.length < N && seen.size < pool.length) {
  const name = pool[Math.floor(rand() * pool.length)];
  if (!seen.has(name)) { seen.add(name); sample.push(name); }
}
console.log(`tier sample: ${retry.length} re-queued, running ${sample.length} of target ${N} (pool ${pool.length})`);

const run = (cmd, tmo) => {
  try {
    return cp.execSync(cmd, { encoding: 'utf8', timeout: tmo, maxBuffer: 128 * 1024 * 1024, stdio: ['ignore', 'pipe', 'pipe'] });
  } catch (e) { return null; }
};

for (const name of sample) {
  const dry = run(`nix build --dry-run nixpkgs#${name} 2>&1`, DRY_TIMEOUT);
  let item = { attr: name, ts: new Date().toISOString() };
  let plan; // {mb} | 'build' | 'alias' | 'err'

  if (dry && /does not provide attribute/.test(dry)) plan = 'alias';
  else {
    const m = dry && dry.match(/(?:these \d+ paths|this path) will be fetched \(([\d.]+) (?:MiB|KiB) download/);
    if (!dry || !dry.trim()) plan = { mb: 0 };
    else if (m) { const v = parseFloat(m[1]); plan = { mb: m[2] === 'KiB' ? v / 1024 : v }; }
    else if (/\/nix\/store\//.test(dry)) plan = { mb: 0 };
    else if (/will be built/.test(dry)) plan = 'build';
    else plan = 'err';
  }

  if (plan === 'alias') item.tier = 'alias-missing';
  else if (plan === 'build') item.tier = 'build-needed';
  else if (plan === 'err') item.tier = 'error';
  else if (plan.mb > SIZE_CAP_MB) { item.tier = 'too-large'; item.mb = +plan.mb.toFixed(0); }
  else {
    item.mb = +plan.mb.toFixed(1);
    const outs = run(`nix build --no-link --print-out-paths nixpkgs#${name} 2>&1`, FETCH_TIMEOUT);
    if (!outs) item.tier = 'fetch-failed';
    else {
      let desks = 0, metas = 0;
      for (const o of outs.trim().split('\n').filter(Boolean)) {
        const df = run(`find "${o}" -name '*.desktop' 2>/dev/null | wc -l`, SCAN_TIMEOUT);
        const mf = run(`find "${o}" -path '*/share/metainfo/*' \\( -name '*.xml' -o -name '*.metainfo' \\) 2>/dev/null | wc -l`, SCAN_TIMEOUT);
        desks += parseInt(df || '0', 10) || 0;
        metas += parseInt(mf || '0', 10) || 0;
      }
      item.desktops = desks;
      item.metainfo = metas;
      item.tier = metas > 0 ? 'rich' : desks > 0 ? 'basic' : 'uncertain-no-evidence';
    }
  }

  fs.appendFileSync(OUTL, JSON.stringify(item) + '\n');
  process.stdout.write(`  ${name.padEnd(28)} -> ${item.tier}${item.mb ? ' (' + item.mb + ' MiB)' : ''}${item.desktops ? ' desks=' + item.desktops : ''}${item.metainfo ? ' meta=' + item.metainfo : ''}\n`);
}
console.log('chunk complete; totals in results/tier-sample.jsonl');