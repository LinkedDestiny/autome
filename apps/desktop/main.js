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
const { app, BrowserWindow, ipcMain, session, dialog } = require('electron');
const { AutomedSidecar } = require('./src/sidecar');
const appProtocol = require('./src/app-protocol');
const ipcGate = require('./src/ipc-gate');
const writeGate = require('./src/write-gate');

// §8.2 phase②: write-gate.js's snake_case kind names translated to Core's
// PascalCase `ProjectKind` variant strings — only `project.pick_target`
// needs this; `project.create_from_target` reads kind off the
// already-persisted target row, never from the request.
const PROJECT_KIND_TO_CORE = Object.freeze({
  new_product: 'NewProduct',
  existing_repository: 'ExistingRepository',
});

appProtocol.registerSchemeAsPrivileged();

function denyAllPermissionRequests() {
  session.defaultSession.setPermissionRequestHandler((_wc, _permission, callback) => {
    callback(false);
  });
}

let mainWindow = null;

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
  win.on('closed', () => {
    if (mainWindow === win) mainWindow = null;
  });

  win.loadURL('autome://app/index.html');
  mainWindow = win;
  return win;
}

let sidecar = null;
let requestCounter = 0;
let commandCounter = 0;
// Split into two independent generators (previously one counter served
// both, called twice per command, so request_id and command_id never
// matched) — §3.2 needs command_id to stay stable across a retry, which
// only holds if it isn't also incrementing every time a request_id is
// drawn.
function nextRequestId() {
  requestCounter += 1;
  return `desktop-req-${requestCounter}`;
}
function nextCommandId() {
  commandCounter += 1;
  return `desktop-cmd-${commandCounter}`;
}

// §9.4: "每个 IPC handler 校验 sender origin、webContents、schema 和 payload
// 上限". Registered once at startup; every call still re-checks the sender
// per-invocation since the handler outlives any single window.
function registerReadChannel() {
  ipcMain.handle('autome:read', async (event, request) => {
    if (!ipcGate.isTrustedSenderUrl(event.senderFrame && event.senderFrame.url)) {
      throw new Error('rejected: sender is not the packaged autome://app origin');
    }
    if (!mainWindow || event.sender !== mainWindow.webContents) {
      throw new Error('rejected: sender is not the main window');
    }
    const validated = ipcGate.validateReadRequest(request);
    if (!validated.ok) {
      throw new Error(`rejected: ${validated.message}`);
    }
    if (!sidecar) {
      throw new Error('automed sidecar is not running');
    }
    const reply = await sidecar.request({
      request_id: nextRequestId(),
      command_id: nextCommandId(),
      expected_revision: null,
      protocol_version: 1,
      method: validated.method,
      params: validated.params,
    });
    if (reply.outcome.status === 'error') {
      throw new Error(`${reply.outcome.code}: ${reply.outcome.message}`);
    }
    return reply.outcome.payload;
  });
}

// §8.2's write path. Same three-gate pattern as registerReadChannel above
// (sender origin → webContents identity → gate validation), backed by a
// wholly separate allowlist (write-gate.js) that never merges with or
// relaxes ipc-gate.js's read-side one. `project.pick_target` is handled
// entirely in Main: it never forwards to Core as-is — the directory comes
// from Main's own `dialog.showOpenDialog`, and only the resulting
// `{ target_id, summary }` (Core's `project.register_target` response) is
// handed back to the Renderer. `project.create_from_target` forwards
// straight through: Core derives the `ProjectLocator` itself from the
// already-persisted target row, never from anything in this request.
function registerWriteChannel() {
  ipcMain.handle('autome:write', async (event, request) => {
    if (!ipcGate.isTrustedSenderUrl(event.senderFrame && event.senderFrame.url)) {
      throw new Error('rejected: sender is not the packaged autome://app origin');
    }
    if (!mainWindow || event.sender !== mainWindow.webContents) {
      throw new Error('rejected: sender is not the main window');
    }
    const validated = writeGate.validateWriteRequest(request);
    if (!validated.ok) {
      throw new Error(`rejected: ${validated.message}`);
    }
    if (!sidecar) {
      throw new Error('automed sidecar is not running');
    }

    if (validated.op === 'project.pick_target') {
      const result = await dialog.showOpenDialog(mainWindow, { properties: ['openDirectory'] });
      if (result.canceled || result.filePaths.length === 0) {
        return { cancelled: true };
      }
      const reply = await sidecar.request({
        request_id: nextRequestId(),
        command_id: nextCommandId(),
        expected_revision: null,
        protocol_version: 1,
        method: 'project.register_target',
        params: {
          kind: PROJECT_KIND_TO_CORE[validated.params.kind],
          path: result.filePaths[0],
        },
      });
      if (reply.outcome.status === 'error') {
        throw new Error(`${reply.outcome.code}: ${reply.outcome.message}`);
      }
      return reply.outcome.payload;
    }

    // Only `project.create_from_target` remains — write-gate.js's
    // allowlist has exactly two ops.
    const reply = await sidecar.request({
      request_id: nextRequestId(),
      command_id: nextCommandId(),
      expected_revision: null,
      protocol_version: 1,
      method: validated.op,
      params: validated.params,
    });
    if (reply.outcome.status === 'error') {
      throw new Error(`${reply.outcome.code}: ${reply.outcome.message}`);
    }
    return reply.outcome.payload;
  });
}

function startSidecar(dbPath) {
  sidecar = new AutomedSidecar({
    dbPath,
    onEvent: () => {},
    onStderrLine: (line) => console.error('[automed]', line),
    onExit: (code, signal) => {
      console.log('[automed] exited', { code, signal });
      sidecar = null;
    },
  }).start();
  return sidecar;
}

// Proves the Main <-> Core round trip actually works at startup, the same
// contract crates/automed/tests/stdio_loop.rs and
// apps/desktop/test/sidecar.test.js verify in isolation. Uses the real
// `queue.get` read method rather than a fabricated write command — the
// prior handshake used `run.advance_nominal`, which appended a real
// `desktop-startup-handshake` Run event to the user's database on every
// single launch. A read call proves the same round trip without writing
// anything.
async function verifySidecarHandshake(s) {
  const reply = await s.request(
    {
      request_id: 'desktop-startup',
      command_id: 'desktop-startup-handshake',
      expected_revision: null,
      protocol_version: 1,
      method: 'queue.get',
      params: {},
    },
    { timeoutMs: 5000 }
  );
  if (reply.outcome.status === 'error') {
    throw new Error(`${reply.outcome.code}: ${reply.outcome.message}`);
  }
}

app.whenReady().then(async () => {
  appProtocol.registerAppProtocol();
  denyAllPermissionRequests();
  registerReadChannel();
  registerWriteChannel();

  const dbPath = process.env.AUTOMED_DB_PATH || path.join(app.getPath('userData'), 'automed.sqlite3');
  const s = startSidecar(dbPath);
  try {
    await verifySidecarHandshake(s);
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
