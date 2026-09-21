'use strict';

// Electron Main. Plan D3: Electron is UI only and owns no business state.
// Main's job is three things and nothing else:
//
//   1. Enforce the security baseline for every window and for the privileged
//      `autome://` scheme.
//   2. Manage the `automed` sidecar's lifecycle and forward its events.
//   3. Own the few capabilities the renderer must never have: the directory
//      picker, opening a path, and running a command in a terminal. Each is
//      keyed by identifier, and Main resolves the actual path or command from
//      the core's own answer — so a compromised renderer can ask to open "the
//      log of session S", never "/Users/me/.ssh/id_rsa".
//
// Deliberately out of scope, tracked rather than forgotten: core-manifest
// signature verification, the single-instance lock, and Electron fuses.
// Packaging, the hardened runtime and signing are in scripts/package.sh and
// the `build` block of package.json.

const path = require('node:path');
const { app, BrowserWindow, ipcMain, session, dialog, shell } = require('electron');
const { AutomedSidecar } = require('./src/sidecar');
const { nextRestart } = require('./src/core-restart');
const { corePathEnv } = require('./src/login-path');
const appProtocol = require('./src/app-protocol');
const ipcGate = require('./src/ipc-gate');
const writeGate = require('./src/write-gate');

appProtocol.registerSchemeAsPrivileged();

let mainWindow = null;
let sidecar = null;
let requestCounter = 0;
let commandCounter = 0;
// Where the core's database lives, so the banner's restart button can start a
// core with the same one Main started the first one with.
let coreDbPath = null;
// Consecutive starts that did not survive; see src/core-restart.js.
let coreQuickFailures = 0;
// The PATH overlay for the core, resolved once from the login shell (see
// src/login-path.js) rather than per start: a restart loop must not spawn a
// shell per attempt, and the user's PATH does not change between them.
let coreEnv;

// Two independent generators: §14 needs `command_id` to stay stable across a
// retry, which only holds if it is not also incremented every time a
// `request_id` is drawn.
function nextRequestId() {
  requestCounter += 1;
  return `desktop-req-${requestCounter}`;
}
function nextCommandId() {
  commandCounter += 1;
  return `desktop-cmd-${commandCounter}`;
}

function denyAllPermissionRequests() {
  session.defaultSession.setPermissionRequestHandler((_wc, _permission, callback) => {
    callback(false);
  });
  session.defaultSession.setPermissionCheckHandler(() => false);
}

function createMainWindow() {
  const win = new BrowserWindow({
    width: 1512,
    height: 944,
    minWidth: 1100,
    minHeight: 700,
    titleBarStyle: 'hiddenInset',
    backgroundColor: '#f8f8f0',
    // macOS swallows the click that activates a background window. Autome's
    // windows lose focus constantly — every session opens a terminal that
    // activates itself — so without this the user's first click on any control
    // does nothing and they have to click it again. Every destructive action
    // here is behind a modal confirmation, so acting on the activating click
    // cannot merge or cancel anything by accident.
    acceptFirstMouse: true,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      nodeIntegration: false,
      contextIsolation: true,
      sandbox: true,
      webSecurity: true,
      webviewTag: false,
      spellcheck: false,
    },
  });

  win.webContents.on('will-navigate', (event) => event.preventDefault());
  win.webContents.setWindowOpenHandler(() => ({ action: 'deny' }));
  win.on('closed', () => {
    if (mainWindow === win) mainWindow = null;
  });

  win.loadURL('autome://app/index.html');
  mainWindow = win;
  return win;
}

// Every handler re-checks the sender per invocation, since it outlives any
// single window.
function assertTrustedSender(event) {
  const url = event.senderFrame && event.senderFrame.url;
  if (!ipcGate.isTrustedSenderUrl(url)) {
    throw new Error('rejected: sender is not the packaged autome://app origin');
  }
  if (!mainWindow || event.sender !== mainWindow.webContents) {
    throw new Error('rejected: sender is not the main window');
  }
}

async function callCore(method, params) {
  if (!sidecar) throw new Error('内核未运行');
  const reply = await sidecar.request({
    request_id: nextRequestId(),
    command_id: nextCommandId(),
    expected_revision: null,
    protocol_version: 1,
    method,
    params,
  });
  if (reply.outcome.status === 'error') {
    const err = new Error(reply.outcome.message);
    err.code = reply.outcome.code;
    throw err;
  }
  return reply.outcome.payload;
}

function registerReadChannel() {
  ipcMain.handle('autome:read', async (event, request) => {
    assertTrustedSender(event);
    const validated = ipcGate.validateReadRequest(request);
    if (!validated.ok) throw new Error(`rejected: ${validated.message}`);
    return callCore(validated.method, validated.params);
  });
}

function registerWriteChannel() {
  ipcMain.handle('autome:write', async (event, request) => {
    assertTrustedSender(event);
    const validated = writeGate.validateWriteRequest(request);
    if (!validated.ok) throw new Error(`rejected: ${validated.message}`);
    const { op, params } = validated;

    // These are handled entirely in Main: the first three are capabilities
    // the renderer must not have, and `core.restart` is the one op that
    // cannot reach the core — it exists precisely because there is none.
    if (op === 'project.pick') return pickProject();
    if (op === 'open.path') return openPath(params);
    if (op === 'open.terminal') return openTerminal(params);
    if (op === 'core.restart') return restartCore();
    if (op === 'env.install') return runInstall(params);
    if (op === 'env.login') return runLogin(params);

    return callCore(op, params);
  });
}

// Main owns the dialog, so the path never originates in the renderer.
async function pickProject() {
  const result = await dialog.showOpenDialog(mainWindow, {
    properties: ['openDirectory', 'createDirectory'],
    buttonLabel: '选择',
    title: '选择项目目录',
  });
  if (result.canceled || result.filePaths.length === 0) {
    return { cancelled: true };
  }
  return callCore('project.add', { path: result.filePaths[0] });
}

// Resolves an identifier to a path by asking the core, then reveals it. The
// renderer names *what*, never *where*.
async function openPath(params) {
  let target = null;
  switch (params.kind) {
    case 'project': {
      const payload = await callCore('project.get', { project_id: params.project_id });
      target = payload.project.path;
      break;
    }
    case 'worktree': {
      const payload = await callCore('task.get', { task_id: params.task_id });
      target = payload.worktree;
      break;
    }
    case 'document': {
      const payload = await callCore('task.get', { task_id: params.task_id });
      const doc = (payload.documents || []).find((d) => d.name === params.name);
      if (!doc) throw new Error(`找不到文档 ${params.name}`);
      target = doc.absolute;
      break;
    }
    case 'log': {
      const payload = await callCore('session.log', { session_id: params.session_id });
      target = payload.path;
      break;
    }
    case 'skill': {
      const payload = await callCore('skills.list', { project_id: params.project_id });
      const skill = (payload.skills || []).find((s) => s.name === params.name);
      if (!skill || !skill.sources.length) throw new Error(`找不到技能 ${params.name}`);
      target = skill.sources[0].path;
      break;
    }
    default:
      throw new Error(`unknown open kind: ${params.kind}`);
  }
  if (typeof target !== 'string' || target.length === 0) {
    throw new Error('内核没有返回可打开的路径');
  }
  // `openPath` reveals the item; it never executes it.
  const error = await shell.openPath(target);
  if (error) throw new Error(error);
  return { opened: target };
}

// The two commands a user can have run for them, both in a visible terminal
// so they can see exactly what happened (requirement E-02, E-03).
async function runInTerminal(command, title) {
  const { spawn } = require('node:child_process');
  const escaped = command.replace(/\\/g, '\\\\').replace(/"/g, '\\"');
  const useITerm = require('node:fs').existsSync('/Applications/iTerm.app');
  const script = useITerm
    ? `tell application "iTerm"
         activate
         if (count of windows) = 0 then
           set w to (create window with default profile)
           set s to current session of w
         else
           tell current window
             set t to (create tab with default profile)
             set s to current session of t
           end tell
         end if
         tell s
           set name to "${title.replace(/"/g, '')}"
           write text "${escaped}"
         end tell
       end tell`
    : `tell application "Terminal"
         activate
         do script "${escaped}"
       end tell`;
  await new Promise((resolve, reject) => {
    const child = spawn('osascript', ['-e', script], { stdio: 'ignore' });
    child.on('error', reject);
    child.on('exit', (code) =>
      code === 0 ? resolve() : reject(new Error(`osascript 退出码 ${code}`))
    );
  });
  return { terminal: useITerm ? 'iTerm2' : 'Terminal' };
}

/**
 * The "open a terminal for this task" button.
 *
 * It used to promise "在 iTerm 中查看", meaning the tab the session was running
 * in — and it was never implemented, so pressing it threw
 * `openTerminal is not defined`. Sessions are headless now, so there is no tab
 * to switch to and the original promise is gone. What is useful instead: a
 * terminal sitting in the task's worktree, following the live session log if
 * one is running. This is the one terminal the user asked for by name, so it
 * is deliberately visible.
 */
async function openTerminal(params) {
  const payload = await callCore('task.get', { task_id: params.task_id });
  const worktree = payload.worktree;
  if (typeof worktree !== 'string' || worktree.length === 0) {
    throw new Error('这个任务还没有 worktree');
  }

  // Follow the running session if there is one; otherwise show the last one
  // that ran. The first version only tailed a *running* session, so opening a
  // terminal on a task that had stopped gave a bare prompt and no sign that
  // anything had ever happened — which, with sessions headless, is the only
  // place the user would have looked.
  // `task.get` returns sessions newest first (`ORDER BY started_at DESC` in
  // store.rs), so the most recent one is at index 0, not at the end.
  const sessions = payload.sessions || [];
  const session = sessions.find((s) => s.running) || sessions[0];

  const parts = [`cd ${shellQuote(worktree)}`];
  if (session) {
    const log = await callCore('session.log', { session_id: session.id });
    const path = log && log.path;
    if (path) {
      const label = `${session.label || '会话'} #${session.round}`;
      parts.push(`echo ${shellQuote(`== ${label} · ${path} ==`)}`);
      parts.push(
        session.running
          ? `tail -f -n +1 ${shellQuote(path)}`
          : `tail -n ${TERMINAL_LOG_LINES} ${shellQuote(path)}`
      );
    }
  } else {
    parts.push(`echo ${shellQuote('这个任务还没有跑过会话。')}`);
  }

  const command = parts.join(' && ');
  const result = await runInTerminal(command, `autome · ${params.task_id}`);
  return { ...result, command, worktree, session: session ? session.id : null };
}

// Enough to see how a round ended without flooding the scrollback. The whole
// file is named on the line above it, and the task panel shows it in full.
const TERMINAL_LOG_LINES = 500;

/** POSIX single-quoting, the same rule as the core's `sh_quote`. */
function shellQuote(value) {
  return `'${String(value).replace(/'/g, `'\''`)}'`;
}

async function runInstall(params) {
  const payload = await callCore('env.install_recipe', { component: params.component });
  if (!payload.prerequisite_ok) {
    throw new Error(`请先安装 ${payload.recipe.prerequisite}`);
  }
  const result = await runInTerminal(payload.recipe.command, `autome · 安装 ${params.component}`);
  return { ...result, command: payload.recipe.command };
}

async function runLogin(params) {
  const payload = await callCore('env.install_recipe', { component: params.component });
  if (!payload.login_command) {
    throw new Error(`${params.component} 没有登录命令`);
  }
  const result = await runInTerminal(payload.login_command, `autome · 登录 ${params.component}`);
  return { ...result, command: payload.login_command };
}

function broadcast(channel, payload) {
  if (mainWindow && !mainWindow.isDestroyed()) {
    mainWindow.webContents.send(channel, payload);
  }
}

function startSidecar(dbPath) {
  const startedAt = Date.now();
  // A GUI launch inherits launchd's PATH, in which `claude`, `codex`, `npm`
  // and `brew` do not exist — the core would then probe a fully-provisioned
  // machine and report them 未安装. Resolved lazily so the log line lands once,
  // next to the start it applies to.
  if (coreEnv === undefined) {
    coreEnv = corePathEnv();
    if (coreEnv) console.log('[automed] core PATH from login shell:', coreEnv.PATH);
  }
  let instance = null;
  instance = new AutomedSidecar({
    dbPath,
    env: coreEnv || undefined,
    onEvent: (event) => broadcast('autome:event', event),
    onStderrLine: (line) => console.error('[automed]', line),
    onExit: (code, signal) => {
      console.log('[automed] exited', { code, signal });
      sidecar = null;
      // The core's own last words. A fatal startup failure — a database it
      // will not open, a binary that is not there — is reported on stderr and
      // nowhere else, so without this the window can only say that the core
      // is unreachable, never why.
      const reason = instance.failureReason();
      const decision = nextRestart({
        ranForMs: Date.now() - startedAt,
        quickFailures: coreQuickFailures,
      });
      coreQuickFailures = decision.quickFailures;

      if (!decision.restart) {
        console.error(
          `[automed] gave up after ${decision.quickFailures} failed starts:`,
          reason || `code=${code} signal=${signal}`
        );
        broadcast('autome:core-status', {
          connected: false,
          fatal: true,
          reason,
          attempts: decision.quickFailures,
          code,
          signal,
        });
        return;
      }

      broadcast('autome:core-status', { connected: false, fatal: false, reason, code, signal });
      // The core is the only place business state lives, so a dead core means
      // a dead app. Restart it rather than leaving the window showing a
      // frozen projection.
      setTimeout(() => {
        if (!sidecar && !app.isQuiting) {
          startSidecar(dbPath);
          broadcast('autome:core-status', { connected: true, restarted: true });
        }
      }, decision.delayMs);
    },
  }).start();
  sidecar = instance;
  return sidecar;
}

/**
 * The banner's button once Main has stopped restarting the core by itself.
 *
 * Deliberately verified rather than optimistic: it starts a core and then
 * makes one real call, so a user who has just moved the offending database
 * aside is told it worked, and a user who has not is handed the same sentence
 * again instead of a window that claims to be connected for one second.
 */
async function restartCore() {
  if (sidecar) return { already_running: true };
  coreQuickFailures = 0;
  const started = startSidecar(coreDbPath);
  try {
    await callCore('scheduler.tick', {});
  } catch (err) {
    throw new Error(started.failureReason() || err.message);
  }
  broadcast('autome:core-status', { connected: true, restarted: true });
  return { restarted: true };
}

// The core does not run its own timer: the polling interval is a UI decision,
// and making it one keeps a constant out of the core that the user might
// reasonably want to change.
const TICK_INTERVAL_MS = 3000;
let tickTimer = null;
// The environment is probed on a background thread in the core, so its result
// arrives between requests rather than in reply to one. The tick reports a
// generation counter; a change means a probe landed and the screens showing
// environment state should re-read.
let lastEnvGeneration = null;

function startTicking() {
  if (tickTimer) return;
  tickTimer = setInterval(async () => {
    if (!sidecar) return;
    try {
      const report = await callCore('scheduler.tick', {});
      const envMoved =
        report.env_generation !== undefined && report.env_generation !== lastEnvGeneration;
      if (envMoved) lastEnvGeneration = report.env_generation;
      if (
        report.sessions_reaped.length ||
        report.tasks_advanced.length ||
        report.tasks_started.length ||
        envMoved
      ) {
        broadcast('autome:event', { event_type: 'tick', payload: report });
      }
      for (const error of report.errors) console.error('[scheduler]', error);
    } catch (err) {
      console.error('[scheduler] tick failed:', err.message);
    }
  }, TICK_INTERVAL_MS);
}

app.whenReady().then(async () => {
  appProtocol.registerAppProtocol();
  denyAllPermissionRequests();
  registerReadChannel();
  registerWriteChannel();

  // `null` means "wherever the core keeps it" — `AUTOME_HOME/state`, which is
  // where the config and the protocol repository already live. The shell has
  // no business choosing a home for state it does not own, and the one it
  // used to choose (Electron's `userData`) made the app's installation a
  // second, separate one from the CLI's.
  coreDbPath = process.env.AUTOMED_DB_PATH || null;
  startSidecar(coreDbPath);
  createMainWindow();
  startTicking();

  // Reconcile after a restart before the window asks for anything: a session
  // that finished while the app was closed is consumed here (design §13).
  try {
    await callCore('scheduler.tick', {});
  } catch (err) {
    console.error('[automed] startup reconciliation failed:', err.message);
  }

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) createMainWindow();
  });

  // Environment state can change while the app is in the background — a CLI
  // installed, a login expired.
  app.on('browser-window-focus', () => {
    callCore('env.detect', {}).catch(() => {});
  });
});

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit();
});

app.on('before-quit', async (event) => {
  if (sidecar) {
    event.preventDefault();
    app.isQuiting = true;
    if (tickTimer) clearInterval(tickTimer);
    const toStop = sidecar;
    sidecar = null;
    await toStop.stop();
    app.quit();
  }
});
