'use strict';

// Electron Main entrypoint. Plan D3: Electron is UI only, owns no business
// state — Main's job here is (a) enforce the §9.4 security baseline for
// every window and the privileged `autome://` scheme, and (b) manage the
// automed Rust Core sidecar's process lifecycle per §9.5.
//
// Deliberately out of scope for this increment (tracked, not forgotten):
// core-manifest.json signature/binary-digest verification, single-instance
// OS lock + parent_instance_nonce binding, PrepareShutdown/SafePark-gated
// quit, tray/hide-on-close, ASAR integrity + Electron fuses, and the real
// §9 navigation UI. Those all depend on packaging and business-state IPC
// surfaces that do not exist yet; building them now would mean guessing at
// unspecified details rather than following the plan.

const path = require('node:path');
const fs = require('node:fs');
const { app, BrowserWindow, protocol, session } = require('electron');
const { AutomedSidecar } = require('./src/sidecar');

const RENDERER_DIR = path.join(__dirname, 'renderer');

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

function denyAllPermissionRequests() {
  session.defaultSession.setPermissionRequestHandler((_wc, _permission, callback) => {
    callback(false);
  });
}

function createMainWindow() {
  const win = new BrowserWindow({
    width: 1100,
    height: 720,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      nodeIntegration: false,
      contextIsolation: true,
      sandbox: true,
      webSecurity: true,
      webviewTag: false,
    },
  });

  // §9.4: "拒绝导航、新窗口和默认权限请求".
  win.webContents.on('will-navigate', (event) => event.preventDefault());
  win.webContents.setWindowOpenHandler(() => ({ action: 'deny' }));

  win.loadURL('autome://app/index.html');
  return win;
}

let sidecar = null;

// Proves the Main <-> Core round trip actually works at startup, the same
// contract crates/automed/tests/stdio_loop.rs and
// apps/desktop/test/sidecar.test.js verify in isolation — not yet wired to
// any renderer-visible state, since no business IPC surface exists yet.
function startSidecarAndHandshake(dbPath) {
  return new Promise((resolve, reject) => {
    const s = new AutomedSidecar({
      dbPath,
      onEvent: (event) => {
        console.log('[automed] handshake event:', event.event_type, event.aggregate_id);
        resolve(s);
      },
      onStderrLine: (line) => console.error('[automed]', line),
      onExit: (code, signal) => console.log('[automed] exited', { code, signal }),
    }).start();
    s.send({
      request_id: 'desktop-startup',
      command_id: 'desktop-startup-handshake',
      expected_revision: null,
      protocol_version: 1,
      method: 'run.advance_nominal',
      params: { aggregate_id: 'desktop-startup-handshake' },
    });
    setTimeout(() => reject(new Error('automed handshake timed out')), 5000);
  });
}

app.whenReady().then(async () => {
  registerAppProtocol();
  denyAllPermissionRequests();

  const dbPath = process.env.AUTOMED_DB_PATH || path.join(app.getPath('userData'), 'automed.sqlite3');
  try {
    sidecar = await startSidecarAndHandshake(dbPath);
  } catch (err) {
    console.error('[automed] startup handshake failed:', err);
  }

  createMainWindow();

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) createMainWindow();
  });
});

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit();
});

app.on('before-quit', async (event) => {
  if (sidecar) {
    event.preventDefault();
    const toStop = sidecar;
    sidecar = null;
    await toStop.stop();
    app.quit();
  }
});
