'use strict';

// Extracted from main.js so both the app and test/dom-harness.js can
// register the same privileged autome://app protocol without duplicating
// the path-traversal guard.
const path = require('node:path');
const fs = require('node:fs');
const { protocol } = require('electron');

const RENDERER_DIR = path.join(__dirname, '..', 'renderer');

function registerSchemeAsPrivileged() {
  protocol.registerSchemesAsPrivileged([
    {
      scheme: 'autome',
      privileges: {
        standard: true,
        secure: true,
        supportFetchAPI: true,
        corsEnabled: false,
        stream: false,
      },
    },
  ]);
}

// Serves only files inside RENDERER_DIR for the autome://app/* origin —
// plan §9.4: "只加载包内 autome://app 自定义安全协议，不使用远程 HTML、CDN
// 或 file://". Path-traverses defensively even though the renderer is our
// own bundled content, since this handler is reachable from anything the
// window ever navigates to.
function registerAppProtocol() {
  protocol.handle('autome', (request) => {
    const url = new URL(request.url);
    if (url.host !== 'app') {
      return new Response('not found', { status: 404 });
    }
    const requestedPath = url.pathname === '/' ? '/index.html' : url.pathname;
    const resolved = path.normalize(path.join(RENDERER_DIR, requestedPath));
    if (!resolved.startsWith(RENDERER_DIR + path.sep) && resolved !== RENDERER_DIR) {
      return new Response('forbidden', { status: 403 });
    }
    if (!fs.existsSync(resolved) || !fs.statSync(resolved).isFile()) {
      return new Response('not found', { status: 404 });
    }
    return new Response(fs.readFileSync(resolved));
  });
}

module.exports = { RENDERER_DIR, registerSchemeAsPrivileged, registerAppProtocol };
