'use strict';

// The read gate is one half of the security boundary between the renderer and
// the core. Its job is narrow — reject anything not on the allowlist, and
// anything shaped wrongly — so the tests are mostly about what it *refuses*.

const test = require('node:test');
const assert = require('node:assert/strict');

const gate = require('../src/ipc-gate');

test('the allowlist is frozen, so nothing can extend it at runtime', () => {
  assert.ok(Object.isFrozen(gate.ALLOWED_READ_METHODS));
  const before = gate.ALLOWED_READ_METHODS.length;
  try {
    gate.ALLOWED_READ_METHODS.push('project.add');
  } catch {
    // Strict mode throws; either way the list must not grow.
  }
  assert.equal(gate.ALLOWED_READ_METHODS.length, before);
});

test('every allowed method is a read in the core', () => {
  // The core's own READ_METHODS is the authority (dispatch.rs). Keeping the
  // two lists in step is what makes the read/write split meaningful: a method
  // here that mutates would turn the separate write gate into decoration.
  const fs = require('node:fs');
  const path = require('node:path');
  const dispatch = fs.readFileSync(
    path.join(__dirname, '..', '..', '..', 'crates', 'automed', 'src', 'dispatch.rs'),
    'utf8'
  );
  const block = dispatch.slice(dispatch.indexOf('pub const READ_METHODS'));
  const coreList = block.slice(0, block.indexOf('];'));
  for (const method of gate.ALLOWED_READ_METHODS) {
    assert.ok(
      coreList.includes(`"${method}"`),
      `${method} is allowed here but is not in the core's READ_METHODS`
    );
  }
});

test('a write method is refused on the read channel', () => {
  for (const method of [
    'project.add',
    'project.remove',
    'task.create',
    'task.approve',
    'config.set_role',
  ]) {
    const result = gate.validateReadRequest({ method });
    assert.equal(result.ok, false, `${method} must not be readable`);
  }
});

test('an unknown method is refused and the message lists the real ones', () => {
  const result = gate.validateReadRequest({ method: 'project.destroy' });
  assert.equal(result.ok, false);
  assert.match(result.message, /project\.list/);
});

test('each allowed method passes with no params', () => {
  for (const method of gate.ALLOWED_READ_METHODS) {
    const result = gate.validateReadRequest({ method });
    assert.equal(result.ok, true, `${method}: ${result.message}`);
    assert.deepEqual(result.params, {});
  }
});

test('a non-object request is refused', () => {
  for (const request of [null, undefined, 'project.list', 42, ['project.list']]) {
    assert.equal(gate.validateReadRequest(request).ok, false);
  }
});

test('params must be a plain object of scalars', () => {
  assert.equal(gate.validateReadRequest({ method: 'project.get', params: [] }).ok, false);
  assert.equal(gate.validateReadRequest({ method: 'project.get', params: 'x' }).ok, false);
  assert.equal(
    gate.validateReadRequest({ method: 'project.get', params: { nested: { a: 1 } } }).ok,
    false
  );
  assert.equal(
    gate.validateReadRequest({ method: 'project.get', params: { list: [1, 2] } }).ok,
    false
  );
  assert.equal(
    gate.validateReadRequest({
      method: 'project.get',
      params: { project_id: 'p1', n: 3, flag: true },
    }).ok,
    true
  );
});

test('an oversized payload is refused before it reaches the sidecar pipe', () => {
  const params = { project_id: 'x'.repeat(gate.MAX_PAYLOAD_JSON_LENGTH) };
  const result = gate.validateReadRequest({ method: 'project.get', params });
  assert.equal(result.ok, false);
  assert.match(result.message, /limit/);
});

test('only the packaged origin is trusted', () => {
  assert.ok(gate.isTrustedSenderUrl('autome://app/index.html'));
  assert.ok(gate.isTrustedSenderUrl('autome://app/screens/task.js'));
  for (const url of [
    'autome://other/index.html',
    'file:///Users/me/index.html',
    'https://example.com/',
    'autome://app',
    '',
    null,
    undefined,
  ]) {
    assert.equal(gate.isTrustedSenderUrl(url), false, `${url} must not be trusted`);
  }
});
