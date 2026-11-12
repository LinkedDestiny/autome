import fs from 'node:fs';
import path from 'node:path';


// The read gate is one half of the security boundary between the renderer and
// the core. Its job is narrow — reject anything not on the allowlist, and
// anything shaped wrongly — so the tests are mostly about what it *refuses*.

import { test } from 'vitest';
import assert from 'node:assert/strict';

import * as gate from '../server/gates/read-gate.ts';


test('the allowlist is frozen, so nothing can extend it at runtime', () => {
  assert.ok(Object.isFrozen(gate.ALLOWED_READ_METHODS));
  const before = gate.ALLOWED_READ_METHODS.length;
  try {
    // @ts-expect-error 检验运行时冻结
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
  
  
  const dispatch = fs.readFileSync(
    path.join(import.meta.dirname, '..', '..', '..', 'crates', 'automed', 'src', 'dispatch.rs'),
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

test('each allowed method passes with its minimal params', () => {
  for (const method of gate.ALLOWED_READ_METHODS) {
    const params = method === 'project.probe' ? { path: '~' } : {};
    const result = gate.validateReadRequest({ method, params });
    assert.equal(result.ok, true, `${method}: ${result.message}`);
    assert.deepEqual(result.params, params);
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
  assert.match(result.message, /上限/);
});


test('参数里的路径和空字符被拒绝，普通标识符通过', () => {
  for (const project_id of ['/etc', 'a/b', 'a\\b', '~user', '..', 'a\0b']) {
    assert.equal(gate.validateReadRequest({ method: 'project.get', params: { project_id } }).ok, false);
  }
  for (const project_id of ['', 'p1', '中文', '0']) {
    assert.equal(gate.validateReadRequest({ method: 'project.get', params: { project_id } }).ok, true);
  }
  const params = { id: 'x'.repeat(4087) }; // JSON 开销 9 字符，恰好 4096。
  assert.equal(gate.validateReadRequest({ method: 'project.get', params }).ok, true);
  assert.equal(gate.validateReadRequest({ method: 'project.get', params: { id: params.id + 'x' } }).ok, false);
});

test('各拒绝分支都有中文原因', () => {
  const cycle: Record<string, unknown> = {}; cycle.self = cycle;
  for (const request of [{ method: 'project.probe' }, { method: 'project.probe', params: { path: '/etc' } }, null, { method: '' }, { method: 'project.get', params: null },
    { method: 'project.get', params: cycle }, { method: 'project.get', params: { id: 'x'.repeat(4096) } },
    { method: 'project.get', params: { id: [] } }, { method: 'project.get', params: { id: '/' } }]) {
    const result = gate.validateReadRequest(request);
    assert.equal(result.ok, false);
    assert.match(result.message!, /[\u4e00-\u9fff]/);
  }
});

test('有意不放行的读方法逐个被拒，清单钉死', () => {
  assert.ok(Object.isFrozen(gate.WITHHELD_READ_METHODS));
  assert.deepEqual([...gate.WITHHELD_READ_METHODS].sort(), ['env.detect', 'events.since']);
  for (const method of gate.WITHHELD_READ_METHODS) {
    assert.equal(gate.validateReadRequest({ method }).ok, false);
  }
});

function parseCoreReads(source: string) {
  const match = source.match(/pub const READ_METHODS:\s*\[&str;\s*(\d+)\]\s*=\s*\[([\s\S]*?)\];/);
  assert.ok(match, 'READ_METHODS 声明必须可解析');
  return { declared: Number(match[1]), methods: [...match[2].matchAll(/"([^"]+)"/g)].map(m => m[1]) };
}
const dispatchSource = fs.readFileSync(path.join(import.meta.dirname, '../../../crates/automed/src/dispatch.rs'), 'utf8');
function missingReads(source: string) {
  return parseCoreReads(source).methods.filter(method =>
    !gate.ALLOWED_READ_METHODS.includes(method) && !gate.WITHHELD_READ_METHODS.includes(method));
}
function checkDeclaredCount(source: string) {
  const { declared, methods } = parseCoreReads(source);
  assert.equal(methods.length, declared);
}

test('内核读方法反向差集为空，例外属于内核且与放行不相交', () => {
  checkDeclaredCount(dispatchSource);
  assert.deepEqual(missingReads(dispatchSource), []);
  for (const method of gate.WITHHELD_READ_METHODS) {
    assert.ok(parseCoreReads(dispatchSource).methods.includes(method));
    assert.ok(!gate.ALLOWED_READ_METHODS.includes(method));
  }
});

test('反向检查抓住新增内核方法，声明数量不符也失败', () => {
  const injected = dispatchSource.replace(/(pub const READ_METHODS:[\s\S]*?)(\];)/, '$1"task.fake_read",\n$2');
  assert.deepEqual(missingReads(injected), ['task.fake_read']);
  assert.throws(() => assert.deepEqual(missingReads(injected), []));
  const wrongCount = dispatchSource.replace(/(pub const READ_METHODS:\s*\[&str;\s*)\d+/, '$1999');
  assert.throws(() => checkDeclaredCount(wrongCount));
});

test('project.probe 家目录路径通过并保持原样，载荷边界不变', () => {
  for (const value of ['~', '~/', '~/code/repo', '~/中文 目录', '~/a..b']) {
    assert.ok(gate.isHomeRelativePath(value));
    const params = { path: value };
    assert.deepEqual(gate.validateReadRequest({ method: 'project.probe', params }), { ok: true, method: 'project.probe', params });
  }
  const value = '~/' + 'a'.repeat(4083); // path 的 JSON 开销为 11。
  assert.equal(JSON.stringify({ path: value }).length, 4096);
  assert.ok(gate.validateReadRequest({ method: 'project.probe', params: { path: value } }).ok);
  assert.equal(gate.validateReadRequest({ method: 'project.probe', params: { path: value + 'a' } }).ok, false);
});

test('project.probe 拒绝非法路径，例外只属于方法与键', () => {
  for (const value of [undefined, '', 7, null, false, [], {}, '/etc', '/Users/x', 'etc', './x', '~root', '~root/x', '~/..', '~/../etc', '~/a/../../etc', '~/a\\b', '~/a\0b']) {
    const result = gate.validateReadRequest({ method: 'project.probe', params: { path: value } });
    assert.equal(result.ok, false);
    assert.match(result.message!, /[\u4e00-\u9fff]/);
  }
  for (const request of [
    { method: 'project.probe' },
    { method: 'project.probe', params: { path: '~/x', other: '~/y' } },
    { method: 'project.get', params: { project_id: '~/x' } },
    { method: 'session.log', params: { session_id: '~/x' } },
    { method: 'config.validate', params: { project_id: '~/x' } },
  ]) assert.equal(gate.validateReadRequest(request).ok, false);
});
