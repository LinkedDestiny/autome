'use strict';

// The read gate is one half of the security boundary between the renderer and
// the core. Its job is narrow — reject anything not on the allowlist, and
// anything shaped wrongly — so the tests are mostly about what it *refuses*.

const test = require('node:test');
const assert = require('node:assert/strict');

const gate = require('../src/ipc-gate');
const labels = require('../renderer/lib/labels.js');

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

test('the component names here are the ones the core puts on the wire', () => {
  // This guard exists because the two disagreed in a shipped build. The core
  // derived Serde on `Component`, which spelled `ITerm2` as `i_term2`, while
  // everything on this side says `iterm2`. The lookup never matched and the
  // environment screen reported iTerm2 as missing on a machine that had it.
  // Every desktop test passed throughout, because the fixtures used the name
  // the renderer wanted rather than the one the core sent.
  const fs = require('node:fs');
  const path = require('node:path');
  const source = fs.readFileSync(
    path.join(__dirname, '..', '..', '..', 'crates', 'autome-domain', 'src', 'environment.rs'),
    'utf8'
  );
  const block = source.slice(source.indexOf('fn as_str'));
  const wireNames = [...block.slice(0, block.indexOf('\n    }')).matchAll(/=> "([a-z0-9_]+)"/g)].map(
    (m) => m[1]
  );

  // `as_str` is only the wire form because the core serialises through it.
  // Checking the names without checking that is the same mistake again: a
  // `rename_all` derive would put a different string on the wire and this
  // test would still pass.
  const decl = source.slice(source.indexOf('pub enum Component'));
  const attrs = source.slice(source.lastIndexOf('#[derive', source.indexOf('pub enum Component')), source.indexOf('pub enum Component'));
  assert.equal(
    /rename_all|Serialize|Deserialize/.test(attrs),
    false,
    'Component must serialise through as_str, not a derive'
  );
  assert.match(decl, /serialize_str\(self\.as_str\(\)\)/);

  assert.equal(wireNames.length, labels.COMPONENT_ORDER.length, 'component count差异');
  for (const name of labels.COMPONENT_ORDER) {
    assert.ok(wireNames.includes(name), `renderer 用 ${name}，核心发的是 ${wireNames.join(', ')}`);
  }
});
