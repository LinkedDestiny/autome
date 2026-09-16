'use strict';

// Guards on the packaging configuration. Technical design §18, milestone M4.
//
// Every assertion here corresponds to a failure that only appears *after* a
// build — usually after signing, on someone else's machine, with no useful
// error. That is the worst place to discover a one-line configuration
// mistake, so each one is pinned here instead.

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const pkg = require('../package.json');
const build = pkg.build;

test('the bundle ships only what the app loads', () => {
  // A `files` list that includes the test directory ships the fixtures — and
  // the fixtures contain invented project paths and task text — into a signed
  // bundle. The list is an allowlist for exactly that reason.
  const files = build.files;
  assert.ok(Array.isArray(files) && files.length > 0);
  for (const unwanted of ['test', 'test/**', '**/*', '.']) {
    assert.ok(!files.includes(unwanted), `files must not include ${unwanted}`);
  }
  for (const needed of ['main.js', 'preload.js', 'src/**/*', 'renderer/**/*']) {
    assert.ok(files.includes(needed), `files must include ${needed}`);
  }
});

test('the core binary is staged where the sidecar looks for it', () => {
  // `sidecar.js` resolves `process.resourcesPath/core/<exe>` when packaged.
  // If `extraResources` put it anywhere else the app would start, find no
  // core, and show an empty window with a reconnect banner forever.
  const resources = build.extraResources;
  assert.ok(Array.isArray(resources));
  const core = resources.find((r) => r.to === 'core');
  assert.ok(core, 'extraResources must place something at core/');
  assert.equal(core.from, 'core');

  const sidecar = fs.readFileSync(path.join(__dirname, '..', 'src', 'sidecar.js'), 'utf8');
  assert.match(
    sidecar,
    /process\.resourcesPath,\s*'core'/,
    'the sidecar must look in the same place extraResources writes to'
  );
});

test('the core binary is a resource, not asar-packed', () => {
  // It has to be executable on disk. Inside the asar archive it is not a file
  // the kernel can exec, and the failure is "cannot start the core" with no
  // indication that packing is the reason.
  assert.equal(build.asar, true, 'the JavaScript should be packed');
  const packedCore = (build.files || []).some((f) => String(f).startsWith('core'));
  assert.ok(!packedCore, 'core/ must not also be in files');
});

test('the hardened runtime is on and declares every capability the app uses', () => {
  assert.equal(build.mac.hardenedRuntime, true);
  const entitlementsPath = path.join(__dirname, '..', build.mac.entitlements);
  assert.ok(fs.existsSync(entitlementsPath), `${build.mac.entitlements} must exist`);
  const plist = fs.readFileSync(entitlementsPath, 'utf8');

  // The session launcher drives iTerm2 or Terminal through osascript. Without
  // this entitlement every session fails to start in a signed build — and only
  // in a signed build, so it would pass every test up to release.
  assert.match(plist, /com\.apple\.security\.automation\.apple-events/);
  // Electron's own requirements.
  for (const entitlement of [
    'com.apple.security.cs.allow-jit',
    'com.apple.security.cs.allow-unsigned-executable-memory',
    'com.apple.security.cs.disable-library-validation',
  ]) {
    assert.match(plist, new RegExp(entitlement.replace(/\./g, '\\.')));
  }
  // Inherited too, or the spawned core loses them.
  assert.equal(build.mac.entitlementsInherit, build.mac.entitlements);
});

test('macOS asks the user about AppleEvents in words they can act on', () => {
  // Without a usage description macOS shows a blank prompt, and the user has
  // no idea why a development tool wants to control their terminal.
  const reason = build.mac.extendInfo.NSAppleEventsUsageDescription;
  assert.ok(typeof reason === 'string' && reason.length > 10);
  assert.ok(/iTerm2|Terminal/.test(reason), 'it should name what it will open');
});

test('the build script signs only when told to, and says which it did', () => {
  const script = fs.readFileSync(
    path.join(__dirname, '..', '..', '..', 'scripts', 'package.sh'),
    'utf8'
  );
  // Best-effort signing is worse than none: an unsigned build that claims to
  // be signed gets distributed.
  assert.match(script, /CSC_IDENTITY_AUTO_DISCOVERY=false/);
  assert.match(script, /cargo build --release/);
  // The staged binary must be executable, or the bundle ships a file the
  // kernel refuses to exec.
  assert.match(script, /chmod 755/);
});

test('the app version matches the product it claims to be', () => {
  assert.match(pkg.version, /^2\./);
  assert.equal(build.productName, 'Autome');
  assert.match(build.appId, /^[a-z]+(\.[a-z]+)+$/);
});

test('build outputs are not committed', () => {
  const gitignore = fs.readFileSync(
    path.join(__dirname, '..', '..', '..', '.gitignore'),
    'utf8'
  );
  for (const entry of ['dist/', 'apps/desktop/core/']) {
    assert.ok(
      gitignore.split('\n').some((l) => l.trim() === entry.trim()),
      `.gitignore should list ${entry}`
    );
  }
});

test('the window accepts the click that activates it', () => {
  // macOS eats that click by default. Sessions open terminals that steal
  // focus, so the user is constantly clicking back into an inactive window;
  // without this every first click is a no-op and the app feels broken.
  const fs = require('node:fs');
  const path = require('node:path');
  const main = fs.readFileSync(path.join(__dirname, '..', 'main.js'), 'utf8');
  const options = main.slice(main.indexOf('new BrowserWindow('));
  assert.match(options.slice(0, options.indexOf('webPreferences')), /acceptFirstMouse:\s*true/);
});

test('the window chrome does not paint its own macOS buttons', () => {
  // The mock drew three circles where macOS draws close/minimise/zoom. Under
  // `titleBarStyle: 'hiddenInset'` the real ones are there too, so the fakes
  // were decoration sitting on top of working controls. They were also the
  // only thing holding that strip open — hence the padding assertion.
  const fs = require('node:fs');
  const path = require('node:path');
  const dir = path.join(__dirname, '..', 'renderer');
  const html = fs.readFileSync(path.join(dir, 'index.html'), 'utf8');
  const css = fs.readFileSync(path.join(dir, 'style.css'), 'utf8');

  assert.equal(/class="lights"/.test(html), false, 'fake traffic lights are back in the markup');
  assert.equal(/^\.lights\b/m.test(css), false, 'fake traffic-light styles are back');

  const rule = /\.titlebar__left\s*\{[^}]*padding-left:\s*(\d+)px/.exec(css);
  assert.ok(rule, '.titlebar__left must set a left padding');
  assert.ok(
    Number(rule[1]) >= 72,
    `padding-left ${rule[1]}px puts the brand under the real macOS buttons`
  );
});

test('every write op Main intercepts has a function behind it', () => {
  // `open.terminal` was on the write allowlist, wired to `openTerminal(params)`
  // in the write handler, and that function did not exist anywhere. Pressing
  // the button threw `openTerminal is not defined` — a reference error that no
  // test saw, because main.js needs Electron to load and nothing read it as
  // text either.
  const fs = require('node:fs');
  const path = require('node:path');
  const main = fs.readFileSync(path.join(__dirname, '..', 'main.js'), 'utf8');

  const intercepts = [...main.matchAll(/if \(op === '([\w.]+)'\) return (\w+)\(/g)];
  assert.ok(intercepts.length >= 5, `expected Main to intercept several ops, saw ${intercepts.length}`);
  for (const [, op, fn] of intercepts) {
    assert.ok(
      new RegExp(`(async )?function ${fn}\\s*\\(`).test(main),
      `${op} is routed to ${fn}(), which is not defined in main.js`
    );
  }
});

test('every op on the write allowlist reaches the core or a handler in Main', () => {
  // The other half: an op allowed by the gate that Main neither intercepts nor
  // the core implements would fail only when someone pressed it.
  const fs = require('node:fs');
  const path = require('node:path');
  const root = path.join(__dirname, '..', '..', '..');
  const gate = require('../src/write-gate');
  const main = fs.readFileSync(path.join(__dirname, '..', 'main.js'), 'utf8');
  const dispatch = fs.readFileSync(path.join(root, 'crates', 'automed', 'src', 'dispatch.rs'), 'utf8');

  for (const op of gate.ALLOWED_WRITE_OPS) {
    const inMain = main.includes(`op === '${op}'`);
    const inCore = dispatch.includes(`"${op}"`);
    assert.ok(inMain || inCore, `${op} is allowed but nothing implements it`);
  }
});

test('taking a stance refreshes the decide drawer instead of closing it', () => {
  // Structural, not behavioural: exercising this needs a live core, and the
  // real check was run against one. What it pins is the specific regression —
  // `onDone` used to call `close()`, so triaging nine Backlog items meant
  // reopening the drawer nine times.
  const fs = require('node:fs');
  const path = require('node:path');
  const src = fs.readFileSync(path.join(__dirname, '..', 'renderer', 'screens', 'task.js'), 'utf8');

  const start = src.indexOf('function decideRow(');
  assert.ok(start > 0, 'decideRow must exist');
  const body = src.slice(start, src.indexOf('\nfunction ', start + 10));

  assert.ok(
    body.includes('refreshDecideDrawer('),
    'a stance must rebuild the drawer body from a fresh read'
  );
  assert.equal(
    /onDone:[\s\S]{0,200}?\bclose\(\)/.test(body),
    false,
    'a stance must not close the drawer'
  );
});
