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
  const proc = spawnSync(electronBinary, [harnessPath], {
    cwd: path.join(__dirname, '..'),
    encoding: 'utf8',
    timeout: 30000,
    env: Object.assign({}, process.env, { ELECTRON_DISABLE_SECURITY_WARNINGS: 'true' }),
  });

  assert.equal(proc.status, 0, `dom-harness.js exited ${proc.status}, stderr:\n${proc.stderr}`);

  let checks;
  try {
    checks = JSON.parse(proc.stdout.trim());
  } catch (err) {
    assert.fail(`dom-harness.js did not print valid JSON.\nstdout:\n${proc.stdout}\nstderr:\n${proc.stderr}`);
  }

  assert.ok(Array.isArray(checks) && checks.length > 0, 'dom-harness.js reported zero checks');

  for (const check of checks) {
    await t.test(check.name, () => {
      assert.ok(check.pass, `detail: ${JSON.stringify(check.detail)}`);
    });
  }
});
