'use strict';

// README screenshots, taken from the real renderer.
//
// Not a mockup and not a photo of a running install: this boots the actual
// Electron shell, answers its reads with the test fixtures, and captures what
// the router paints. So the pictures in the README are the same DOM the
// suite asserts against — a screen that regresses produces a screenshot that
// regresses, and neither can drift from the shipped app without the other.
//
// Fixtures, not a real machine: `test/fixtures.js` is invented data (岛屿商店,
// 海风 CLI, T-015…), which is the point. A screenshot of this developer's
// actual Autome would put their repositories, task text and skill inventory
// into a public README.
//
// Reads are served here in Main rather than stubbed in the page, because the
// page cannot be stubbed: `window.autome` comes from the preload over
// contextBridge and is read-only. Answering the real IPC channel means the
// screenshot goes through the real `load()` → `render()` → chrome path, with
// the real connection state — which is also why no offline banner appears.
//
//   npm run shots            all of them
//   npm run shots -- dash    just one
//
// Runs under `electron`, not `node`.

const fs = require('node:fs');
const path = require('node:path');
const { app, BrowserWindow, ipcMain } = require('electron');
const appProtocol = require('../src/app-protocol');
const { FIXTURES, PROTOCOL_SCREEN, TASK_AT_MERGE } = require('../test/fixtures');

appProtocol.registerSchemeAsPrivileged();
// The capture runs headless; without this the GPU process is what decides
// whether there is an image at all, and in a sandbox it often decides no.
app.disableHardwareAcceleration();

/** The design's reference viewport (requirement U-11), as in dom-harness.js. */
const VIEWPORT = { width: 1512, height: 944 };
/** Wide enough to read, small enough that six of them are not a download. */
const OUTPUT_WIDTH = 1200;
const OUT_DIR = path.join(__dirname, '..', '..', '..', 'docs', 'images');

/**
 * Long enough for the entry stagger to finish (app.js allows 1400ms for it)
 * plus the routing screen's edges, which are drawn a frame after layout.
 * Capturing earlier catches cards mid-fade, which looks like a rendering bug.
 */
const SETTLE_MS = 1700;

/** Breathing room below the last card, so the crop is not flush with it. */
const CROP_PADDING = 44;
/** A crop shorter than this is a measurement gone wrong; keep the full frame. */
const MIN_CROP_HEIGHT = 320;

/** `light`, `dark` or `system`; `--dark` on the command line flips it. */
const THEME = process.argv.includes('--dark') ? 'dark' : 'light';

/** What each screenshot is of, in README order. */
const SHOTS = [
  { file: 'dashboard', screen: 'dash', params: {} },
  { file: 'project', screen: 'project', params: { projectId: 'prj_island' } },
  { file: 'task', screen: 'task', params: { taskId: 'T-015' } },
  { file: 'routing', screen: 'routing', params: { projectId: 'prj_island' } },
  { file: 'skills', screen: 'skills', params: { projectId: 'prj_island' } },
  { file: 'environment', screen: 'env', params: {} },
];

/**
 * The core's answer to one read, from the fixtures.
 *
 * Keyed by the *core* method name, because that is what crosses the IPC
 * channel — the preload's friendlier names (`dashboard`, `listSkills`) are a
 * renderer-side convenience and never reach Main.
 */
function payloadFor(method) {
  switch (method) {
    case 'dashboard.get':
      return FIXTURES.dash;
    case 'project.list':
      return FIXTURES.projects;
    case 'project.get':
      return FIXTURES.project;
    case 'task.get':
      return FIXTURES.task;
    case 'task.changes':
      return TASK_AT_MERGE.changes || null;
    case 'config.get':
      // The fixture carries no `ui` block, which would leave the theme to
      // whatever this machine's macOS is set to — so the README's appearance
      // would depend on who ran the tool. Naming it here pins it.
      return { ...FIXTURES.routing, global: { ...FIXTURES.routing.global, ui: { theme: THEME } } };
    case 'skills.list':
      return FIXTURES.skills;
    case 'env.get':
      return FIXTURES.env;
    case 'protocol.get':
      return PROTOCOL_SCREEN.protocol;
    case 'protocol.eval':
      return PROTOCOL_SCREEN.gate;
    case 'protocol.triggers':
      return PROTOCOL_SCREEN.triggers;
    case 'protocol.versions':
      return PROTOCOL_SCREEN.versions;
    default:
      // Loud, not empty: a screen quietly rendering its "nothing here" face
      // would ship a screenshot of an empty product.
      throw new Error(`screenshots.js has no fixture for ${method}`);
  }
}

async function run() {
  appProtocol.registerAppProtocol();
  ipcMain.handle('autome:read', (_event, request) => payloadFor(request && request.method));
  // Every button in these screenshots is real and would really fire. One write
  // is not a button: the project page asks the core to recompute rule
  // proposals while rendering (it is a write because it records what was
  // offered). Refusing it painted a red failure toast across the screenshot,
  // so it is answered with the honest empty case — this project has no
  // pending proposals — and everything else still refuses, loudly.
  ipcMain.handle('autome:write', (_event, request) => {
    if (request && request.op === 'rules.proposals') {
      return { proposals: [], experiments: [], retirement_candidates: [] };
    }
    throw new Error(`截图模式不执行写操作：${request && request.op}`);
  });

  const win = new BrowserWindow({
    show: false,
    useContentSize: true,
    width: VIEWPORT.width,
    height: VIEWPORT.height,
    webPreferences: {
      preload: path.join(__dirname, '..', 'preload.js'),
      nodeIntegration: false,
      contextIsolation: true,
      sandbox: true,
      webSecurity: true,
      backgroundThrottling: false,
    },
  });

  await win.loadURL('autome://app/index.html');

  const only = process.argv.slice(2).filter((a) => !a.startsWith('-'));
  const wanted = only.length ? SHOTS.filter((s) => only.includes(s.file)) : SHOTS;
  if (!wanted.length) throw new Error(`nothing matches ${only.join(', ')}`);

  fs.mkdirSync(OUT_DIR, { recursive: true });
  for (const shot of wanted) {
    await win.webContents.executeJavaScript(
      `(async () => {
         const app = await import('autome://app/app.js');
         await app.navigate(${JSON.stringify(shot.screen)}, ${JSON.stringify(shot.params)});
       })()`
    );
    await new Promise((resolve) => setTimeout(resolve, SETTLE_MS));

    const painted = await win.webContents.executeJavaScript(
      `(() => {
         const screen = document.querySelector('#main .screen');
         return { screen: screen ? screen.dataset.screen : null, state: screen ? screen.dataset.state : null };
       })()`
    );
    // A screen that failed to load still renders — as its error face. Shipping
    // that as documentation is worse than shipping nothing.
    if (painted.state === 'error' || !painted.screen) {
      throw new Error(`${shot.file}: the router painted ${JSON.stringify(painted)}`);
    }

    // How far down the content actually reaches. Every screen is built to fit
    // 1512x944 without scrolling (U-11), so a fixture with three cards leaves
    // half a screen of empty background — which in a README reads as a sparse
    // product rather than as a small fixture.
    const contentBottom = await win.webContents.executeJavaScript(
      `(() => {
         // Every descendant, not just the top-level blocks: a card's shadow
         // and a drawer's handle hang past their container, and a crop taken
         // at the container's edge shaves them.
         const screen = document.querySelector('#main .screen');
         const all = Array.from(screen.querySelectorAll('*'));
         const bottom = all.reduce((max, el) => {
           const rect = el.getBoundingClientRect();
           return rect.width && rect.height ? Math.max(max, rect.bottom) : max;
         }, screen.getBoundingClientRect().top);
         return Math.ceil(bottom);
       })()`
    );

    const image = await win.webContents.capturePage();
    if (image.isEmpty()) throw new Error(`${shot.file}: capturePage returned an empty image`);
    const size = image.getSize();
    const scale = size.width / VIEWPORT.width;
    const height = Math.min(size.height, Math.round((contentBottom + CROP_PADDING) * scale));
    const cropped = height >= MIN_CROP_HEIGHT * scale
      ? image.crop({ x: 0, y: 0, width: size.width, height })
      : image;
    const file = path.join(OUT_DIR, `${shot.file}.png`);
    fs.writeFileSync(file, cropped.resize({ width: OUTPUT_WIDTH }).toPNG());
    process.stdout.write(`${path.relative(process.cwd(), file)}\n`);
  }

  app.exit(0);
}

app.whenReady()
  .then(run)
  .catch((err) => {
    process.stderr.write(`${String((err && err.stack) || err)}\n`);
    app.exit(1);
  });
