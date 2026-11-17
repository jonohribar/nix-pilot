#!/usr/bin/env node
// First searchable catalogue: ranked search over the exported metadata
// (results/records.json). Query terms score against attribute path, pname and
// description; results are ranked and bounded. This is the "a user can search
// one term and receive relevant application results" acceptance probe.
//
// Usage: node search.js "<query>" | node search.js --stats
const fs = require('fs');
const path = require('path');

const recs = JSON.parse(fs.readFileSync(path.join(__dirname, 'results/records.json'), 'utf8'));
const drv = recs.filter((r) => r.isDerivation && !r.evalError);
const toks = (q) => q.toLowerCase().split(/[^a-z0-9.+-]+/).filter(Boolean);

function score(rec, terms) {
  const nameT = (rec.name || '').toLowerCase();
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

if (process.argv[2] === '--stats') {
  const linuxOK = (r) => (r.platforms || []).some((p) => /(x86_64|aarch64|i686)-linux/.test(p));
  const stats = {
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

const q = process.argv.slice(2).join(' ').trim();
if (!q) {
  console.error('usage: node search.js "<query>"  |  node search.js --stats');
  process.exit(1);
}
const terms = toks(q);
const ranked = drv
  .map((r) => [score(r, terms), r])
  .filter((x) => x[0] > 0)
  .sort((a, b) => b[0] - a[0])
  .slice(0, 15);

console.log(JSON.stringify(
  ranked.map(([s, r]) => ({
    score: s,
    attr: r.name,
    pname: r.pname,
    version: r.version,
    description: (r.description || '').slice(0, 90),
    licenseOk: r.licenseOk,
    linuxOK: (r.platforms || []).some((p) => /(x86_64|aarch64|i686)-linux/.test(p)),
  })),
  null, 2,
));