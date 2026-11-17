#!/usr/bin/env node
// Precision/recall from the QA sample. Parses the "My score" column of
// sample-v1.md (user-approved, skip = agree) and computes join metrics.
//
// Usage: node qa/score.js
const fs = require('fs');
const path = require('path');

const md = fs.readFileSync(path.join(__dirname, 'sample-v1.md'), 'utf8');
const rows = md.split('\n').filter((l) => /^\| \d+ \|/.test(l));
const verdicts = rows
  .map((l) => {
    const t = l.split('|').map((c) => c.trim());
    // 0=empty,1=#,2=name,3=match,4=tier,5=desc,6=My score,7=You
    const m = (t[6] || '').match(/^(GOOD|WRONG|NOT-APP|UNSURE|CORRECT-ABSENT|MISSING)/);
    return m ? m[1] : 'UNSCORED';
  });

const cnt = verdicts.reduce((a, v) => { a[v] = (a[v] || 0) + 1; return a; }, {});
const n = verdicts.length;
const GOOD = cnt.GOOD || 0;
const WRONG = cnt.WRONG || 0;
const MISSING = cnt.MISSING || 0;
const UNSURE = cnt.UNSURE || 0;
const NOTAPP = cnt['NOT-APP'] || 0;

// Known apps that should be findable = GOOD + WRONG (matched) + MISSING (real apps absent).
// UNSURE (feh) is an app-like find; reported both ways.
const knownApps = GOOD + WRONG + MISSING;
const recall = GOOD / knownApps;                                    // excluding feh
const recallWithUnsure = (GOOD + UNSURE) / (knownApps + UNSURE);    // counting feh as found
// Precision among rows the store would present as an app.
const precisionAsApp = GOOD / (GOOD + WRONG);
// Precision IF CLIs/libs were naively presented as apps (no classification).
const precisionNoClass = GOOD / (GOOD + WRONG + NOTAPP);

const out = {
  sample: n,
  verdicts: cnt,
  ignoredUnscored: cnt.UNSCORED || 0,
  knownApps,
  recall: +recall.toFixed(3),
  recallCountingFehUnsure: +recallWithUnsure.toFixed(3),
  precisionPresentedAsApp: +precisionAsApp.toFixed(3),
  precisionIfNoClassification: +precisionNoClass.toFixed(3),
};
console.log(JSON.stringify(out, null, 2));