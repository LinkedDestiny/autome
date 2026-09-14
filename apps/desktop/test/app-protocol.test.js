'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');

test('app-protocol exports RENDERER_DIR pointing at the renderer/ directory', () => {
  const appProtocol = require('../src/app-protocol');
  assert.equal(appProtocol.RENDERER_DIR, path.join(__dirname, '..', 'renderer'));
});

test('app-protocol exposes registerSchemeAsPrivileged and registerAppProtocol as functions', () => {
  const appProtocol = require('../src/app-protocol');
  assert.equal(typeof appProtocol.registerSchemeAsPrivileged, 'function');
  assert.equal(typeof appProtocol.registerAppProtocol, 'function');
});
