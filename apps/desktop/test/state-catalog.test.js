'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const catalog = require('../renderer/state-catalog');

test('§9.3 catalog: 49 states (source sentence expanded one "/"-alternative per state)', () => {
  assert.equal(catalog.STATES.length, 49);
});

test('every state has a unique id', () => {
  const ids = catalog.STATES.map((s) => s.id);
  assert.equal(new Set(ids).size, ids.length);
});

test('every state has non-empty zh and microcopy (exhaustive)', () => {
  for (const state of catalog.STATES) {
    assert.ok(state.zh && state.zh.length > 0, `state "${state.id}" has no zh label`);
    assert.ok(state.microcopy && state.microcopy.length > 0, `state "${state.id}" has no microcopy`);
  }
});

test('stateById resolves a known id', () => {
  assert.equal(catalog.stateById('no_project').zh, '无 Project');
});

test('stateById throws on an unknown id instead of returning undefined', () => {
  assert.throws(() => catalog.stateById('nonexistent'), RangeError);
});
