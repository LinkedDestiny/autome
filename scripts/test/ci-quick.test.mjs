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
    put('apps/web/scripts/quick-related.mjs', `import { writeFileSync } from 'node:fs';
writeFileSync(process.argv[2], "ci_partial 'vitest related fixture' npx vitest related\\n");`);
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
test('Rust integration tests build matching CLI and kernel before execution', t => {
  const f = fixture(t, true); f.change('crates/autome-cli/tests/cli.rs');
  const output = f.plan();
  assert.ok(output.indexOf('prep\tBuild core') >= 0 && output.indexOf('prep\tBuild core') < output.indexOf('candidate\t'));
  f.put('apps/web/scripts/e2e-catalog.mjs', "console.log('CATALOG');");
  const r = f.run([]); ok(r);
  assert.ok(r.stdout.indexOf('INSTALL') < r.stdout.indexOf('CATALOG'));
  assert.ok(r.stdout.includes('CATALOG'));
  assert.ok(r.stdout.indexOf('BUILD') >= 0 && r.stdout.indexOf('BUILD') < r.stdout.indexOf('CANDIDATE test'));
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
  const r = spawnSync(process.execPath, ['--input-type=module', '-e', `import { resolveConfig } from 'vitest/node'; const { test: c } = await resolveConfig(); console.log(JSON.stringify({ globalSetup: c.globalSetup.map(p => p.split("/").pop()), projects: c.projects.map(p => ({ name: p.test.name, testTimeout: p.test.testTimeout, hookTimeout: p.test.hookTimeout })) }));`], { cwd: resolve(source, 'apps/web'), env: cleanEnv, encoding: 'utf8' }); ok(r);
  assert.deepEqual(JSON.parse(r.stdout.trim()), { globalSetup: ['global-setup.ts'], projects: ['unit', 'kernel', 'exclusive'].map(name => ({ name, testTimeout: 60000, hookTimeout: 60000 })) });
  const related = spawnSync(command[0], command.slice(1), { cwd: source, env: { ...cleanEnv, NO_COLOR: '1' }, encoding: 'utf8', timeout: 30_000 }); ok(related);
  assert.match(related.stdout, /Tests\s+13 passed \(13\)/);
  assert.doesNotMatch(related.stdout, /No (?:changed|test) files/);
  const selected = resolve(f.dir, 'related.sh');
  ok(spawnSync(process.execPath, ['apps/web/scripts/quick-related.mjs', selected, 'test/ci-config.test.ts'], {
    cwd: source, env: cleanEnv, encoding: 'utf8', timeout: 30_000,
  }));
  const script = readFileSync(selected, 'utf8');
  assert.equal(script.trim().split('\n').length, 1);
  assert.match(script, /ci_quick_admit.*ci-config\.test\.ts/);
  assert.match(script, /ci_partial.*--cases vitest/);
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
  // No inputs change after computing the pass keys: the catalog was already present.
  const r = f.run([], { AUTOME_GATE_PASSES: passes, AUTOME_GATE_REPORT: f.report }); ok(r);
  assert.match(r.stdout, /未取得浏览器用例清单/); // no node_modules in this fixture; still runs when Web steps reuse
  const report = readFileSync(f.report, 'utf8'); assert.equal((report.match(/^reused\t/gm) ?? []).length, 5); assert.match(report, /partial\t/);
  // Reusing Desktop must not suppress the Rust integration target's fresh binaries.
  assert.equal((r.stdout.match(/^BUILD$/gm) ?? []).length, 1);
  assert.ok(r.stdout.indexOf('BUILD') < r.stdout.indexOf('CANDIDATE test'));
});

test('14 Web related files recheck budget and report the unstarted remainder', t => {
  const f = fixture(t, true); f.put('apps/web/app/probe.ts', 'export {};');
  const clock = resolve(f.dir, 'clock');
  writeFileSync(clock, '100\n');
  f.put('advance-clock.sh', 'echo FIRST_WEB\necho 102 > "$QUICK_TEST_CLOCK"\n');
  writeFileSync(resolve(f.dir, 'bin/date'), '#!/bin/sh\ncat "$QUICK_TEST_CLOCK"\n', { mode: 0o755 });
  f.put('apps/web/scripts/quick-related.mjs', `import { writeFileSync } from 'node:fs';
writeFileSync(process.argv[2], "if ci_quick_admit first-web; then ci_partial first-web sh advance-clock.sh; fi\\nif ci_quick_admit second-web; then ci_partial second-web sh -c 'echo SECOND_WEB'; fi\\n");`);
  const r = f.run([], { CI_QUICK_BUDGET: '1', QUICK_TEST_CLOCK: clock, AUTOME_GATE_REPORT: f.report }); ok(r);
  assert.match(r.stdout, /FIRST_WEB/);
  assert.match(r.stdout, /最后准入的步骤 first-web 从第 \d+ 秒开始、用时 \d+ 秒；之后的 1 步未跑/);
  assert.doesNotMatch(r.stdout, /SECOND_WEB/);
  assert.match(readFileSync(f.report, 'utf8'), /partial\t-\tfirst-web/);
  assert.match(readFileSync(f.report, 'utf8'), /not_run\t-\tsecond-web/);
});

test('15 quick summary explains overrun and excludes later browser work', t => {
  const f = fixture(t, true);
  const script = `. scripts/ci/lib.sh
ci_quick_start
ci_quick_budget=10
ci_quick_started=$(($(date +%s) - 12))
ci_quick_last_name='last-web'
ci_quick_last_start=9
ci_quick_last_duration=3
ci_quick_admit next-web || :
ci_quick_finish
ci_quick_started=0
summary`;
  const r = spawnSync('sh', ['-c', script], { cwd: f.root, env: { ...cleanEnv, root: f.root }, encoding: 'utf8' }); ok(r);
  assert.match(r.stdout, /快检相关阶段用时 1[23] 秒（准入预算 10 秒）；最后准入的步骤 last-web 从第 9 秒开始、用时 3 秒；之后的 1 步未跑/);
  const normal = spawnSync('sh', ['-c', '. scripts/ci/lib.sh; ci_quick_start; ci_quick_finish; summary'], { cwd: f.root, env: { ...cleanEnv, root: f.root }, encoding: 'utf8' }); ok(normal);
  assert.match(normal.stdout, /快检相关阶段用时 \d+ 秒（准入预算 120 秒）/);
  assert.doesNotMatch(normal.stdout, /最后准入/);
});

test('16 each discovered Web candidate contains exactly one test file', t => {
  const dir = mkdtempSync(resolve(tmpdir(), 'quick-files-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const selected = resolve(dir, 'related.sh');
  ok(spawnSync(process.execPath, ['apps/web/scripts/quick-related.mjs', selected, 'test/ci-config.test.ts', 'test/test-groups.test.ts'], { cwd: source, env: cleanEnv, encoding: 'utf8', timeout: 30_000 }));
  const lines = readFileSync(selected, 'utf8').trim().split('\n');
  assert.equal(lines.length, 2);
  for (const line of lines) {
    assert.match(line, /^if ci_quick_admit .*; then ci_partial .* --cases vitest .*; fi$/);
    const command = line.split(' --cases vitest ')[1];
    assert.equal((command.match(/test\/[\w.-]+\.test\.ts/g) ?? []).length, 1, command);
    assert.match(command, /vitest run/);
    assert.doesNotMatch(command, /vitest related/);
  }
});

test('17 shared automed test inputs never admit the whole end_to_end target', t => {
  const f = fixture(t);
  f.change('crates/automed/tests/golden/prompts/impl-before-merge-quick.md');
  const output = f.plan();
  const list = commands(output);
  assert.ok(list.some(c => c.includes('prompt_rendering')));
  assert.ok(list.every(c => !c.includes('--tests') && !c.includes('end_to_end')));
  assert.match(output, /未跑.*end_to_end/);
});
