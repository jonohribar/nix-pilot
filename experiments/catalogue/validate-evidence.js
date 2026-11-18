#!/usr/bin/env node
// Post-evidence sanity check (nus.4): for each well-known GUI application in
// the recall sample, report the class/tier it landed in after evidence
// reclassification. Real GUI apps should be class 'app' (rich/basic, or
// unfree-deferred for commercial apps). Ones that fell into 'unverified' or
// 'cli' (or are missing) are flagged for review — they may be genuinely
// unevidenced (demotion hazard) or real demotion errors.
//
// Usage: node validate-evidence.js   (run after classify-evidence.js --write)
const fs = require('fs');
const idx = JSON.parse(fs.readFileSync('results/catalogue-index.json', 'utf8'));
const recall = fs.readFileSync('recall.js', 'utf8');
const knownAttrs = [...recall.matchAll(/\['([^']+)',\s*'([^']+)'\]/g)].map((m) => m[2]);
const by = new Map(idx.entries.map((e) => [e.attr, e]));

const cat = (e) => `${e.class}/${e.tier}`;
const ok = (e) => e && e.class === 'app'; // rich/basic/unfree-deferred shown as applications
const bad = [];

for (const attr of [...new Set(knownAttrs)]) {
  const e = by.get(attr);
  if (!e) { bad.push({ attr, status: 'MISSING from index' }); continue; }
  if (!ok(e)) bad.push({ attr, status: cat(e), desc: (e.description || '').slice(0, 50) });
}

// Inventory summary over all entries.
const inv = { total: idx.entries.length };
for (const e of idx.entries) inv[cat(e)] = (inv[cat(e)] || 0) + 1;

console.log(JSON.stringify({
  knownAppsChecked: [...new Set(knownAttrs)].length,
  flaggedForReview: bad.length,
  flags: bad,
  inventory: inv,
}, null, 2));

if (bad.length) { console.error(`\n[note] ${bad.length} known app(s) not class 'app' — review these (some are expected: htop/btop are TUIs, discord/slack/spotify are unfree).`); }