#!/usr/bin/env node
// Heuristic classification probe: does an evidence-based keyword classifier
// distinguish GUI-app candidates from libraries/CLIs, with measurable
// confidence? Validated against the QA sample v1 labels (GOOD vs NOT-APP).
//
// Signals available without output inspection: mainProgram presence and the
// description text. Research-stage; output-evidence (desktop entries) would
// replace it once the coverage scan runs.
//
// Usage: node classify.js
const fs = require('fs');
const path = require('path');

const recs = JSON.parse(fs.readFileSync(path.join(__dirname, 'results/records.json'), 'utf8'));
const byAttr = new Map(recs.filter((r) => r.isDerivation && !r.evalError).map((r) => [r.name, r]));

const APP_HINTS = [
  'browser', 'editor', 'viewer', 'player', 'manager', 'client', 'suite', 'desktop',
  'graphical', 'graphics', 'gui', 'environment', 'shell', 'game', 'studio',
  'terminal emulator', 'image viewer', 'media', 'office', 'chat', 'messenger',
  'note', 'reader', 'player for', 'image editor', 'video',
];
const CLI_HINTS = [
  'library', 'a library', 'development', 'dev tool', 'command-line', 'command line',
  'cli ', 'header', 'runtime', 'daemon', 'server', 'sdk', 'binding', 'api', 'module',
  'tool for developers', 'compiler', 'kernel', 'framework', 'protocol',
  'utilities', 'script', 'driver', 'plugin', 'python', 'javascript module',
];

const classify = (r) => {
  const d = (r.description || '').toLowerCase();
  const hasMain = !!r.mainProgram;
  let app = 0;
  let cli = 0;
  for (const h of APP_HINTS) if (d.includes(h)) app += 1;
  for (const h of CLI_HINTS) if (d.includes(h)) cli += 2;
  if (hasMain) app += 1;
  // name-adjacent CLI tell: attr named like a plain tool that's also a CLI
  if (/^(lib|lib|python|node|ruby|perl|haskell)/.test(r.name)) cli += 3;
  // "for X" describing what it is for, often lib-ish
  if (/for developers|for programming|binding for|generation for/.test(d)) cli += 2;
  const conf = Math.abs(app - cli);
  const verdict = app > cli ? 'APP' : cli > app ? 'CLI' : 'UNSURE';
  return { verdict, conf, d: d.slice(0, 50) };
};

// QA labels: read sample-v1.md, map attr -> expected class (GOOD => APP, NOT-APP => CLI).
const md = fs.readFileSync(path.join(__dirname, 'qa/sample-v1.md'), 'utf8');
const labels = [];
for (const l of md.split('\n').filter((x) => /^\| \d+ \|/.test(x))) {
  const c = l.split('|').map((s) => s.trim());
  const attr = c[3] && c[3].startsWith('`') ? c[3].replace(/`/g, '') : null;
  const score = (c[6] || '').match(/^(GOOD|NOT-APP)/);
  if (attr && score && byAttr.has(attr)) {
    labels.push({ attr, want: score[1] === 'GOOD' ? 'APP' : 'CLI' });
  }
}

let tp = 0, fp = 0, tn = 0, fn = 0, uns = 0;
const wrong = [];
for (const { attr, want } of labels) {
  const { verdict, conf } = classify(byAttr.get(attr));
  if (verdict === 'UNSURE') { uns++; continue; }
  const isApp = verdict === 'APP';
  if (isApp && want === 'APP') tp++;
  else if (isApp && want === 'CLI') { fp++; wrong.push([attr, 'FALSE-APP', conf]); }
  else if (!isApp && want === 'CLI') tn++;
  else { fn++; wrong.push([attr, 'FALSE-CLI', conf]); }
}
const acc = (tp + tn) / Math.max(1, tp + tn + fp + fn);
const prec = tp / Math.max(1, tp + fp);
const recAll = tp / Math.max(1, tp + fn);
console.log(JSON.stringify({
  labelledRows: labels.length,
  unsure: uns,
  trueApp: tp, falseApp: fp, trueCli: tn, falseCli: fn,
  accuracy: +acc.toFixed(3),
  appPrecision: +prec.toFixed(3),
  appRecall: +recAll.toFixed(3),
  wrongExamples: wrong.slice(0, 10),
}, null, 2));