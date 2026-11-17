#!/usr/bin/env node
// Tiny static server for the Phase 0 catalogue viewer. No deps.
// Usage: node serve.js [port]   (default 8137)  ->  http://localhost:8137/view.html
const http = require('http');
const fs = require('fs');
const path = require('path');

const root = __dirname;
const port = Number(process.argv[2]) || 8137;
const TYPES = {
  '.html': 'text/html; charset=utf-8', '.json': 'application/json',
  '.js': 'text/javascript; charset=utf-8', '.css': 'text/css',
  '.svg': 'image/svg+xml', '.png': 'image/png', '.ico': 'image/x-icon',
};

http.createServer((req, res) => {
  let p = decodeURIComponent(req.url.split('?')[0]);
  if (p === '/') p = '/index.html';
  const file = path.normalize(path.join(root, p));
  if (!file.startsWith(root) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) {
    res.writeHead(404, { 'Content-Type': 'text/plain' }); res.end('not found'); return;
  }
  const type = TYPES[path.extname(file).toLowerCase()] || 'application/octet-stream';
  res.writeHead(200, { 'Content-Type': type });
  fs.createReadStream(file).pipe(res);
}).listen(port, () => {
  console.log(`Phase 0 catalogue viewer:  http://localhost:${port}/view.html`);
});