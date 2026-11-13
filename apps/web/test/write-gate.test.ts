import fs from 'node:fs';
import path from 'node:path';


// The write gate is the other half of the boundary. Two properties matter
// more than the rest, and both have negative tests below:
//
//   1. The renderer can never name a filesystem path. `project.add` takes one,
//      so it is absent from the allowlist entirely; `project.pick` is the only
//      way in, and Main owns the dialog.
//   2. Free text is bounded, and allowed only where the product needs it.

import { test } from 'vitest';
import assert from 'node:assert/strict';



import * as gate from '../server/gates/write-gate.ts';

const ROLE_RS = path.join(import.meta.dirname, '..', '..', '..', 'crates', 'autome-domain', 'src', 'role.rs');

/**
 * The role wire names as the core defines them, read from `Role::as_str`'s
 * match arms. Parsed rather than listed, because a list here would be a third
 * copy that can go stale the same way the gate's did.
 */
function coreRoles() {
  const source = fs.readFileSync(ROLE_RS, 'utf8');
  const arms = [...source.matchAll(/Role::(\w+)\s*=>\s*"(\w+)"/g)].map((m) => m[2]);
  assert.ok(arms.length >= 5, `role.rs parsed as ${arms.length} roles — has as_str moved?`);
  return arms;
}

test('the allowlist is frozen', () => {
  assert.ok(Object.isFrozen(gate.ALLOWED_WRITE_OPS));
});

test('the gate knows exactly the roles the core knows', () => {
  // The gate's list had five entries while the core had six: `retro` joined
  // the loop and nothing brought this copy along. The user saw "rejected:
  // unknown role" on 复盘 — from Main, before the core was ever asked.
  assert.deepEqual(
    [...gate.ROLES].sort(),
    [...coreRoles()].sort(),
    'crates/autome-domain/src/role.rs and src/write-gate.js disagree'
  );
});

test('every role the core knows can be saved and reset', () => {
  for (const role of coreRoles()) {
    for (const op of ['config.set_role', 'config.reset_role']) {
      const result = gate.validateWriteRequest({
        op,
        params: { project_id: 'prj-1', role },
      });
      assert.equal(result.ok, true, `${op} ${role}: ${result.message}`);
    }
  }
  assert.equal(
    gate.validateWriteRequest({ op: 'config.set_role', params: { role: 'invent' } }).ok,
    false
  );
});

test('project.add is not reachable from the renderer', () => {
  // It takes a raw path. Letting the renderer call it would mean a compromised
  // renderer could register any directory on the machine as a project — and a
  // project gets a `.autome/` scaffold and an init commit written into it.
  assert.equal(gate.ALLOWED_WRITE_OPS.includes('project.add'), false);
  const result = gate.validateWriteRequest({ op: 'project.add', params: { path: '/etc' } });
  assert.equal(result.ok, false);
});

test('a read method is refused on the write channel', () => {
  for (const op of ['project.list', 'task.get', 'config.get']) {
    assert.equal(gate.validateWriteRequest({ op }).ok, false, `${op} must not be writable`);
  }
});

test('an unknown op is refused', () => {
  assert.equal(gate.validateWriteRequest({ op: 'task.nuke' }).ok, false);
});

test('a non-object request is refused', () => {
  for (const request of [null, undefined, 'task.approve', 7, []]) {
    assert.equal(gate.validateWriteRequest(request).ok, false);
  }
});

test('a task request may be prose, within a bound', () => {
  const ok = gate.validateWriteRequest({
    op: 'task.create',
    params: { project_id: 'p1', request: '给岛屿商店加购物车结算：支持优惠码。\n不要动支付网关。' },
  });
  assert.equal(ok.ok, true, ok.message);

  const tooLong = gate.validateWriteRequest({
    op: 'task.create',
    params: { project_id: 'p1', request: 'x'.repeat(gate.MAX_PROSE_LENGTH + 1) },
  });
  assert.equal(tooLong.ok, false);
  assert.match(tooLong.message, /request/);
});

test('an identifier field is held to a much tighter bound than prose', () => {
  const result = gate.validateWriteRequest({
    op: 'task.approve',
    params: { task_id: 'x'.repeat(600) },
  });
  assert.equal(result.ok, false);
  assert.match(result.message, /task_id/);
});

test('a null byte is refused anywhere in a string', () => {
  const result = gate.validateWriteRequest({
    op: 'task.approve',
    params: { task_id: 'T-1\u0000rest' },
  });
  assert.equal(result.ok, false);
  assert.match(result.message, /空字符/);
});

test('an array is allowed only where the table says, and only of short strings', () => {
  assert.equal(gate.validateWriteRequest({ op: 'task.create', params: { attachments: ['/tmp/a.pdf'] } }).ok, false);
  for (const doc_refs of [['/tmp/outside.md'], ['../../outside.md'], ['~/.ssh/config']]) {
    assert.equal(gate.validateWriteRequest({ op: 'task.create', params: { doc_refs } }).ok, false);
  }
  assert.equal(gate.validateWriteRequest({ op: 'task.create', params: { doc_refs: [] } }).ok, true);
  assert.equal(
    gate.validateWriteRequest({
      op: 'task.create',
      params: { project_id: 'p', request: 'r', attachments: ['00000000-0000-0000-0000-000000000001'] },
    }).ok,
    true
  );
  assert.equal(
    gate.validateWriteRequest({
      op: 'task.approve',
      params: { task_id: 'T-1', extra: ['a'] },
    }).ok,
    false
  );
  assert.equal(
    gate.validateWriteRequest({
      op: 'task.create',
      params: { project_id: 'p', request: 'r', attachments: [{ path: '/tmp/a' }] },
    }).ok,
    false
  );
  assert.equal(
    gate.validateWriteRequest({
      op: 'task.create',
      params: { project_id: 'p', request: 'r', surprise: ['a'] },
    }).ok,
    false,
    'an allowed op does not get to carry a list under any key it likes'
  );
});

// --- the payloads the screens actually send -------------------------------
//
// These pin the gate to its callers rather than to itself. The gate used to
// refuse every array outside `task.create`, which meant the routing screen's
// role save and the skills screen's binding drawer were both refused in full
// — `rejected: config.set_role params must not contain arrays`, on a build
// whose own gate tests were green. A rule the product needs and a rule the
// gate enforces are two different things, and only a test written from the
// call site can tell them apart.

test('the routing screen can save a role, skill list and all', () => {
  // renderer/screens/routing.js saveDraft(): every field of the draft, on
  // every save. `skills` is always an array — usually empty — so the old
  // rule refused not just skill edits but every role save there is.
  for (const skills of [[], ['autome-v2-simplify'], ['a', 'b', 'c']]) {
    const result = gate.validateWriteRequest({
      op: 'config.set_role',
      params: {
        role: 'impl',
        enabled: true,
        runtime: 'codex',
        model: 'gpt-5-codex',
        effort: null,
        skills,
      },
    });
    assert.equal(result.ok, true, `${JSON.stringify(skills)}: ${result.message}`);
  }
});

test('the skills screen can bind and unbind a skill', () => {
  // renderer/screens/skills.js saveBindings(): the role's whole list, since a
  // binding *is* the list. Unbinding the last skill sends `[]`.
  for (const skills of [['repo-facts'], []]) {
    const result = gate.validateWriteRequest({
      op: 'config.set_role',
      params: { project_id: 'prj-1', role: 'review', skills },
    });
    assert.equal(result.ok, true, `${JSON.stringify(skills)}: ${result.message}`);
  }
});

test('a skill is named, never pathed', () => {
  // A skill is a directory under a runtime's skill root, so its name is one
  // segment. The renderer only ever echoes back names the core listed; this
  // is what stops a compromised one from writing `../../` into the config.
  for (const bad of ['../../etc', 'a/b', '~/evil', '']) {
    const result = gate.validateWriteRequest({
      op: 'config.set_role',
      params: { role: 'impl', skills: [bad] },
    });
    assert.equal(result.ok, false, `${JSON.stringify(bad)} must be refused`);
  }
});

test('a list has an upper bound', () => {
  const tooMany = Array.from({ length: gate.MAX_ARRAY_ENTRIES + 1 }, (_, i) => `s${i}`);
  assert.equal(
    gate.validateWriteRequest({ op: 'config.set_role', params: { role: 'impl', skills: tooMany } })
      .ok,
    false
  );
});

test('the workspace answer carries a choice, never a path', () => {
  // `project.add` is not on the allowlist at all because it takes a path.
  // `project.confirm` is how the renderer answers a question about the
  // directory Main already picked — so it may name a member repository, and
  // that name must still not be a path.
  assert.equal(
    gate.validateWriteRequest({
      op: 'project.confirm',
      params: { workspace: true, docs_repo: 'docs' },
    }).ok,
    true
  );
  assert.equal(
    gate.validateWriteRequest({ op: 'project.confirm', params: { workspace: false } }).ok,
    true
  );
  for (const bad of ['../etc', 'a/b', '~/evil']) {
    assert.equal(
      gate.validateWriteRequest({
        op: 'project.confirm',
        params: { workspace: true, docs_repo: bad },
      }).ok,
      false,
      `${bad} must be refused`
    );
  }
  assert.equal(gate.ALLOWED_WRITE_OPS.includes('project.add'), false, 'still not reachable');
});

test('open.path is keyed by identifier and refuses anything path-shaped', () => {
  assert.equal(
    gate.validateWriteRequest({
      op: 'open.path',
      params: { kind: 'project', project_id: 'prj-1' },
    }).ok,
    true
  );
  for (const name of ['../../etc/passwd', '/etc/passwd', '~/.ssh/id_rsa', 'a/b']) {
    const result = gate.validateWriteRequest({
      op: 'open.path',
      params: { kind: 'document', task_id: 'T-1', name },
    });
    assert.equal(result.ok, false, `${name} must be refused`);
    assert.match(result.message, /路径/);
  }
});

test('open.path refuses an unknown kind', () => {
  const result = gate.validateWriteRequest({ op: 'open.path', params: { kind: 'anything' } });
  assert.equal(result.ok, false);
  assert.match(result.message, /未知的打开类型/);
});

test('an install or login names one of the four known components', () => {
  assert.equal(
    gate.validateWriteRequest({ op: 'env.install', params: { component: 'iterm2' } }).ok,
    true
  );
  for (const component of ['bash', 'node', '../../bin/sh', '']) {
    assert.equal(
      gate.validateWriteRequest({ op: 'env.install', params: { component } }).ok,
      false,
      `${component} must be refused`
    );
  }
});

test('a decision names a known kind and disposition', () => {
  assert.equal(
    gate.validateWriteRequest({
      op: 'task.decide',
      params: { task_id: 'T-1', kind: 'backlog', item_id: 'B-01', disposition: 'include' },
    }).ok,
    true
  );
  assert.equal(
    gate.validateWriteRequest({
      op: 'task.decide',
      params: { task_id: 'T-1', kind: 'nonsense', item_id: 'B-01', disposition: 'include' },
    }).ok,
    false
  );
  assert.equal(
    gate.validateWriteRequest({
      op: 'task.decide',
      params: { task_id: 'T-1', kind: 'backlog', item_id: 'B-01', disposition: 'delete' },
    }).ok,
    false
  );
});

test('a role config names a known role and runtime', () => {
  assert.equal(
    gate.validateWriteRequest({
      op: 'config.set_role',
      params: { role: 'impl', runtime: 'claude', model: 'claude-opus-5' },
    }).ok,
    true
  );
  assert.equal(
    gate.validateWriteRequest({ op: 'config.set_role', params: { role: 'implement' } }).ok,
    false
  );
  assert.equal(
    gate.validateWriteRequest({
      op: 'config.set_role',
      params: { role: 'impl', runtime: 'gemini' },
    }).ok,
    false
  );
});

test('a role config may omit the runtime, which means "leave it alone"', () => {
  const result = gate.validateWriteRequest({
    op: 'config.set_role',
    params: { role: 'impl', effort: 'high' },
  });
  assert.equal(result.ok, true, result.message);
});

test('an oversized payload is refused', () => {
  const result = gate.validateWriteRequest({
    op: 'config.set_role',
    params: { role: 'impl', skills: new Array(200).fill('x'.repeat(1000)) },
  });
  assert.equal(result.ok, false);
  assert.match(result.message, /上限/);
});

test('only the Onboarding save and task.create get headroom', () => {
  // Only these two operations carry large documents.
  assert.ok(gate.payloadLimitFor('project.onboarding.save') > gate.MAX_DOCUMENT_LENGTH);
  assert.equal(gate.payloadLimitFor('task.create'), gate.MAX_TASK_CREATE_PAYLOAD_LENGTH);
  for (const op of gate.ALLOWED_WRITE_OPS.filter(op => !['project.onboarding.save', 'task.create'].includes(op))) {
    assert.equal(gate.payloadLimitFor(op), gate.MAX_PAYLOAD_JSON_LENGTH);
  }

  const bigDocument = {
    op: 'project.onboarding.save',
    params: { project_id: 'p', path: 'AGENTS.md', content: 'x'.repeat(100 * 1024) },
  };
  assert.equal(gate.validateWriteRequest(bigDocument).ok, true);

  const tooBigDocument = {
    op: 'project.onboarding.save',
    params: { project_id: 'p', path: 'AGENTS.md', content: 'x'.repeat(gate.MAX_DOCUMENT_LENGTH + 1) },
  };
  assert.equal(gate.validateWriteRequest(tooBigDocument).ok, false);
});

test('the Onboarding save refuses any path but the two known files', () => {
  for (const path of ['README.md', '../../etc/passwd', '.autome/config.toml']) {
    const result = gate.validateWriteRequest({
      op: 'project.onboarding.save',
      params: { project_id: 'p', path, content: 'x' },
    });
    assert.equal(result.ok, false, `${path} must be refused`);
  }
  assert.equal(
    gate.validateWriteRequest({
      op: 'project.onboarding.save',
      params: { project_id: 'p', path: 'docs/agent-project-profile.md', content: 'x' },
    }).ok,
    true
  );
});

test('missing params default to an empty object', () => {
  const result = gate.validateWriteRequest({ op: 'project.pick' });
  assert.equal(result.ok, true);
  assert.deepEqual(result.params, {});
});

test('looksLikeAPath catches the shapes that matter and allows an ordinary name', () => {
  for (const value of ['a/b', 'a\\b', '~/x', '../x', 'a\u0000b']) {
    assert.equal(gate.looksLikeAPath(value), true, `${JSON.stringify(value)} is path-shaped`);
  }
  for (const value of ['checkout-flow', 'T-15', 'repo-facts', '岛屿商店']) {
    assert.equal(gate.looksLikeAPath(value), false, `${value} is an ordinary name`);
  }
});

test('a theme must be one of the three the core knows', () => {
  for (const theme of ['system', 'light', 'dark']) {
    assert.equal(
      gate.validateWriteRequest({ op: 'config.set_theme', params: { theme } }).ok,
      true,
      `${theme} must be allowed`
    );
  }
  for (const theme of ['midnight', '', 'DARK', '../x']) {
    const result = gate.validateWriteRequest({ op: 'config.set_theme', params: { theme } });
    assert.equal(result.ok, false, `${theme} must be refused`);
    assert.match(result.message, /theme/);
  }
});

test('the component names here are the ones the core puts on the wire', () => {
  // This guard exists because the two disagreed in a shipped build. The core
  // derived Serde on `Component`, which spelled `ITerm2` as `i_term2`, while
  // everything on this side says `iterm2`. The lookup never matched and the
  // environment screen reported iTerm2 as missing on a machine that had it.
  // Every desktop test passed throughout, because the fixtures used the name
  // the renderer wanted rather than the one the core sent.
  
  
  const source = fs.readFileSync(
    path.join(import.meta.dirname, '..', '..', '..', 'crates', 'autome-domain', 'src', 'environment.rs'),
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

  assert.equal(wireNames.length, gate.COMPONENTS.length, 'component count差异');
  for (const name of gate.COMPONENTS) {
    assert.ok(wireNames.includes(name), `renderer 用 ${name}，核心发的是 ${wireNames.join(', ')}`);
  }
});

test('附件必须是上传编号，路径和非数组均拒绝', () => {
  const id = 'abcdef01-2345-6789-abcd-0123456789ab';
  for (const attachments of [[], [id], Array(256).fill(id)]) {
    assert.equal(gate.validateWriteRequest({ op: 'task.create', params: { attachments } }).ok, true);
  }
  for (const attachments of [['/tmp/a.pdf'], ['a/b'], ['a\\b'], ['../a'], ['~/.ssh'], [''],
    [id.toUpperCase()], [id + 'a'], [id.slice(1)], [id + '\n'], [null], null, id, Array(257).fill(id)]) {
    assert.equal(gate.validateWriteRequest({ op: 'task.create', params: { attachments } }).ok, false);
  }
});

test('doc_refs 非空一律拒绝，空数组和缺省通过', () => {
  for (const doc_refs of [['/tmp/outside.md'], ['../../outside.md'], ['~/.ssh/config'], ['README.md'], null, '']) {
    assert.equal(gate.validateWriteRequest({ op: 'task.create', params: { doc_refs } }).ok, false);
  }
  for (const params of [{}, { doc_refs: [] }]) {
    assert.equal(gate.validateWriteRequest({ op: 'task.create', params }).ok, true);
  }
});

test('写门禁全部拒绝消息为中文', () => {
  const source = fs.readFileSync(new URL('../server/gates/write-gate.ts', import.meta.url), 'utf8');
  const messages = [...source.matchAll(/message: (?:'([^']*)'|`([^`]*)`)/g)];
  assert.ok(messages.length >= 25);
  for (const match of messages) assert.match(match[1] ?? match[2], /[\u4e00-\u9fff]/);
  const cycle: Record<string, unknown> = {}; cycle.self = cycle;
  for (const params of [null, [], '', cycle, { extra: {} }, { extra: ['x'] }]) {
    const result = gate.validateWriteRequest({ op: 'task.approve', params });
    assert.equal(result.ok, false);
    assert.match(result.message!, /[\u4e00-\u9fff]/);
  }
});

test('dispatch 中取任意 path 的方法在两个浏览器门禁都不可达', async () => {
  const read = await import('../server/gates/read-gate.ts');
  const source = fs.readFileSync(path.resolve(import.meta.dirname, '../../../crates/automed/src/dispatch.rs'), 'utf8');
  const arms = [...source.matchAll(/^ {8}"([\w.]+)" => ([\s\S]*?)(?=^ {8}"|^ {4}}\s*$)/gm)];
  const pathMethods = arms.filter((m) => m[2].includes('str_param(p, "path")')).map((m) => m[1]);
  assert.deepEqual(pathMethods.sort(), ['project.add', 'project.probe']);
  for (const method of pathMethods) {
    assert.equal(read.validateReadRequest({ method, params: { path: '/etc' } }).ok, false);
    assert.equal(gate.validateWriteRequest({ op: method, params: { path: '/etc' } }).ok, false);
  }
  const block = source.slice(source.indexOf('pub const READ_METHODS'));
  const reads = [...block.slice(0, block.indexOf('];')).matchAll(/"([\w.]+)"/g)].map((m) => m[1]);
  assert.equal(reads.length, 17);
  for (const method of reads) {
    // env.detect 是 desktop 原有的显式写入口；其它读入口不能跨到写通道。
    assert.equal(gate.ALLOWED_WRITE_OPS.includes(method), method === 'env.detect', method);
  }
});

test('两份门禁互不 import，也不 import desktop', () => {
  for (const name of ['read-gate', 'write-gate']) {
    const source = fs.readFileSync(new URL(`../server/gates/${name}.ts`, import.meta.url), 'utf8');
    assert.doesNotMatch(source, /\bimport\b|\brequire\s*\(|\bfrom\s*['"]/);
  }
});

test('长度上限钉死，边界两侧和全部 op 正路径可达', () => {
  assert.equal(gate.MAX_BRIEF_BYTES, 65536);
  assert.equal(gate.MAX_TASK_CREATE_PAYLOAD_LENGTH, 524288);
  assert.equal(gate.MAX_PROSE_LENGTH, 8000);
  assert.equal(gate.MAX_DOCUMENT_LENGTH, 262144);
  assert.equal(gate.MAX_PAYLOAD_JSON_LENGTH, 32768);
  assert.equal(gate.MAX_ARRAY_ENTRIES, 256);
  const check = (op: string, params: object) => gate.validateWriteRequest({ op, params }).ok;
  for (const [op, key, limit, extra] of [
    ['task.create', 'request', 8000, {}],
    ['task.reply', 'info', 8000, { task_id: 't1' }],
    ['task.approve', 'task_id', 512, {}],
    ['project.onboarding.save', 'content', 262144, { path: 'AGENTS.md' }],
  ] as const) {
    for (const n of [0, 1, limit]) assert.equal(check(op, { ...extra, [key]: 'x'.repeat(n) }), true);
    assert.equal(check(op, { ...extra, [key]: 'x'.repeat(limit + 1) }), false);
  }
  for (const n of [0, 1, 256, 257]) {
    assert.equal(check('config.set_role', { role: 'impl', skills: Array(n).fill('s') }), n <= 256);
  }
  for (const n of [1, 1024, 1025]) {
    assert.equal(check('config.set_role', { role: 'impl', skills: ['x'.repeat(n)] }), n <= 1024);
  }
  assert.equal(check('config.set_role', { role: 'impl', skills: ['x\0y'] }), false);
  for (const length of [32768, 32769]) {
    const params: Record<string, string> = {};
    for (let i = 0; i < 62; i++) params[`k${i}`] = 'x'.repeat(512);
    params.tail = '';
    params.tail = 'x'.repeat(length - JSON.stringify(params).length);
    assert.equal(JSON.stringify(params).length, length);
    assert.equal(check('task.approve', params), length === 32768);
  }
  const specific: Record<string, object> = {
    'task.set_after': { task_id: 'T-1', after: [] }, 'task.move': { task_id: 'T-1', top: true },
    'task.start': { task_id: 't1' }, 'task.reply': { task_id: 't1', info: '' },
    'open.path': { kind: 'log', session_id: 's1' }, 'config.set_theme': { theme: 'system' },
    'env.install': { component: 'git' }, 'env.login': { component: 'codex' },
    'task.decide': { kind: 'dispute', disposition: 'ruled' },
    'task.convert': { task_id: 'T-1', kind: 'feature' },
    'project.onboarding.save': { path: 'AGENTS.md', content: '' },
    'config.set_role': { role: 'impl' }, 'config.reset_role': { role: 'audit' },
  };
  for (const op of gate.ALLOWED_WRITE_OPS) assert.equal(check(op, specific[op] ?? {}), true, op);
  for (const key of ['project_id', 'task_id', 'session_id', 'name']) {
    assert.equal(check('open.path', { kind: 'log', [key]: '/etc' }), false);
  }
  assert.equal(check('open.path', {}), false);
  for (const op of ['env.install', 'env.login']) {
    for (const component of gate.COMPONENTS) assert.equal(check(op, { component }), true);
    assert.equal(check(op, { component: 'bash' }), false);
  }
});

test('task.start 要求非空字符串 task_id', () => {
  for (const task_id of [undefined, null, '', 7, true, [], {}]) {
    const result = gate.validateWriteRequest({ op: 'task.start', params: { task_id } });
    assert.equal(result.ok, false);
    assert.match(result.message!, /[\u4e00-\u9fff]/);
  }
  assert.equal(gate.validateWriteRequest({ op: 'task.start', params: { task_id: 't1' } }).ok, true);
});

test('task.reply 必填、类型、散文与 8000 字符边界', () => {
  for (const task_id of [undefined, null, '', 7, true, [], {}]) {
    assert.equal(gate.validateWriteRequest({ op: 'task.reply', params: { task_id, info: '补充' } }).ok, false);
  }
  for (const info of [undefined, null, 7, true, [], {}, 'x'.repeat(8001), 'a\0b']) {
    const result = gate.validateWriteRequest({ op: 'task.reply', params: { task_id: 't1', info } });
    assert.equal(result.ok, false);
    assert.match(result.message!, /[\u4e00-\u9fff]/);
  }
  for (const info of ['', ' \n\t', '中'.repeat(8000), '路径 /a/b 与换行\n补充']) {
    const params = { task_id: 't1', info };
    const result = gate.validateWriteRequest({ op: 'task.reply', params });
    assert.equal(result.ok, true);
    assert.deepEqual(result.params, params);
  }
});

test('task.create kind 枚举、缺省与 null', () => {
  for (const kind of [undefined, null, 'feature', 'bug']) {
    assert.equal(gate.validateWriteRequest({ op: 'task.create', params: { kind } }).ok, true);
  }
  for (const kind of ['', 'BUG', 'other', 7, true, [], {}]) {
    assert.equal(gate.validateWriteRequest({ op: 'task.create', params: { kind } }).ok, false);
  }
});

test('task.create draft 只接受布尔与缺省', () => {
  for (const draft of [undefined, true, false]) {
    assert.equal(gate.validateWriteRequest({ op: 'task.create', params: { draft } }).ok, true);
  }
  for (const draft of [null, '', 'true', 0, 1, [], {}]) {
    assert.equal(gate.validateWriteRequest({ op: 'task.create', params: { draft } }).ok, false);
  }
});

test('brief 以 UTF-8 字节计边界，空白语义留给内核', () => {
  for (const brief of ['', ' ', 'a', 'x'.repeat(65536), '中'.repeat(21845) + 'a', '😀'.repeat(16384)]) {
    assert.equal(gate.utf8ByteLength(brief), Buffer.byteLength(brief));
    assert.equal(gate.validateWriteRequest({ op: 'task.create', params: { brief } }).ok, true);
  }
  for (const brief of ['x'.repeat(65537), '中'.repeat(21845) + 'aa', '😀'.repeat(16384) + 'a', '中'.repeat(21846)]) {
    const result = gate.validateWriteRequest({ op: 'task.create', params: { brief } });
    assert.equal(result.ok, false);
    assert.equal(result.message, `任务卡正文超过 64KB（${Buffer.byteLength(brief)} 字节，上限 65536 字节）`);
  }
  assert.equal(gate.validateWriteRequest({ op: 'task.create', params: { brief: 'a\0b' } }).ok, false);
});

test('brief 非字符串被拒', () => {
  for (const brief of [null, 7, true, [], {}]) {
    const result = gate.validateWriteRequest({ op: 'task.create', params: { brief } });
    assert.equal(result.ok, false);
    assert.match(result.message!, /[\u4e00-\u9fff]/);
  }
});

test('task.create 最大合法组合及序列化载荷边界', () => {
  const params: Record<string, unknown> = {
    brief: '\u0001'.repeat(65536), request: '\u0001'.repeat(8000), project_id: '\u0001'.repeat(512),
    attachments: Array(256).fill('00000000-0000-0000-0000-000000000001'), kind: 'bug', draft: true,
  };
  assert.equal(gate.validateWriteRequest({ op: 'task.create', params }).ok, true);
  // Fill with individually legal fields to isolate the overall payload boundary.
  for (let i = 0; JSON.stringify(params).length + 530 < 524288; i++) params[`k${i}`] = 'x'.repeat(512);
  params.tail = '';
  params.tail = 'x'.repeat(524288 - JSON.stringify(params).length);
  assert.equal(JSON.stringify(params).length, 524288);
  assert.equal(gate.validateWriteRequest({ op: 'task.create', params }).ok, true);
  params.tail += 'x';
  const result = gate.validateWriteRequest({ op: 'task.create', params });
  assert.equal(result.ok, false);
  assert.equal(result.message, 'params 超过 524288 字符上限');
  assert.match(gate.validateWriteRequest({ op: 'task.create', params: { brief: 'x'.repeat(524288) } }).message!, /上限/);
});

test('brief 上限与超限消息解析内核源码核对', () => {
  const source = fs.readFileSync(path.resolve(import.meta.dirname, '../../../crates/automed/src/dispatch.rs'), 'utf8');
  const match = source.match(/if body\.len\(\) > (\d+) \{\s*return Err\(bad_params\(format!\(\s*"([^"]+)"/);
  assert.ok(match, '必须解析到内核正文长度检查，不能空匹配假绿');
  assert.equal(gate.MAX_BRIEF_BYTES, Number(match[1]));
  const bytes = Number(match[1]) + 1;
  const result = gate.validateWriteRequest({ op: 'task.create', params: { brief: 'a'.repeat(bytes) } });
  assert.equal(result.ok, false);
  assert.equal(result.message, match[2].replace('{}', String(bytes)));
});

test('task.convert 只接受需求方向、非空任务编号与可选布尔 start', async () => {
  const valid = { task_id: 'T-1', kind: 'feature' };
  for (const params of [valid, { ...valid, start: true }, { ...valid, start: false },
    { ...valid, task_id: 'x'.repeat(512) }, { ...valid, task_id: 'x' }]) {
    assert.equal(gate.validateWriteRequest({ op: 'task.convert', params }).ok, true, JSON.stringify(params));
  }
  for (const [params, message] of [
    [{ kind: 'feature' }, 'task_id 必须是非空字符串'],
    [{ ...valid, task_id: '' }, 'task_id 必须是非空字符串'],
    [{ ...valid, task_id: 1 }, 'task_id 必须是非空字符串'],
    [{ task_id: 'T-1' }, 'kind 只能是 feature'],
    [{ ...valid, kind: 'bug' }, 'kind 只能是 feature'],
    [{ ...valid, start: 'yes' }, 'start 必须是布尔值'],
    [{ ...valid, start: 1 }, 'start 必须是布尔值'],
    [{ ...valid, task_id: 'x'.repeat(513) }, 'task_id 超过 512 字符上限'],
    [{ ...valid, start: null }, 'start 必须是布尔值'],
  ] as const) {
    const result = gate.validateWriteRequest({ op: 'task.convert', params });
    assert.equal(result.ok, false, JSON.stringify(params));
    assert.equal(result.message, message);
  }
  const read = await import('../server/gates/read-gate.ts');
  assert.equal(read.validateReadRequest({ method: 'task.convert', params: valid }).ok, false);
});

test('依赖与移动操作放行，task_id 必须是非空字符串', () => {
  for (const [op, extra] of [['task.set_after', { after: [] }], ['task.move', { top: true }]] as const) {
    const params = { task_id: 'T-1', ...extra };
    assert.deepEqual(gate.validateWriteRequest({ op, params }), { ok: true, op, params });
    for (const task_id of [undefined, null, '', 7, true, [], {}]) {
      const result = gate.validateWriteRequest({ op, params: { ...extra, task_id } });
      assert.equal(result.ok, false);
      assert.match(result.message!, /[\u4e00-\u9fff]/);
    }
  }
});

test('task.move 三个位置恰好一个，位置编号与 top 类型受限', () => {
  for (let mask = 0; mask < 8; mask++) {
    const params = { task_id: 'T-1', ...(mask & 1 ? { before: 'T-2' } : {}),
      ...(mask & 2 ? { after: 'T-3' } : {}), ...(mask & 4 ? { top: true } : {}) };
    assert.equal(gate.validateWriteRequest({ op: 'task.move', params }).ok, [1, 2, 4].includes(mask));
  }
  for (const key of ['before', 'after']) {
    for (const id of ['', 't-1', 'T-', 'T--1', 'T-1.0', 'T-１', ' T-1', 'T-1\n', null, 1, true, [], {}]) {
      assert.equal(gate.validateWriteRequest({ op: 'task.move', params: { task_id: 'T-1', [key]: id } }).ok, false);
    }
    for (const id of ['T-0', 'T-01', 'T-99999999999999999999']) {
      assert.equal(gate.validateWriteRequest({ op: 'task.move', params: { task_id: 'T-1', [key]: id, top: false } }).ok, true);
    }
  }
  for (const top of [false, null, '', 'true', 1, [], {}]) {
    assert.equal(gate.validateWriteRequest({ op: 'task.move', params: { task_id: 'T-1', top } }).ok, false);
  }
});

test('创建与修改依赖的列表边界和编号格式，移动 after 是单个编号', () => {
  for (const op of ['task.create', 'task.set_after']) {
    for (const count of [0, 1, 8, 9]) {
      for (const after of [Array(count).fill('T-1'), Array.from({ length: count }, (_, i) => `T-${i}`)]) {
        assert.equal(gate.validateWriteRequest({ op, params: { task_id: 'T-1', after } }).ok, count <= 8);
      }
    }
    for (const after of [null, '', 'T-1', 1, true, {}, [''], ['bad'], ['T-1\n'], ['T--1'], ['T-１'], [1], [null]]) {
      const result = gate.validateWriteRequest({ op, params: { task_id: 'T-1', after } });
      assert.equal(result.ok, false);
      assert.match(result.message!, /[\u4e00-\u9fff]/);
    }
    assert.equal(gate.validateWriteRequest({ op, params: { task_id: 'T-1', after: ['T-0', 'T-01'] } }).ok, true);
    assert.equal(gate.validateWriteRequest({ op, params: { task_id: 'T-1' } }).ok, op === 'task.create');
  }
  assert.equal(gate.validateWriteRequest({ op: 'task.move', params: { task_id: 'T-1', after: ['T-2'] } }).ok, false);
});
