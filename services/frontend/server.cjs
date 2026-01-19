const http = require('http');
const fs = require('fs');
const path = require('path');

const PORT = 5173;
const API_PORT = 8080;
const DIST_DIR = path.join(__dirname, 'dist');

const mimeTypes = {
  '.html': 'text/html',
  '.js': 'application/javascript',
  '.css': 'text/css',
  '.json': 'application/json',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.svg': 'image/svg+xml',
  '.ico': 'image/x-icon',
};

const server = http.createServer((req, res) => {
  const url = req.url;

  if (url.startsWith('/v1/') || url.startsWith('/auth/google')) {
    const options = {
      hostname: 'localhost',
      port: API_PORT,
      path: url,
      method: req.method,
      headers: {
        ...req.headers,
        host: `localhost:${API_PORT}`,
      },
    };

    const proxyReq = http.request(options, (proxyRes) => {
      res.writeHead(proxyRes.statusCode, proxyRes.headers);
      proxyRes.pipe(res, { end: true });
    });

    proxyReq.on('error', (err) => {
      console.error('Proxy error:', err);
      res.writeHead(502, { 'Content-Type': 'text/plain' });
      res.end('Bad gateway');
    });

    req.pipe(proxyReq, { end: true });
    return;
  }

  let urlPath = url.split('?')[0];
  let normalizedPath = path.normalize(urlPath);

  let filePath;
  if (normalizedPath === '/' || normalizedPath === '.') {
    filePath = path.join(DIST_DIR, 'index.html');
  } else {
    filePath = path.join(DIST_DIR, normalizedPath);
  }

  const resolvedPath = path.resolve(filePath);
  const distResolvedPath = path.resolve(DIST_DIR);

  if (!resolvedPath.startsWith(distResolvedPath + path.sep) && resolvedPath !== distResolvedPath) {
    res.writeHead(400, { 'Content-Type': 'text/plain' });
    res.end('Bad Request: Invalid path');
    return;
  }

  const ext = path.extname(filePath);
  const contentType = mimeTypes[ext] || 'application/octet-stream';
  
  fs.readFile(filePath, (err, content) => {
    if (err) {
      if (err.code === 'ENOENT') {
        fs.readFile(path.join(DIST_DIR, 'index.html'), (err, content) => {
          if (err) {
            res.writeHead(500);
            res.end('Server error');
          } else {
            res.writeHead(200, { 'Content-Type': 'text/html' });
            res.end(content);
          }
        });
      } else {
        res.writeHead(500);
        res.end('Server error');
      }
    } else {
      res.writeHead(200, { 'Content-Type': contentType });
      res.end(content);
    }
  });
});

server.listen(PORT, '0.0.0.0', () => {
  console.log(`Frontend server running on http://localhost:${PORT}`);
  console.log(`API proxy: http://localhost:${PORT}/v1/* -> http://localhost:${API_PORT}/v1/*`);
  console.log(`OAuth proxy: http://localhost:${PORT}/auth/google/* -> http://localhost:${API_PORT}/auth/google/*`);
});
