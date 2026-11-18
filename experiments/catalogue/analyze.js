#!/usr/bin/env node
// Phase 0 measurement analyzer. Reads records.json produced by measure.sh
// and prints a measurement summary. Usage: analyze.js <start_ms> <end_ms>
const fs = require('fs');
const path = require('path');

const root = path.join(__dirname, 'results');
const recs = JSON.parse(fs.readFileSync(path.join(root, 'records.json'), 'utf8'));
const [, , s0, s1] = process.argv;
// measure.sh passes epoch-seconds.millis; diff is already in seconds.
const seconds = Math.max(0, (parseFloat(s1) - parseFloat(s0)) || 0);

const total = recs.length;
const drv = recs.filter((r) => r.isDerivation);
const exportedOk = drv.filter((r) => r.pname != null && r.version != null);
const present = (n) => drv.filter((r) => r[n] != null && r[n] !== '').length;

const out = {
  pin: { narHash: process.env.PIN_HASH || 'sha256-qXFIqA6FCcx7XeDzw0ZNlkDT9UiCzrNDwK8RuU8RcHM=' },
  system: 'x86_64-linux',
  sampleSize: total,
  seconds: +seconds.toFixed(2),
  recordsPerSecond: total && seconds > 0 ? +(total / seconds).toFixed(1) : null,
  derivations: drv.length,
  nonDerivations: total - drv.length,
  exportedOk: exportedOk.length,
  // field completeness among derivations
  pnamePresent: present('pname'),
  versionPresent: present('version'),
  descriptionPresent: present('description'),
  homePagePresent: present('homePage'),
  platformsPresent: present('platforms'),
  licenseOk: drv.filter((r) => r.licenseOk).length,  licenseUnknown: drv.filter((r) => r.licenseOk === false && r.licenseType === 'unreadable').length,
  mainProgramPresent: present('mainProgram'),
};

console.log(JSON.stringify(out, null, 2));