#!/usr/bin/env node
// Assembles qa/sample-v1.md from entries.json + a labeller score file
// (lines "id=<n> <VERDICT> <reason>").
// Usage: node qa/assemble.js <labeller-output-file>
const fs = require('fs');
const path = require('path');

const qa = path.join(__dirname);
const entries = JSON.parse(fs.readFileSync(path.join(qa, 'entries.json'), 'utf8'));
const scoreFile = process.argv[2];
const lines = fs.readFileSync(scoreFile, 'utf8').split('\n');
const scores = {};
for (const l of lines) {
  const m = l.match(/^id=(\d+)\s+([A-Z-]+)\s*(.*)$/);
  if (m) scores[Number(m[1])] = { v: m[2], why: (m[3] || '').trim() };
}

const rows = entries.map((e) => {
  const s = scores[e.id] || { v: 'UNSCORED', why: '' };
  return {
    ...e, ...s,
    match: e.found ? e.attr : '— no match —',
    tier: e.found ? 'exact' : 'absent',
  };
});

const cnt = rows.reduce((a, r) => { a[r.v] = (a[r.v] || 0) + 1; return a; }, {});

const md = `# QA Sample v1 — app-to-package join review

This sample checks whether the catalogue's automatic join between a consumer-facing
app name and a Nixpkgs package is correct and appropriate for a normal user. It was
generated from the Phase 0 export (see [../README.md](../README.md)); 116 entries.

## What I need from you (about 20–30 minutes, skimmable)

- Read the table below — each row is one app name a user might search for.
- Column "My score" is an independent labeller's verdict (a subagent role-playing a
  non-technical consumer).
- **If you agree with "My score", leave the row untouched (skip = agree).** If you
  disagree, overwrite the **"Your verdict"** cell with one of the tokens below.
- Verdict tokens:
  - \`GOOD\` — right app, belongs in a consumer store.
  - \`WRONG\` — same name, wrong package.
  - \`NOT-APP\` — it's a library/CLI/runtime/daemon; must not be shown as an app.
  - \`UNSURE\` — can't tell.
  - \`CORRECT-ABSENT\` — no match found and that's fine (made-up, wrong-platform).
  - \`MISSING\` — a real consumer app a normal user would expect; its absence is a
    coverage gap.
- When done, commit the edited file or reply with just the ids you disagree with.
  I compute the precision/recall numbers from your version.

## What this scores

- "My score = GOOD" rows measure recall (are real apps found?).
- "My score = NOT-APP" rows measure precision (do we wrongly present libs/CLIs?).
- "MISSING" rows flag real coverage gaps (Kdenlive, Spectacle, Cursor, Dropbox).

---

## Table

| # | App (user searches) | Proposed match | Tier | Short description | My score | Your verdict |
|---|---|---|---|---|---|---|
${rows.map((r) => `| ${r.id} | ${r.name} | \`${r.match}\` | ${r.tier} | ${(r.desc || '').slice(0, 60)} | ${r.v}${r.why ? ' (' + r.why + ')' : ''} |  |`).join('\n')}
`;

fs.writeFileSync(path.join(qa, 'sample-v1.md'), md);
console.log('wrote qa/sample-v1.md; verdict counts:', JSON.stringify(cnt));