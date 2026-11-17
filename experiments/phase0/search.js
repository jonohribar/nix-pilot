#!/usr/bin/env node
// First searchable catalogue: ranked search over the exported metadata or the
// generated index (results/catalogue-index.json when present). Query terms
// score against attribute path, pname and description; results are grouped by
// type (app vs command-line tool, decision D16) and bounded.
//
// Usage: node search.js "<query>" [--hide-unfree]  |  node search.js --stats [--hide-unfree]
// --hide-unfree excludes known-unfree applications (opt-in filter, D1/l3q.2);
// the signal comes from the generated index entry's unfree flag.
const fs = require('fs');
const path = require('path');

const idxPath = path.join(__dirname, 'results/catalogue-index.json');
const idx = fs.existsSync(idxPath) ? JSON.parse(fs.readFileSync(idxPath, 'utf8')) : null;
const recs = idx
  ? idx.entries
  : JSON.parse(fs.readFileSync(path.join(__dirname, 'results/records.json'), 'utf8')).filter((r) => r.isDerivation && !r.evalError);
const toks = (q) => q.toLowerCase().split(/[^a-z0-9.+-]+/).filter(Boolean);

function score(rec, terms) {
  const nameT = (rec.name || rec.attr || '').toLowerCase();
  const pn = (rec.pname || '').toLowerCase();
  const desc = (rec.description || '').toLowerCase();
  let s = 0;
  for (const t of terms) {
    if (t.length < 2) continue;
    if (nameT === t || pn === t) s += 100;
    else if (nameT.startsWith(t) || pn.startsWith(t)) s += 30;
    else if (nameT.includes(t) || pn.includes(t)) s += 10;
    if (desc.includes(t)) s += 3;
  }
  return s;
}

const drv = recs.filter((r) => !(idx && r.class === undefined) || r.class);

const hideUnfree = process.argv.includes('--hide-unfree');

if (process.argv[2] === '--stats') {
  const linuxOK = (r) => (r.platforms || []).some((p) => /(x86_64|aarch64|i686)-linux/.test(p));
  const stats = idx
    ? {
        catalogue: 'index', schemaVersion: idx.schemaVersion, generated: idx.generated.date,
        unfree: idx.entries.filter((r) => r.unfree).length, ...idx.counts,
      }
    : {
        catalogue: 'raw metadata',
        cataloguedDocuments: drv.length,
        withPname: drv.filter((r) => r.pname).length,
        withVersion: drv.filter((r) => r.version).length,
        withDescription: drv.filter((r) => r.description).length,
        withHomePage: drv.filter((r) => r.homePage).length,
        withLicense: drv.filter((r) => r.licenseOk).length,
        withMainProgram: drv.filter((r) => r.mainProgram).length,
        installableOnLinux: drv.filter(linuxOK).length,
        evalErrorEntries: recs.filter((r) => r.evalError).length,
      };
  console.log(JSON.stringify(stats, null, 2));
  process.exit(0);
}

const q = process.argv.slice(2).filter((a) => !a.startsWith('--')).join(' ').trim();
if (!q) {
  console.error('usage: node search.js "<query>"  |  node search.js --stats');
  process.exit(1);
}
const terms = toks(q);
const ranked = drv
  .filter((r) => !hideUnfree || !r.unfree)
  .map((r) => [score(r, terms), r])
  .filter((x) => x[0] > 0)
  .sort((a, b) => b[0] - a[0])
  .slice(0, 15);

console.log(JSON.stringify(
  ranked.map(([s, r]) => ({
    score: s,
    group: r.class === 'cli' ? 'command-line tool' : r.class === 'term-app' ? 'terminal application' : 'application',
    attr: r.name || r.attr,
    pname: r.pname,
    version: r.version,
    description: (r.description || '').slice(0, 90),
    licenseOk: r.licenseOk,
    unfree: !!r.unfree,
    linuxOK: (r.platforms || []).some((p) => /(x86_64|aarch64|i686)-linux/.test(p)),
  })),
  null, 2,
));