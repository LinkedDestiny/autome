import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync, existsSync, symlinkSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, dirname } from 'node:path';

const source = resolve(import.meta.dirname, '../..');
const files = execFileSync('git', ['ls-files', '-co', '--exclude-standard', '-z'], { cwd: source, encoding: 'utf8' }).split('\0').filter(Boolean);
function fixture(t, custom = false) {
  const dir = mkdtempSync(resolve(tmpdir(), 'ci-reuse-'));
  const root = resolve(dir, 'repo');
  mkdirSync(root);
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  for (const file of new Set(files)) {
    if (file.startsWith('docs/') || file.startsWith('.autome/')) continue;
    const path = resolve(root, file);
    mkdirSync(dirname(path), { recursive: true });
    copyFileSync(resolve(source, file), path);
  }
  const put = (path, value) => { mkdirSync(dirname(resolve(root, path)), { recursive: true }); writeFileSync(resolve(root, path), value); };
  put('docs/only.md', 'documentation');
  if (custom) {
    put('scripts/ci/steps.sh', `GLOBAL_INPUTS='scripts .gitignore'\nNO_GATE='docs'\nstep 'Stable' 'stable' sh -c 'echo stable'\nprep 'Build core' sh -c 'echo BUILD'\nstep 'Changing' 'changing' sh -c 'test "$(cat changing)" = good'\n`);
    put('stable', 'unchanged'); put('changing', 'bad');
  }
  execFileSync('git', ['init', '-q'], { cwd: root });
  execFileSync('git', ['add', '-A'], { cwd: root });
  const cleanEnv = { ...process.env };
  for (const key of Object.keys(cleanEnv)) if (key.startsWith('AUTOME_') || key === 'CI_LOCAL_ALL' || key.startsWith('GIT_')) delete cleanEnv[key];
  const run = (args = ['--plan'], env = {}) => spawnSync('sh', ['scripts/ci-local.sh', ...args], {
    cwd: root, env: { ...cleanEnv, ...env }, encoding: 'utf8', timeout: 120_000,
  });
  const plan = (env = {}, args = ['--plan']) => {
    const result = run(args, env);
    assert.equal(result.status, 0, result.stderr + result.stdout);
    return result.stdout.split('\n').filter(l => l.startsWith('plan\t')).map(l => {
      const [, action, key, name] = l.split('\t'); return { action, key, name };
    });
  };
  const passes = resolve(dir, 'passes');
  const save = rows => writeFileSync(passes, rows.map(r => `${r.key}\tT-1\tmerge_gate\tabc123\t2026-11-14`).join('\n') + '\n');
  const seed = () => { const rows = plan(); save(rows); return rows; };
  return { root, dir, put, run, plan, save, seed, passes, env: { AUTOME_GATE_PASSES: passes } };
}
const executed = rows => rows.filter(r => r.action === '执行').map(r => r.name);
const allRun = rows => assert.equal(executed(rows).length, rows.length);

test('01 Web-only input reruns Desktop and all five Web steps', t => {
  const f = fixture(t); f.seed(); f.put('apps/web/app/reuse-probe.ts', 'export {};');
  assert.deepEqual(executed(f.plan(f.env)), ['Desktop', 'Web lint', 'Web typecheck', 'Web build', 'Web test', 'Web e2e']);
});
test('02 Rust input includes Desktop and Web test dependencies', t => {
  const f = fixture(t); f.seed(); f.put('crates/automed/src/reuse-probe.rs', '// changed');
  assert.deepEqual(executed(f.plan(f.env)), ['Format', 'Clippy', 'Test (unit, end to end, adapter flags)', 'Desktop', 'Web test', 'Web e2e']);
});
test('03 docs-only input reuses everything', t => {
  const f = fixture(t); f.seed(); f.put('docs/only.md', 'new docs');
  assert.deepEqual(executed(f.plan(f.env)), []);
});
test('04 gate implementation changes invalidate every step', t => {
  const f = fixture(t); f.seed(); f.put('scripts/ci-local.sh', readFileSync(resolve(f.root, 'scripts/ci-local.sh'), 'utf8') + '\n# change\n');
  allRun(f.plan(f.env));
});
test('05 unclaimed new path invalidates all and fails claims guard', t => {
  const f = fixture(t); f.seed(); f.put('newtop/x', 'new'); allRun(f.plan(f.env));
  const result = f.run(['--check-claims']); assert.equal(result.status, 1); assert.match(result.stdout, /newtop\/x/);
});
test('06 both forced execution switches ignore matching passes', t => {
  const f = fixture(t); f.seed();
  allRun(f.plan(f.env, ['--plan', '--all'])); allRun(f.plan({ ...f.env, CI_LOCAL_ALL: '1' }));
});
test('07 worktree records including symlink aliases are rejected', t => {
  const f = fixture(t); f.seed(); f.put('docs/forged', readFileSync(f.passes));
  const alias = resolve(f.dir, 'alias'); symlinkSync(resolve(f.root, 'docs/forged'), alias);
  for (const path of [resolve(f.root, 'docs/forged'), alias]) {
    allRun(f.plan({ AUTOME_GATE_PASSES: path }));
    assert.match(f.run(['--plan'], { AUTOME_GATE_PASSES: path }).stderr, /工作区内，不采信/);
  }
});
test('08 no host variables executes all and creates no report', t => {
  const f = fixture(t, true); f.put('changing', 'good');
  const result = f.run([]); assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /==> Stable/); assert.match(result.stdout, /==> Changing/);
  assert.doesNotMatch(result.stdout, /沿用 ←/); assert.equal(existsSync(resolve(f.dir, 'report')), false);
});
test('09 failure, repair, rerun uses only unchanged successful input', t => {
  const f = fixture(t, true); const report = resolve(f.dir, 'report');
  let result = f.run([], { AUTOME_GATE_REPORT: report }); assert.equal(result.status, 1);
  const lines = readFileSync(report, 'utf8').trim().split('\n');
  assert.equal(lines[0], 'autome-gate-report 1');
  assert.match(lines[1], /^ran\t[a-f0-9]+\tStable$/); assert.match(lines[2], /^failed\t[a-f0-9]+\tChanging$/);
  f.save([{ key: lines[1].split('\t')[1] }]); f.put('changing', 'good');
  result = f.run([], { ...f.env, AUTOME_GATE_REPORT: report }); assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /沿用 ← T-1\/merge_gate@abc123 2026-11-14 · 输入/);
  assert.match(result.stdout, /ok \(\d+s\) · 输入/);
  assert.match(readFileSync(report, 'utf8'), /reused\t[^\n]+\tStable\nran\t[^\n]+\tChanging\n$/);
  assert.match(result.stdout, /  ok      Changing\n/);
});
test('10 toolchain node version changes invalidate all', t => {
  const f = fixture(t); f.seed(); const bin = resolve(f.dir, 'bin'); mkdirSync(bin);
  const node = process.execPath.replaceAll("'", "'\\''");
  writeFileSync(resolve(bin, 'node'), `#!/bin/sh\nif [ "$1" = -v ]; then echo v999; else exec '${node}' "$@"; fi\n`, { mode: 0o755 });
  allRun(f.plan({ ...f.env, PATH: `${bin}:${process.env.PATH}` }));
});
test('11 Build core precedes executing consumer even when Rust reused; absent when all reused', t => {
  const f = fixture(t, true);
  f.put('scripts/ci/steps.sh', `GLOBAL_INPUTS='scripts .gitignore'\nNO_GATE='docs'\nstep 'Rust test' 'stable' sh -c 'echo RUST'\nprep 'Build core' sh -c 'echo BUILD'\nstep 'Web test' 'changing' sh -c 'echo WEB'\n`);
  f.seed(); f.put('changing', 'changed');
  let result = f.run([], f.env); assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /==> Rust test\n    沿用/);
  assert.ok(result.stdout.indexOf('==> Build core') < result.stdout.indexOf('==> Web test'));
  f.seed(); result = f.run([], f.env); assert.equal(result.status, 0);
  assert.doesNotMatch(result.stdout, /==> Build core/); assert.doesNotMatch(result.stdout, /^WEB$/m);
});
test('12 binary override variables are cleared, logged, and excluded from keys', t => {
  const f = fixture(t, true);
  f.put('scripts/ci/steps.sh', `GLOBAL_INPUTS='scripts .gitignore'\nNO_GATE='docs stable changing'\nstep 'Environment' '' sh -c 'test -z "\${AUTOMED_BIN+x}\${AUTOME_BIN+x}\${CARGO_TARGET_DIR+x}\${CARGO_BUILD_TARGET+x}"'\n`);
  const vars = { AUTOMED_BIN: '/missing', AUTOME_BIN: '/missing', CARGO_TARGET_DIR: '/missing', CARGO_BUILD_TARGET: 'invalid' };
  assert.deepEqual(f.plan(vars), f.plan());
  const result = f.run([], vars); assert.equal(result.status, 0, result.stderr);
  for (const key of Object.keys(vars)) assert.ok(result.stdout.includes(`已忽略环境变量 ${key}`));
});

test('13 input mutation during success is partial, never a reusable pass', t => {
  const f = fixture(t, true);
  f.put('scripts/ci/steps.sh', `GLOBAL_INPUTS='scripts .gitignore'\nNO_GATE='docs changing'\nstep 'Mutate' 'stable' sh -c 'echo mutated >> stable'\n`);
  const report = resolve(f.dir, 'report'); const result = f.run([], { AUTOME_GATE_REPORT: report });
  assert.equal(result.status, 0); assert.equal(readFileSync(report, 'utf8'), 'autome-gate-report 1\npartial\t-\tMutate\n');
});
test('14 unreadable or malformed records fail closed; plan never writes report', t => {
  const f = fixture(t, true); const report = resolve(f.dir, 'report');
  for (const value of ['missing', 'malformed']) {
    if (value === 'malformed') writeFileSync(f.passes, 'not a record\n');
    allRun(f.plan({ ...f.env, AUTOME_GATE_REPORT: report }));
    assert.equal(existsSync(report), false);
  }
});
test('15 filenames with spaces, deletion, ignored files and segment boundaries', t => {
  const f = fixture(t, true); f.put('stable', 'single'); f.put('docs/with space 汉字', 'doc');
  f.seed(); f.put('node_modules/ignored-probe', 'ignored'); assert.deepEqual(executed(f.plan(f.env)), []);
  f.put('docs/with space 汉字', 'changed'); assert.deepEqual(executed(f.plan(f.env)), []);
  f.put('stable-other/file', 'outside segment'); allRun(f.plan(f.env));
  rmSync(resolve(f.root, 'stable-other'), { recursive: true });
  rmSync(resolve(f.root, 'stable')); assert.deepEqual(executed(f.plan(f.env)), ['Stable']);
});

test('16 temporary snapshot preserves real index and keys ignore commit identity', t => {
  const f = fixture(t, true);
  const git = args => execFileSync('git', args, { cwd: f.root, encoding: 'utf8' }).trim();
  git(['-c', 'core.hooksPath=/dev/null', '-c', 'user.name=Test', '-c', 'user.email=test@example.test', 'commit', '-qm', 'baseline']);
  const index = readFileSync(resolve(f.root, '.git/index')); const before = f.seed();
  assert.deepEqual(readFileSync(resolve(f.root, '.git/index')), index);
  git(['-c', 'core.hooksPath=/dev/null', '-c', 'user.name=Test', '-c', 'user.email=test@example.test', 'commit', '--allow-empty', '-qm', 'same inputs']);
  assert.deepEqual(f.plan().map(r => r.key), before.map(r => r.key));
  f.put('stable', 'unstaged'); f.plan();
  assert.equal(git(['diff', '--cached', '--name-only']), '');
});
test('17 prep failures remain failures, later checks run; missing-directory condition is honored', t => {
  const f = fixture(t, true);
  f.put('scripts/ci/steps.sh', `GLOBAL_INPUTS='scripts .gitignore'\nNO_GATE='docs changing'\nprep 'Install' --if-missing node_modules sh -c 'exit 7'\nstep 'Stable' 'stable' sh -c 'echo LATER'\n`);
  const report = resolve(f.dir, 'report'); let result = f.run([], { AUTOME_GATE_REPORT: report });
  assert.equal(result.status, 1); assert.match(result.stdout, /^LATER$/m);
  assert.match(result.stdout, /  FAILED  Install\n/);
  assert.doesNotMatch(readFileSync(report, 'utf8'), /Install/);
  mkdirSync(resolve(f.root, 'node_modules')); result = f.run([]);
  assert.equal(result.status, 0); assert.doesNotMatch(result.stdout, /==> Install/);
});
test('18 empty snapshot, invalid names, and failed snapshot creation are explicit', t => {
  const f = fixture(t, true); writeFileSync(f.passes, ''); allRun(f.plan(f.env));
  f.put('scripts/ci/steps.sh', "GLOBAL_INPUTS='scripts'\nNO_GATE=''\nstep 'bad\tname' '' true\n");
  let result = f.run(['--plan']); assert.equal(result.status, 2); assert.match(result.stderr, /步骤名不能含 Tab/);
  f.put('scripts/ci/steps.sh', "GLOBAL_INPUTS='scripts'\nNO_GATE=''\nstep 'valid' '' true\n");
  writeFileSync(resolve(f.root, '.git/index'), 'invalid index');
  result = f.run(['--plan']); assert.equal(result.status, 2); assert.doesNotMatch(result.stdout, /^plan\t/m);
});
