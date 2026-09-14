'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const gate = require('../src/ipc-gate');

test('exactly the five read methods are allowed', () => {
  assert.deepEqual(
    [...gate.ALLOWED_READ_METHODS].sort(),
    ['project.get', 'project.list', 'queue.get', 'task.get', 'task.list'].sort()
  );
});

for (const method of ['project.list', 'project.get', 'task.list', 'task.get', 'queue.get']) {
  test(`isAllowedMethod accepts "${method}"`, () => {
    assert.equal(gate.isAllowedMethod(method), true);
  });
}

test('isAllowedMethod rejects a write method', () => {
  assert.equal(gate.isAllowedMethod('run.advance_nominal'), false);
});

test('isAllowedMethod rejects an unknown or empty method', () => {
  assert.equal(gate.isAllowedMethod('project.create'), false);
  assert.equal(gate.isAllowedMethod(''), false);
  assert.equal(gate.isAllowedMethod(undefined), false);
});

test('validateReadRequest accepts an allowed method with no params', () => {
  const result = gate.validateReadRequest({ method: 'queue.get' });
  assert.equal(result.ok, true);
  assert.equal(result.method, 'queue.get');
  assert.deepEqual(result.params, {});
});

test('validateReadRequest accepts an allowed method with plain string params', () => {
  const result = gate.validateReadRequest({ method: 'project.get', params: { project_id: 'proj-1' } });
  assert.equal(result.ok, true);
  assert.deepEqual(result.params, { project_id: 'proj-1' });
});

test('validateReadRequest rejects any method not in the allowlist', () => {
  const result = gate.validateReadRequest({ method: 'project.create', params: {} });
  assert.equal(result.ok, false);
  assert.match(result.message, /must be one of/);
});

test('validateReadRequest rejects a write method even though the sidecar itself would accept it', () => {
  const result = gate.validateReadRequest({ method: 'run.advance_nominal', params: { aggregate_id: 'x' } });
  assert.equal(result.ok, false);
});

test('validateReadRequest rejects a non-object request', () => {
  assert.equal(gate.validateReadRequest(null).ok, false);
  assert.equal(gate.validateReadRequest('queue.get').ok, false);
  assert.equal(gate.validateReadRequest(['queue.get']).ok, false);
});

test('validateReadRequest rejects non-object params', () => {
  const result = gate.validateReadRequest({ method: 'queue.get', params: 'not-an-object' });
  assert.equal(result.ok, false);
});

test('validateReadRequest rejects params carrying a nested object or array (no argv/shell smuggling)', () => {
  assert.equal(gate.validateReadRequest({ method: 'task.get', params: { task_id: { nested: true } } }).ok, false);
  assert.equal(gate.validateReadRequest({ method: 'task.get', params: { task_id: ['a', 'b'] } }).ok, false);
});

test('validateReadRequest rejects an oversized params payload', () => {
  const result = gate.validateReadRequest({
    method: 'task.get',
    params: { task_id: 'x'.repeat(gate.MAX_PAYLOAD_JSON_LENGTH) },
  });
  assert.equal(result.ok, false);
  assert.match(result.message, /exceed/);
});

test('isTrustedSenderUrl accepts only the packaged app origin', () => {
  assert.equal(gate.isTrustedSenderUrl('autome://app/index.html'), true);
  assert.equal(gate.isTrustedSenderUrl('autome://app/nested/path.html'), true);
});

test('isTrustedSenderUrl rejects any other scheme or host', () => {
  assert.equal(gate.isTrustedSenderUrl('https://evil.example/index.html'), false);
  assert.equal(gate.isTrustedSenderUrl('file:///etc/passwd'), false);
  assert.equal(gate.isTrustedSenderUrl('autome://not-app/index.html'), false);
  assert.equal(gate.isTrustedSenderUrl(''), false);
  assert.equal(gate.isTrustedSenderUrl(undefined), false);
});
