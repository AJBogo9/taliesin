// Minimal static HTTP server over a built unit's output dir. We serve over
// http:// (not file://) because {js} cells' relative import() is blocked on
// file:// origins and because localStorage theme-seeding needs a stable origin.

import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';

const TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.gif': 'image/gif',
  '.webp': 'image/webp',
  '.avif': 'image/avif',
  '.woff': 'font/woff',
  '.woff2': 'font/woff2',
  '.ttf': 'font/ttf',
  '.map': 'application/json; charset=utf-8',
  '.mp4': 'video/mp4',
  '.webm': 'video/webm',
  '.ico': 'image/x-icon',
};

// Start a server on an ephemeral port. Returns { url, port, close() }.
export function serveDir(root) {
  const server = http.createServer((req, res) => {
    let urlPath;
    try {
      urlPath = decodeURIComponent(new URL(req.url, 'http://x').pathname);
    } catch {
      res.writeHead(400).end();
      return;
    }
    // Chrome auto-probes /favicon.ico when a page declares no icon link.
    // Taliesin inlines its favicon as a data URI, so this probe is pure noise
    // (a harness artifact, not a page bug). Answer 204 so it never shows up as
    // a spurious 404 in console/network logs.
    if (urlPath === '/favicon.ico') {
      res.writeHead(204).end();
      return;
    }
    let filePath = path.join(root, urlPath);
    // Directory -> index.html
    try {
      if (fs.statSync(filePath).isDirectory()) {
        filePath = path.join(filePath, 'index.html');
      }
    } catch {
      /* fall through to read attempt */
    }
    // Prevent path escape.
    if (!path.resolve(filePath).startsWith(path.resolve(root))) {
      res.writeHead(403).end();
      return;
    }
    fs.readFile(filePath, (err, data) => {
      if (err) {
        res.writeHead(404, { 'content-type': 'text/plain' }).end('404');
        return;
      }
      const type = TYPES[path.extname(filePath).toLowerCase()] ||
        'application/octet-stream';
      res.writeHead(200, { 'content-type': type }).end(data);
    });
  });

  return new Promise((resolve) => {
    server.listen(0, '127.0.0.1', () => {
      const { port } = server.address();
      resolve({
        url: `http://127.0.0.1:${port}`,
        port,
        close: () => new Promise((r) => server.close(r)),
      });
    });
  });
}
