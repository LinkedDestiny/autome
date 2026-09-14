'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const gate = require('../src/write-gate');

test('exactly the two write ops are allowed', () => {
  assert.deepEqual(
    [...gate.ALLOWED_WRITE_OPS].sort(),
    ['project.create_from_target', 'project.pick_target'].sort()
  );
});

for (const op of ['project.pick_target', 'project.create_from_target']) {
  test(`isAllowedOp accepts "${op}"`, () => {
    assert.equal(gate.isAllowedOp(op), true);
  });
}

test('isAllowedOp rejects project.register_target (Main-only, takes a raw path)', () => {
  assert.equal(gate.isAllowedOp('project.register_target'), false);
});

test('isAllowedOp rejects project.create (the old raw-locator primitive)', () => {
  assert.equal(gate.isAllowedOp('project.create'), false);
});

test('isAllowedOp rejects an unknown or empty op', () => {
  assert.equal(gate.isAllowedOp('project.delete'), false);
  assert.equal(gate.isAllowedOp(''), false);
  assert.equal(gate.isAllowedOp(undefined), false);
});

test('validateWriteRequest rejects project.register_target even with well-formed params', () => {
  const result = gate.validateWriteRequest({
    op: 'project.register_target',
    params: { kind: 'existing_repository', path: '/Users/dannie/project/autome-v2' },
  });
  assert.equal(result.ok, false);
  assert.match(result.message, /must be one of/);
});

test('validateWriteRequest rejects project.create even with well-formed params', () => {
  const result = gate.validateWriteRequest({
    op: 'project.create',
    params: { locator: { kind: 'ExistingRepository' } },
  });
  assert.equal(result.ok, false);
  assert.match(result.message, /must be one of/);
});

test('validateWriteRequest accepts project.pick_target with an allowed kind', () => {
  for (const kind of ['new_product', 'existing_repository']) {
    const result = gate.validateWriteRequest({ op: 'project.pick_target', params: { kind } });
    assert.equal(result.ok, true);
    assert.deepEqual(result.params, { kind });
  }
});

test('validateWriteRequest rejects project.pick_target with an illegal kind', () => {
  const result = gate.validateWriteRequest({ op: 'project.pick_target', params: { kind: 'anything_else' } });
  assert.equal(result.ok, false);
  assert.match(result.message, /kind must be one of/);
});

test('validateWriteRequest rejects project.pick_target with extra params', () => {
  const result = gate.validateWriteRequest({
    op: 'project.pick_target',
    params: { kind: 'new_product', path: '/tmp' },
  });
  assert.equal(result.ok, false);
  assert.match(result.message, /accepts only \{ kind \}/);
});

test('validateWriteRequest accepts a full project.create_from_target (existing_repository shape)', () => {
  const result = gate.validateWriteRequest({
    op: 'project.create_from_target',
    params: {
      target_id: 'tgt-1',
      display_name: 'My Repo',
      trust_confirmed: true,
      destination_name: null,
    },
  });
  assert.equal(result.ok, true);
  assert.deepEqual(result.params, {
    target_id: 'tgt-1',
    display_name: 'My Repo',
    trust_confirmed: true,
    destination_name: null,
  });
});

test('validateWriteRequest accepts project.create_from_target with a destination_name (new_product shape)', () => {
  const result = gate.validateWriteRequest({
    op: 'project.create_from_target',
    params: {
      target_id: 'tgt-2',
      display_name: 'New Product',
      trust_confirmed: false,
      destination_name: 'my-new-product',
    },
  });
  assert.equal(result.ok, true);
});

test('validateWriteRequest accepts project.create_from_target when destination_name is simply absent', () => {
  const result = gate.validateWriteRequest({
    op: 'project.create_from_target',
    params: { target_id: 'tgt-3', display_name: 'My Repo', trust_confirmed: true },
  });
  assert.equal(result.ok, true);
});

test('validateWriteRequest rejects target_id when omitted entirely', () => {
  const result = gate.validateWriteRequest({
    op: 'project.create_from_target',
    params: { display_name: 'My Repo', trust_confirmed: true, destination_name: null },
  });
  assert.equal(result.ok, false);
  assert.match(result.message, /target_id/);
});

test('validateWriteRequest rejects a null, empty, or non-string target_id', () => {
  for (const targetId of [null, '', 42]) {
    const result = gate.validateWriteRequest({
      op: 'project.create_from_target',
      params: { target_id: targetId, display_name: 'My Repo', trust_confirmed: true, destination_name: null },
    });
    assert.equal(result.ok, false, `target_id=${JSON.stringify(targetId)} should be rejected`);
    assert.match(result.message, /target_id/);
  }
});

test('validateWriteRequest rejects display_name when omitted entirely', () => {
  const result = gate.validateWriteRequest({
    op: 'project.create_from_target',
    params: { target_id: 'tgt-1', trust_confirmed: true, destination_name: null },
  });
  assert.equal(result.ok, false);
  assert.match(result.message, /display_name/);
});

test('validateWriteRequest rejects a null, empty, or whitespace-only display_name', () => {
  for (const displayName of [null, '', '   ']) {
    const result = gate.validateWriteRequest({
      op: 'project.create_from_target',
      params: { target_id: 'tgt-1', display_name: displayName, trust_confirmed: true, destination_name: null },
    });
    assert.equal(result.ok, false, `display_name=${JSON.stringify(displayName)} should be rejected`);
    assert.match(result.message, /display_name/);
  }
});

test('validateWriteRequest rejects trust_confirmed when omitted entirely', () => {
  const result = gate.validateWriteRequest({
    op: 'project.create_from_target',
    params: { target_id: 'tgt-1', display_name: 'My Repo', destination_name: null },
  });
  assert.equal(result.ok, false);
  assert.match(result.message, /trust_confirmed/);
});

test('validateWriteRequest rejects a non-boolean trust_confirmed', () => {
  for (const trustConfirmed of [null, 'true', 1, 0]) {
    const result = gate.validateWriteRequest({
      op: 'project.create_from_target',
      params: { target_id: 'tgt-1', display_name: 'My Repo', trust_confirmed: trustConfirmed, destination_name: null },
    });
    assert.equal(result.ok, false, `trust_confirmed=${JSON.stringify(trustConfirmed)} should be rejected`);
    assert.match(result.message, /trust_confirmed/);
  }
});

test('validateWriteRequest rejects project.create_from_target with unknown extra params', () => {
  const result = gate.validateWriteRequest({
    op: 'project.create_from_target',
    params: {
      target_id: 'tgt-1',
      display_name: 'My Repo',
      trust_confirmed: true,
      destination_name: null,
      extra_field: 'nope',
    },
  });
  assert.equal(result.ok, false);
  assert.match(result.message, /accepts only target_id, display_name, trust_confirmed, destination_name/);
});

const PATH_SHAPES = [
  ['a forward slash', '/etc/passwd'],
  ['a backslash', 'C:\\Windows\\System32'],
  ['a leading ~', '~/secrets'],
  ['a .. traversal', '../../etc/passwd'],
  ['an embedded NUL', 'name\u0000.txt'],
];

for (const [label, value] of PATH_SHAPES) {
  test(`validateWriteRequest rejects display_name containing ${label}`, () => {
    const result = gate.validateWriteRequest({
      op: 'project.create_from_target',
      params: { target_id: 'tgt-1', display_name: value, trust_confirmed: true, destination_name: null },
    });
    assert.equal(result.ok, false);
    assert.match(result.message, /display_name must not look like a filesystem path/);
  });

  test(`validateWriteRequest rejects destination_name containing ${label}`, () => {
    const result = gate.validateWriteRequest({
      op: 'project.create_from_target',
      params: { target_id: 'tgt-1', display_name: 'My Repo', trust_confirmed: true, destination_name: value },
    });
    assert.equal(result.ok, false);
    assert.match(result.message, /destination_name must not look like a filesystem path/);
  });
}

test('validateWriteRequest accepts a display_name containing a space (not a path shape)', () => {
  const result = gate.validateWriteRequest({
    op: 'project.create_from_target',
    params: { target_id: 'tgt-1', display_name: 'My Repo', trust_confirmed: true, destination_name: null },
  });
  assert.equal(result.ok, true);
});

test('validateWriteRequest rejects an empty-string destination_name (must be non-empty when present)', () => {
  const result = gate.validateWriteRequest({
    op: 'project.create_from_target',
    params: { target_id: 'tgt-1', display_name: 'My Repo', trust_confirmed: true, destination_name: '' },
  });
  assert.equal(result.ok, false);
  assert.match(result.message, /destination_name must be a non-empty string/);
});

test('validateWriteRequest rejects a non-object request', () => {
  assert.equal(gate.validateWriteRequest(null).ok, false);
  assert.equal(gate.validateWriteRequest('project.pick_target').ok, false);
  assert.equal(gate.validateWriteRequest(['project.pick_target']).ok, false);
});

test('validateWriteRequest rejects non-object params', () => {
  const result = gate.validateWriteRequest({ op: 'project.pick_target', params: 'not-an-object' });
  assert.equal(result.ok, false);
});

test('validateWriteRequest rejects params carrying a nested object or array (no argv/shell smuggling)', () => {
  assert.equal(
    gate.validateWriteRequest({ op: 'project.pick_target', params: { kind: { nested: true } } }).ok,
    false
  );
  assert.equal(
    gate.validateWriteRequest({
      op: 'project.create_from_target',
      params: {
        target_id: 'tgt-1',
        display_name: 'My Repo',
        trust_confirmed: true,
        destination_name: ['a', 'b'],
      },
    }).ok,
    false
  );
});

test('validateWriteRequest rejects an oversized params payload', () => {
  const result = gate.validateWriteRequest({
    op: 'project.create_from_target',
    params: {
      target_id: 'tgt-1',
      display_name: 'x'.repeat(gate.MAX_PAYLOAD_JSON_LENGTH),
      trust_confirmed: true,
      destination_name: null,
    },
  });
  assert.equal(result.ok, false);
  assert.match(result.message, /exceed/);
});

test('validateWriteRequest rejects a request whose op is missing or not in the allowlist', () => {
  assert.equal(gate.validateWriteRequest({ params: {} }).ok, false);
  assert.equal(gate.validateWriteRequest({ op: '', params: {} }).ok, false);
  assert.equal(gate.validateWriteRequest({ op: 'run.advance_nominal', params: {} }).ok, false);
});

test('validateWriteRequest defaults missing params to {} for project.pick_target-shaped requests', () => {
  // project.pick_target itself requires `kind`, so omitting params must fail structurally,
  // not by throwing — confirms the {} default is applied before the per-op checks run.
  const result = gate.validateWriteRequest({ op: 'project.pick_target' });
  assert.equal(result.ok, false);
  assert.match(result.message, /kind must be one of/);
});
