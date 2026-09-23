'use strict';

// Real-DOM layer (plan §"测试", item 2): spawns dom-harness.js under the
// actual Electron binary — not plain node — so the assertions run inside
// the real Chromium renderer with the real CSP, the real autome://
// protocol, and the real app.js/nav.js wiring. No sidecar is started.
//
// Electron binary missing/unresolvable -> explicit skip, never a silent
// pass (same discipline as sidecar.test.js's build-precondition check).

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

function resolveElectronBinary() {
  let electronPath;
  try {
    electronPath = require('electron');
  } catch {
    return null;
  }
  if (typeof electronPath !== 'string') return null;
  const resolved = path.isAbsolute(electronPath) ? electronPath : path.join(__dirname, '..', electronPath);
  return fs.existsSync(resolved) ? resolved : null;
}

test('electron dom harness: real-DOM assertions against the actual renderer', { concurrency: false }, async (t) => {
  const electronBinary = resolveElectronBinary();
  if (!electronBinary) {
    t.skip('electron binary not found (run `npm install` in apps/desktop first)');
    return;
  }

  const harnessPath = path.join(__dirname, 'dom-harness.js');
  // Through files rather than pipes, and that is the whole point of the
  // detour: `spawnSync`'s timeout kills the *child*, then keeps reading its
  // pipes until they close — and Electron's helper processes inherit them.
  // On a CI runner the helpers outlived the kill, the read never ended, and
  // the job sat there for 45 minutes before the runner's own limit put it
  // down. With file descriptors there is no pipe for anyone to hold open, so
  // the timeout below actually bounds the step.
  const scratch = fs.mkdtempSync(path.join(os.tmpdir(), 'autome-dom-'));
  const outPath = path.join(scratch, 'checks.json');
  const errPath = path.join(scratch, 'stderr.txt');
  const outFd = fs.openSync(outPath, 'w');
  const errFd = fs.openSync(errPath, 'w');
  let proc;
  try {
    proc = spawnSync(electronBinary, [harnessPath], {
      cwd: path.join(__dirname, '..'),
      timeout: 60000,
      killSignal: 'SIGKILL',
      stdio: ['ignore', outFd, errFd],
      env: Object.assign({}, process.env, { ELECTRON_DISABLE_SECURITY_WARNINGS: 'true' }),
    });
  } finally {
    fs.closeSync(outFd);
    fs.closeSync(errFd);
  }
  const stdout = fs.readFileSync(outPath, 'utf8');
  const stderr = fs.readFileSync(errPath, 'utf8');
  fs.rmSync(scratch, { recursive: true, force: true });

  assert.ok(
    !proc.error || proc.error.code !== 'ETIMEDOUT',
    `dom-harness.js did not finish within 60s. stderr:\n${stderr}`
  );
  assert.equal(proc.status, 0, `dom-harness.js exited ${proc.status}, stderr:\n${stderr}`);

  let checks;
  try {
    checks = JSON.parse(stdout.trim());
  } catch (err) {
    assert.fail(`dom-harness.js did not print valid JSON.\nstdout:\n${stdout}\nstderr:\n${stderr}`);
  }

  assert.ok(Array.isArray(checks) && checks.length > 0, 'dom-harness.js reported zero checks');

  for (const check of checks) {
    await t.test(check.name, () => {
      assert.ok(check.pass, `detail: ${JSON.stringify(check.detail)}`);
    });
  }
});
