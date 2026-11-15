import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';
import { tmpdir } from 'node:os';

const root = resolve(import.meta.dirname, '../..');
const quote = value => `'${value.replaceAll("'", "'\\''")}'`;
function sandbox(t) {
  const dir = mkdtempSync(resolve(tmpdir(), 'ci-summary-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  return dir;
}
function run(dir, commands) {
  const result = spawnSync('sh', ['-c', `set -u
root=${quote(root)}
. "$root/scripts/ci/lib.sh"
${commands}
summary
`], { cwd: dir, encoding: 'utf8', timeout: 60_000,
    env: { ...process.env, AUTOME_HOME: resolve(dir, 'home'), AUTOMED_DB_PATH: resolve(dir, 'state.sqlite3'), CARGO_TARGET_DIR: resolve(dir, 'target'), CARGO_TERM_COLOR: 'never' } });
  assert.ifError(result.error);
  return { status: result.status, log: result.stdout, summary: result.stdout.split('==> 汇总\n')[1] };
}
function cargoLog(names, target = 'sample') {
  return `     Running tests/${target}.rs (target/debug/deps/${target}-abc123)\n\nfailures:\n\n${names.map(name => `    ${name}`).join('\n')}\n\ntest result: FAILED\n`;
}
function fixture(dir, contents) {
  writeFileSync(resolve(dir, 'output'), contents);
  return "sh -c 'cat output; exit 7'";
}
function tail(log) {
  const lines = spawnSync('tail', ['-n', '60'], { input: log }).stdout;
  const bytes = spawnSync('tail', ['-c', '6000'], { input: lines }).stdout;
  let start = 0;
  while ((bytes[start] & 0xc0) === 0x80) start++;
  return new TextDecoder('utf-8', { fatal: true }).decode(bytes.subarray(start));
}

test('Doc-tests failures belong to their crate, not the preceding unit target', t => {
  const dir = sandbox(t);
  const log = '     Running unittests src/lib.rs (target/debug/deps/previous-abc123)\ntest result: ok\n   Doc-tests autome_domain\n\nfailures:\n\n    src/lib.rs - example (line 1)\n\ntest result: FAILED\n';
  const result = run(dir, `run 'Rust' --cases cargo ${fixture(dir, log)}`);
  assert.equal(result.status, 1);
  assert.match(result.summary, /Doc-tests autome_domain: src\/lib.rs - example \(line 1\)/);
  assert.doesNotMatch(result.summary, /previous \(lib\):/);
});

test('real cargo runs both failing targets and reports both names', t => {
  const dir = sandbox(t);
  mkdirSync(resolve(dir, 'tests'));
  writeFileSync(resolve(dir, 'Cargo.toml'), '[package]\nname="ci-fixture"\nversion="0.0.0"\nedition="2021"\n[workspace]\n');
  for (const name of ['first', 'second']) writeFileSync(resolve(dir, `tests/${name}.rs`), `#[test] fn ${name}_failure() { assert!(false); }\n`);
  const result = run(dir, "run 'Rust' --cases cargo cargo test --offline --no-fail-fast -- --nocapture");
  assert.equal(result.status, 1);
  for (const name of ['first', 'second']) assert.match(result.summary, new RegExp(`${name}: ${name}_failure`));
});

test('real Vitest output lists file and case, including a whole-file failure', t => {
  const dir = sandbox(t);
  // Captured from Vitest 5.1.0, two real failing test files; no Vitest install needed by this suite.
  const real = readFileSync(resolve(import.meta.dirname, 'fixtures/vitest-failures.txt'), 'utf8');
  const result = run(dir, `run 'Web test' --cases vitest ${fixture(dir, real + '\u001b[31m FAIL  broken.test.ts [ broken.test.ts ]\u001b[0m\n')}`);
  assert.equal(result.status, 1);
  for (const name of ['first', 'second']) assert.match(result.summary, new RegExp(`${name}.test.js > ${name} failure`));
  assert.match(result.summary, /broken.test.ts \[ broken.test.ts \]/);
});

test('40 ASCII cases list only 20 with exact omitted count and survive the tail', t => {
  const dir = sandbox(t);
  const result = run(dir, `run 'Rust' --cases cargo ${fixture(dir, cargoLog(Array.from({ length: 40 }, (_, i) => `failure_${i}`)))}`);
  assert.equal(result.summary.match(/^  - /gm).length, 20);
  assert.match(result.summary, /另有 20 个/);
  assert.match(tail(result.log), /失败用例 · Rust/);
  assert.match(tail(result.log), /sample: failure_19/);
  assert.match(tail(result.log), /有步骤未通过/);
  assert.ok(Buffer.byteLength(result.summary) <= 3000);
});

test('step lines remain byte-compatible and new lines never start with FAILED', t => {
  const dir = sandbox(t);
  const result = run(dir, `run 'Format' true\nrun 'Rust' --cases cargo ${fixture(dir, cargoLog(['error_case']))}`);
  assert.match(result.log, /==> 汇总\n/);
  assert.ok(result.summary.split('\n').includes('  ok      Format'));
  assert.ok(result.summary.split('\n').includes('  FAILED  Rust'));
  assert.deepEqual(result.summary.split('\n').filter(line => line.trimStart().startsWith('FAILED')), ['  FAILED  Rust']);
});

test('failure never stops later steps; empty, single and all-success runs exit zero', t => {
  const dir = sandbox(t);
  const failed = run(dir, "run 'early' false\nrun 'later' sh -c 'echo later-ran'");
  assert.equal(failed.status, 1);
  assert.match(failed.log, /later-ran/);
  assert.match(failed.summary, /执行 2 步，沿用 0 步，失败 1 步/);
  for (const commands of ['', "run 'only' true", "run 'one' true\nrun 'two' true"]) {
    const result = run(dir, commands);
    assert.equal(result.status, 0);
    assert.match(result.summary, /全部通过/);
    assert.doesNotMatch(result.summary, /失败用例/);
  }
});

test('unparsed failures and empty parsed output do not invent cases', t => {
  const dir = sandbox(t);
  const command = fixture(dir, cargoLog(['not_a_clippy_case']));
  const result = run(dir, `run 'Clippy' ${command}\nrun 'Empty' --cases cargo false\nrun 'Success' --cases cargo cat output`);
  assert.equal(result.status, 1);
  assert.doesNotMatch(result.summary, /失败用例/);
});

test('18 long multibyte names stay within global byte budget on character boundaries', t => {
  const dir = sandbox(t);
  const names = Array.from({ length: 18 }, (_, i) => `${String(i).padStart(4, '0')}${'汉'.repeat(156)}`);
  const result = run(dir, `run 'Rust' --cases cargo ${fixture(dir, cargoLog(names))}`);
  const lines = result.summary.split('\n').filter(line => line.startsWith('  - '));
  assert.ok(lines.length > 0 && lines.length < 18);
  for (const line of lines) {
    assert.ok(Buffer.byteLength(line) <= 200);
    assert.ok(Array.from(line).length <= 160);
    assert.ok(line.endsWith('…'));
    assert.ok(!line.includes('\ufffd'));
  }
  assert.match(result.summary, new RegExp(`另有 ${18 - lines.length} 个`));
  assert.ok(Buffer.byteLength(result.summary) <= 3000);
  const ending = tail(result.log);
  assert.match(ending, /失败用例 · Rust/);
  assert.ok(ending.includes(lines.at(-1)));
  assert.match(ending, /有步骤未通过/);
});
