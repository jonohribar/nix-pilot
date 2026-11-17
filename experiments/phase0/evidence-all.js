#!/usr/bin/env node
// Full evidence pass (nus.4): fetch desktop-entry / AppStream-metainfo output
// evidence for EVERY app candidate in the generated index and tier it:
//    rich   = ships share/metainfo AppStream XML  (highest-confidence app)
//    basic  = ships a .desktop entry              (solid app)
//    uncertain-no-evidence = no output evidence    (demoted to D16 group later)
// plus too-large / build-needed / alias-missing / unfree-deferred / error / fetch-failed.
// unfree-deferred is terminal (D17: no public-cache route, enrichment deferred).
//
// Resumable (results/evidence.jsonl, append-only): items already recorded are
// skipped; error + fetch-failed re-queued each run so a stopped chunk loses
// nothing. Runs a bounded LIMIT of NEW items per invocation then stops cleanly
// so it can be called repeatedly in stop-and-resume chunks.
//
// Usage: node evidence-all.js [LIMIT=200] [--bucket I] [--nbuckets N]
// When --bucket/--nbuckets are given, only attrs whose stable hash falls in
// bucket I of N are processed, so several instances can run in parallel on
// disjoint pools into the same append-only evidence.jsonl (O_APPEND line writes
// are atomic). A normal run (no flags) is fully sequential.
const fs = require('fs');
const path = require('path');
const cp = require('child_process');

const LIMIT = Number(process.argv[2] || 200);
const argB = process.argv.indexOf('--bucket');
const argN = process.argv.indexOf('--nbuckets');
const B = argB >= 0 ? Number(process.argv[argB + 1]) : 0;
const NB = argN >= 0 ? Number(process.argv[argN + 1]) : 1;
// Stable string hash so a bucket partition is deterministic across runs.
const hash = (s) => { let x = 0; for (let i = 0; i < s.length; i++) x = ((x * 31) + s.charCodeAt(i)) >>> 0; return x; };
const inBucket = (a) => (NB <= 1 || hash(a) % NB === B);
const SIZE_CAP_MB = 500;
const DRY_TIMEOUT = Number(process.env.EV_DRY_MS || 120000);
const FETCH_TIMEOUT = Number(process.env.EV_FETCH_MS || 300000);
const SCAN_TIMEOUT = Number(process.env.EV_SCAN_MS || 30000);

const root = path.join(__dirname);
const idx = JSON.parse(fs.readFileSync(path.join(root, 'results/catalogue-index.json'), 'utf8'));
const pool = idx.entries.filter((e) => e.class === 'app').map((e) => e.attr);
const OUTL = path.join(root, 'results/evidence.jsonl');

// Capture output even on non-zero exit (unfree/alias/eval errors exit non-zero but
// their diagnostics live in stderr, which execSync would swallow on throw).
const run = (cmd, tmo) => {
  const r = cp.spawnSync('bash', ['-lc', cmd], { encoding: 'utf8', timeout: tmo, maxBuffer: 128 * 1024 * 1024 });
  if (r.error) return null;
  return (r.stdout || '') + (r.stderr || '');
};

// Recover prior state: which attrs are already tiered, and which should retry.
const seen = new Set();
const retry = new Set();
if (fs.existsSync(OUTL)) {
  for (const l of fs.readFileSync(OUTL, 'utf8').split('\n').filter(Boolean)) {
    let j;
    try { j = JSON.parse(l); } catch { continue; } // ignore a torn/partial line
    seen.add(j.attr);
    if (inBucket(j.attr) && (j.tier === 'error' || j.tier === 'fetch-failed')) retry.add(j.attr);
  }
}

// Work queue: this bucket's re-queued failures first, then its unprocessed attrs.
const queue = [...retry, ...pool.filter((a) => inBucket(a) && !seen.has(a))];
const toRun = queue.slice(0, LIMIT);
console.log(`evidence-all: ${retry.size} re-queued, running ${toRun.length} (limit ${LIMIT}; ${queue.length - toRun.length} still queued of ${pool.length} app candidates)`);

for (const name of toRun) {
  const dry = run(`nix build --dry-run nixpkgs#${name} 2>&1`, DRY_TIMEOUT);
  let item = { attr: name, ts: new Date().toISOString() };
  let plan;

  if (dry && /has an unfree license/.test(dry)) plan = 'unfree'; // D17: no cache route, never evidence-tierable
  else if (dry && /does not provide attribute/.test(dry)) plan = 'alias';
  else {
    const m = dry && dry.match(/(?:these \d+ paths|this path) will be fetched \(([\d.]+) (?:MiB|KiB) download/);
    if (!dry || !dry.trim()) plan = { mb: 0 };
    else if (m) { const v = parseFloat(m[1]); plan = { mb: m[2] === 'KiB' ? v / 1024 : v }; }
    else if (/\/nix\/store\//.test(dry)) plan = { mb: 0 };
    else if (/will be built/.test(dry)) plan = 'build';
    else plan = 'err';
  }

  if (plan === 'alias') item.tier = 'alias-missing';
  else if (plan === 'unfree') item.tier = 'unfree-deferred';
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
  process.stdout.write(`  ${(name || '').padEnd(28)} -> ${item.tier}${item.mb ? ' (' + item.mb + ' MiB)' : ''}${item.desktops ? ' desks=' + item.desktops : ''}${item.metainfo ? ' meta=' + item.metainfo : ''}\n`);
}
console.log('chunk complete; full pass resumes from results/evidence.jsonl');