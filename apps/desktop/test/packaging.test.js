'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { build } = require('../package.json');
const root = path.resolve(__dirname, '../../..');
const read = name => fs.readFileSync(path.join(root, name), 'utf8');

test('files only ships the thin shell', () => {
  assert.deepEqual(build.files, ['main.js', 'preload.js', 'src/**/*']);
  assert.equal(fs.existsSync(path.join(root, 'apps/desktop/renderer')), false);
  assert.deepEqual(fs.readdirSync(path.join(root, 'apps/desktop/src')), ['shell']);
});
test('core and web are staged as resources at the shell lookup paths', () => {
  assert.deepEqual(build.extraResources, [{ from: 'core', to: 'core' }, { from: 'web', to: 'web' }]);
  const main = read('apps/desktop/main.js');
  assert.ok(main.includes("'core/autome'"));
  assert.match(main, /'web'/);
});
test('asar contains JavaScript only, executable resources stay outside', () => {
  assert.equal(build.asar, true);
  assert.ok(!build.files.some(file => /^(core|web)/.test(file)));
});
test('hardened runtime keeps Electron capabilities without AppleEvents', () => {
  assert.equal(build.mac.hardenedRuntime, true);
  assert.equal(build.mac.entitlementsInherit, build.mac.entitlements);
  const plist = read('apps/desktop/' + build.mac.entitlements);
  for (const name of ['allow-jit', 'allow-unsigned-executable-memory', 'disable-library-validation']) {
    assert.ok(plist.includes('com.apple.security.cs.' + name));
  }
  assert.doesNotMatch(plist, /automation\.apple-events/);
  assert.equal(build.mac.extendInfo?.NSAppleEventsUsageDescription, undefined);
});
test('package builds and stages autome plus web before electron-builder', () => {
  const script = read('scripts/package.sh');
  assert.match(script, /cargo build --release[^\n]*-p autome-cli/);
  assert.match(script, /cp "\$root\/target\/release\/autome" "\$staging\/autome"/);
  assert.match(script, /chmod 755 "\$staging\/autome"/);
  assert.doesNotMatch(script, /\bautomed\b/);
  assert.ok(script.indexOf('run stage-web') < script.indexOf('npx electron-builder'));
  assert.match(script, /CSC_IDENTITY_AUTO_DISCOVERY=false/);
});
test('both CI bundle checks execute autome and run packaged panel smoke', () => {
  for (const file of ['scripts/ci-local.sh', '.github/workflows/ci.yml']) {
    const script = read(file);
    assert.match(script, /Resources\/core\/autome" --version/);
    assert.doesNotMatch(script, /Resources\/core\/automed/);
    assert.match(script, /Bundle launches the panel/);
    assert.match(script, /AUTOME_SMOKE_APP=.*Autome\.app.*npm --prefix apps\/desktop run smoke/);
  }
});
test('package prepares web before staging and packaging', () => {
  const script = read('scripts/package.sh');
  const prepare = script.indexOf('sh "$root/scripts/prepare-web.sh"');
  assert.ok(prepare >= 0);
  assert.ok(prepare < script.indexOf('run stage-web'));
  assert.ok(script.indexOf('run stage-web') < script.indexOf('npx electron-builder'));
});
