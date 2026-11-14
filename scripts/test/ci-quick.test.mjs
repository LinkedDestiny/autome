import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync, appendFileSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, dirname } from 'node:path';

const source = resolve(import.meta.dirname, '../..');
const files = execFileSync('git', ['ls-files', '-co', '--exclude-standard', '-z'], { cwd: source, encoding: 'utf8' }).split('\0').filter(Boolean);
const cargo = execFileSync('sh', ['-c', 'command -v cargo'], { encoding: 'utf8' }).trim();
const cleanEnv = { ...process.env };
for (const key of Object.keys(cleanEnv)) if (key.startsWith('AUTOME_') || key.startsWith('CI_QUICK_') || key.startsWith('GIT_') || key === 'CI_LOCAL_ALL') delete cleanEnv[key];
function fixture(t, fake = false) {
  const dir = mkdtempSync(resolve(tmpdir(), 'ci-quick-'));
  const root = resolve(dir, 'repo'); mkdirSync(root);
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  for (const file of new Set(files)) {
    if (file.startsWith('docs/') || file.startsWith('.autome/')) continue;
    mkdirSync(dirname(resolve(root, file)), { recursive: true });
    copyFileSync(resolve(source, file), resolve(root, file));
  }
  const put = (path, value) => { mkdirSync(dirname(resolve(root, path)), { recursive: true }); writeFileSync(resolve(root, path), value); };
  const git = args => execFileSync('git', args, { cwd: root, encoding: 'utf8' }).trim();
  const env = { ...cleanEnv };
  if (fake) {
    put('scripts/ci/steps.sh', `GLOBAL_INPUTS='scripts'\nNO_GATE='docs'\nstep 'Format' 'crates' true\nstep 'Clippy' 'crates protocol' true\nprep 'Web install' sh -c 'echo INSTALL'\nstep 'Web lint' 'apps/web' true\nstep 'Web typecheck' 'apps/web' true\nprep 'Desktop install' true\nprep 'Build core' sh -c 'echo BUILD'\nstep 'Desktop' 'apps/desktop crates protocol' sh -c 'echo DESKTOP'\nstep 'Rust test' 'crates protocol' false\nstep 'Web test' 'apps/web crates protocol' false\n`);
    const bin = resolve(dir, 'bin'); mkdirSync(bin);
    writeFileSync(resolve(bin, 'npx'), '#!/bin/sh\necho "WEB CANDIDATE $*"\n', { mode: 0o755 });
    writeFileSync(resolve(bin, 'cargo'), `#!/bin/sh\ncase "$1" in metadata|-V|clippy) exec '${cargo}' "$@" ;; esac\necho "CANDIDATE $*"\nif [ "\${QUICK_SLEEP-}" = 1 ]; then sleep 2; fi\nif [ "\${QUICK_FAIL-}" = 1 ]; then printf '     Running tests/cli.rs (target/debug/deps/cli-abc)\\nfailures:\\n    changed_case\\n\\n'; exit 1; fi\n`, { mode: 0o755 });
    env.PATH = `${bin}:${env.PATH}`;
  }
  git(['init', '-q']); git(['add', '-A']);
  git(['-c', 'core.hooksPath=/dev/null', '-c', 'user.name=Test', '-c', 'user.email=test@example.test', 'commit', '-qm', 'baseline']);
  const base = git(['rev-parse', 'HEAD']);
  const report = resolve(dir, 'report');
  const run = (args = ['--plan'], more = {}) => spawnSync('sh', ['scripts/ci-quick.sh', ...args], {
    cwd: root, env: { ...env, AUTOME_ROUND_START: base, ...more }, encoding: 'utf8', timeout: 120_000,
  });
  const plan = (more = {}, args = ['--plan']) => { const r = run(args, more); assert.equal(r.status, 0, r.stdout + r.stderr); return r.stdout; };
  const change = path => appendFileSync(resolve(root, path), '\n// quick change\n');
  return { root, dir, put, git, base, report, run, plan, change };
}
const commands = output => output.split('\n').filter(l => l.startsWith('candidate\t')).map(l => JSON.parse(l.slice(10)));
const ok = r => assert.equal(r.status, 0, r.stdout + r.stderr);

test('01 changed integration target only; committed, unstaged and untracked changes are included', t => {
  const f = fixture(t); f.change('crates/autome-cli/tests/cli.rs');
  let list = commands(f.plan());
  assert.deepEqual(list, [['cargo', 'test', '-p', 'autome-cli', '--test', 'cli', '--no-fail-fast']]);
  f.git(['add', '-A']); f.git(['-c', 'core.hooksPath=/dev/null', '-c', 'user.name=Test', '-c', 'user.email=t@t', 'commit', '-qm', 'change']);
  f.put('crates/autome-cli/tests/new.rs', '#[test] fn new() {}');
  list = commands(f.plan()); assert.equal(list.length, 2); assert.ok(list.every(c => !c.includes('automed')));
});
test('02 automed source selects its entire lib, without domain tests', t => {
  const f = fixture(t); f.change('crates/automed/src/gate.rs');
  assert.deepEqual(commands(f.plan()), [['cargo', 'test', '-p', 'automed', '--lib', '--no-fail-fast']]);
});
test('03 one Web related command includes source and test, quoted paths and Build core first', t => {
  const f = fixture(t); f.put("apps/web/app/with space '汉字.ts", 'export {};'); f.change('apps/web/test/ci-config.test.ts');
  const output = f.plan(); const list = commands(output); assert.equal(list.length, 1);
  assert.equal(list[0][0], 'sh'); assert.match(list[0][2], /^cd apps\/web && npx vitest related --run /);
  assert.ok(list[0][2].includes("test/ci-config.test.ts")); assert.ok(!list[0][2].includes('apps/web/app/'));
  assert.ok(output.indexOf('prep\tBuild core') >= 0 && output.indexOf('prep\tBuild core') < output.indexOf('candidate\t'));
  // Execute the shell wrapper with a harmless npx to verify argv and cwd.
  const bin = resolve(f.dir, 'bin'); mkdirSync(bin);
  writeFileSync(resolve(bin, 'npx'), '#!/bin/sh\npwd\nprintf "%s\\n" "$@"\n', { mode: 0o755 });
  const r = spawnSync(list[0][0], list[0].slice(1), { cwd: f.root, env: { ...cleanEnv, PATH: `${bin}:${cleanEnv.PATH}` }, encoding: 'utf8' }); ok(r);
  assert.match(r.stdout, /\/apps\/web\nvitest\nrelated\n--run\n/); assert.ok(r.stdout.includes("app/with space '汉字.ts\n"));
});
test('04 partial success never publishes Rust/Web whole-step passes; Desktop remains full', t => {
  const f = fixture(t, true); f.change('crates/autome-cli/tests/cli.rs'); f.put('apps/desktop/probe.js', '// test');
  f.put('apps/web/app/probe.ts', 'export {};');
  const r = f.run([], { AUTOME_GATE_REPORT: f.report }); ok(r);
  const report = readFileSync(f.report, 'utf8'); assert.match(report, /partial\t-\t.*cargo/);
  assert.match(report, /partial\t-\t.*vitest related/);
  assert.match(report, /ran\t[a-f0-9]+\tDesktop\n/); assert.doesNotMatch(report, /ran\t[^\n]*\t(?:Rust test|Web test)\n/);
  assert.ok(r.stdout.indexOf('BUILD') >= 0 && r.stdout.indexOf('BUILD') < r.stdout.indexOf('DESKTOP'));
});
test('05 budget checked before each candidate, running command completes and remainder is not_run', t => {
  const f = fixture(t, true); f.change('crates/autome-cli/tests/cli.rs'); f.put('crates/autome-cli/tests/z.rs', '// z');
  const r = f.run([], { CI_QUICK_BUDGET: '1', QUICK_SLEEP: '1', AUTOME_GATE_REPORT: f.report }); ok(r);
  assert.equal((r.stdout.match(/^CANDIDATE /gm) ?? []).length, 1); assert.match(r.stdout, /未跑.*--test.*z.*超出快检预算 1s/);
  const report = readFileSync(f.report, 'utf8'); assert.match(report, /partial\t/); assert.match(report, /not_run\t-\t.*z/);
});
test('06 failed related test names case, exits 1 and is never recorded as passing', t => {
  const f = fixture(t, true); f.change('crates/autome-cli/tests/cli.rs');
  const r = f.run([], { QUICK_FAIL: '1', AUTOME_GATE_REPORT: f.report }); assert.equal(r.status, 1, r.stderr);
  assert.match(r.stdout, /失败用例[\s\S]*changed_case/); assert.match(readFileSync(f.report, 'utf8'), /failed\t-\t.*cargo/);
});
test('07 absent baseline still runs static checks and reports omission; base and --since fallback', t => {
  const f = fixture(t, true); f.change('crates/autome-cli/tests/cli.rs');
  const r = f.run([], { AUTOME_ROUND_START: '', AUTOME_BASE: '', AUTOME_GATE_REPORT: f.report }); ok(r);
  assert.match(r.stdout, /未提供本轮起点/); assert.doesNotMatch(r.stdout, /CANDIDATE/);
  assert.equal((readFileSync(f.report, 'utf8').match(/^ran\t/gm) ?? []).length, 4);
  assert.equal(commands(f.plan({ AUTOME_ROUND_START: '', AUTOME_BASE: f.base })).length, 1);
  assert.equal(commands(f.plan({ AUTOME_ROUND_START: '', AUTOME_BASE: '' }, ['--plan', '--since', f.base])).length, 1);
  ok(f.run(['--plan', '--since', 'invalid'], { AUTOME_BASE: 'invalid' })); // round start wins
});
test('08 end_to_end body-only changes select names; helper/deletion/boundary changes omit whole suite', t => {
  const f = fixture(t);
  const path = 'crates/automed/tests/end_to_end.rs';
  const initial = 'fn helper() {\n    let _ = 1;\n}\n#[test]\nfn first() {\n    assert!(true);\n}\n#[test]\nfn second() {\n    assert!(true);\n}\n';
  f.put(path, initial); f.git(['add', '-A']); f.git(['-c', 'core.hooksPath=/dev/null', '-c', 'user.name=T', '-c', 'user.email=t@t', 'commit', '-qm', 'test baseline']);
  const env = { AUTOME_ROUND_START: f.git(['rev-parse', 'HEAD']) };
  f.put(path, initial.replaceAll('assert!(true)', 'assert!(false)'));
  assert.deepEqual(commands(f.plan(env))[0].slice(-3), ['--', 'first', 'second']);
  for (const text of [initial.replace('    assert!(true);\n}', '}'), initial.replace('fn first() {\n', 'fn first() {\n    let _ = 3;\n')]) {
    f.put(path, text); assert.deepEqual(commands(f.plan(env))[0].slice(-2), ['--', 'first']);
  }
  for (const text of [initial.replace('let _ = 1', 'let _ = 2'), initial.replace('fn first()', 'fn renamed()'), initial.replace('#[test]\nfn first() {\n    assert!(true);\n}\n', '')]) {
    f.put(path, text); const out = f.plan(env); assert.equal(commands(out).length, 0); assert.match(out, /未跑.*end_to_end/);
  }
  const embedded = 'const TEXT: &str = r#"\n#[test]\nfn fake() {\n    old_text\n}\n"#;\n' + initial;
  f.put(path, embedded); f.git(['add', '-A']); f.git(['-c', 'core.hooksPath=/dev/null', '-c', 'user.name=T', '-c', 'user.email=t@t', 'commit', '-qm', 'raw string baseline']);
  f.put(path, embedded.replace('old_text', 'new_text'));
  const out = f.plan({ AUTOME_ROUND_START: f.git(['rev-parse', 'HEAD']) });
  assert.equal(commands(out).length, 0); assert.match(out, /未跑.*end_to_end/);
  rmSync(resolve(f.root, path));
  assert.match(f.plan(env), /未跑.*end_to_end/);
});
test('09 Web command cwd loads real config and related actually runs changed tests', t => {
  const f = fixture(t); f.change('apps/web/test/ci-config.test.ts');
  const command = commands(f.plan())[0]; assert.match(command[2], /^cd apps\/web && /);
  const r = spawnSync(process.execPath, ['--input-type=module', '-e', `import { resolveConfig } from 'vitest/node'; const { test: c } = await resolveConfig(); console.log(JSON.stringify({ testTimeout: c.testTimeout, hookTimeout: c.hookTimeout, fileParallelism: c.fileParallelism }));`], { cwd: resolve(source, 'apps/web'), env: cleanEnv, encoding: 'utf8' }); ok(r);
  assert.deepEqual(JSON.parse(r.stdout.trim()), { testTimeout: 60000, hookTimeout: 60000, fileParallelism: false });
  const related = spawnSync(command[0], command.slice(1), { cwd: source, env: { ...cleanEnv, NO_COLOR: '1' }, encoding: 'utf8', timeout: 30_000 }); ok(related);
  assert.match(related.stdout, /Tests\s+9 passed \(9\)/);
  assert.doesNotMatch(related.stdout, /No (?:changed|test) files/);
});
test('10 metadata selects CLI bins and domain dependants; all real targets compile with --no-run', t => {
  const f = fixture(t); f.change('crates/autome-cli/src/main.rs');
  assert.deepEqual(commands(f.plan()), [['cargo', 'test', '-p', 'autome-cli', '--bins', '--no-fail-fast']]);
  f.change('crates/autome-domain/src/lib.rs'); const list = commands(f.plan());
  assert.equal(list.length, 3);
  assert.ok(list.some(c => c.includes('automed') && c.includes('--lib')));
  assert.ok(list.some(c => c.includes('autome-domain') && c.includes('--lib')));
  for (const command of list) {
    const r = spawnSync(command[0], [...command.slice(1), '--no-run'], { cwd: source, env: cleanEnv, encoding: 'utf8', timeout: 180_000 }); ok(r);
  }
});
test('11 protocol, fixtures, build scripts, docs, deletions and priority/deduplication', t => {
  const f = fixture(t); f.put('docs/only.md', 'docs'); assert.equal(commands(f.plan()).length, 0);
  const manifest = readFileSync(resolve(f.root, 'crates/automed/Cargo.toml'), 'utf8');
  f.put('crates/automed/Cargo.toml', manifest + '\n# changed manifest\n');
  assert.deepEqual(commands(f.plan()), [['cargo', 'test', '-p', 'automed', '--lib', '--no-fail-fast']]);
  f.put('crates/automed/Cargo.toml', manifest);
  f.change('protocol/loop-protocol.md'); f.put('crates/autome-cli/tests/fixtures/new.txt', 'fixture');
  f.change('crates/automed/build.rs'); f.change('crates/automed/src/gate.rs');
  f.put('crates/autome-cli/tests/new.rs', '// target');
  const list = commands(f.plan()); assert.ok(list[0].includes('new'));
  assert.equal(list.filter(c => c.includes('automed') && c.includes('--lib') && !c.includes('protocol')).length, 1);
  assert.ok(list.some(c => c.includes('--tests'))); assert.ok(list.some(c => c.includes('prompt_rendering')));
  rmSync(resolve(f.root, 'crates/autome-cli/tests/cli.rs')); assert.ok(commands(f.plan()).some(c => c.includes('cli')));
});
test('12 zero/leading-zero budget, invalid arguments/revision, plan leaves report untouched', t => {
  const f = fixture(t, true); f.change('crates/autome-cli/tests/cli.rs');
  for (const budget of ['0', '000']) {
    const r = f.run([], { CI_QUICK_BUDGET: budget, AUTOME_GATE_REPORT: f.report }); ok(r);
    assert.doesNotMatch(r.stdout, /CANDIDATE/); assert.match(readFileSync(f.report, 'utf8'), /not_run\t/);
  }
  for (const budget of ['', '-1', 'no', '1.5', '9007199254740992']) assert.equal(f.run(['--plan'], { CI_QUICK_BUDGET: budget }).status, 2);
  ok(f.run(['--plan'], { CI_QUICK_BUDGET: '9007199254740991' }));
  assert.equal(f.run(['--since']).status, 2); assert.equal(f.run(['--unknown']).status, 2);
  assert.equal(f.run(['--plan'], { AUTOME_ROUND_START: 'missing-revision' }).status, 2);
  rmSync(f.report); ok(f.run(['--plan'], { AUTOME_GATE_REPORT: f.report })); assert.equal(existsSync(f.report), false);
  assert.equal(f.run([], { AUTOME_GATE_REPORT: f.dir }).status, 2);
  f.put('apps/web/app/tab\tname.ts', 'export {};');
  assert.equal(f.run(['--plan']).status, 2);
});
test('13 static steps share complete-gate keys and reuse; related subsets still run', t => {
  const f = fixture(t, true); f.change('crates/autome-cli/tests/cli.rs');
  f.put('apps/desktop/probe.js', '// desktop');
  const full = spawnSync('sh', ['scripts/ci-local.sh', '--plan'], { cwd: f.root, env: cleanEnv, encoding: 'utf8' }); ok(full);
  const records = new Map(full.stdout.split('\n').filter(l => l.startsWith('plan\t')).map(l => { const [, , key, name] = l.split('\t'); return [name, key]; }));
  for (const line of f.plan().split('\n').filter(l => l.startsWith('plan\t'))) {
    const [, , key, name] = line.split('\t'); assert.equal(key, records.get(name));
  }
  const passes = resolve(f.dir, 'passes'); writeFileSync(passes, [...records.values()].map(k => `${k}\tT\tgate\tabc\tnow\n`).join(''));
  const r = f.run([], { AUTOME_GATE_PASSES: passes, AUTOME_GATE_REPORT: f.report }); ok(r);
  const report = readFileSync(f.report, 'utf8'); assert.equal((report.match(/^reused\t/gm) ?? []).length, 5); assert.match(report, /partial\t/);
  assert.doesNotMatch(r.stdout, /^BUILD$/m);
});
