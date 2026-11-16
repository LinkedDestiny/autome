// M-06 audit regression: related branch always false, rebuild, run BL-01.
// Run separately from the normal suite: node scripts/test-related-mutation.mjs
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { readFileSync, writeFileSync, rmSync } from 'node:fs';
import { resolve } from 'node:path';

const web = resolve(import.meta.dirname, '..');
const source = resolve(web, 'app/lib/live-routes.ts');
const reportPath = resolve(web, '../../.autome/output/playwright-gate/report.json');
const original = readFileSync(source, 'utf8');
const branch = "&& relatedSubjects.get(params[param] ?? '')?.has(event.subject) === true";
assert.equal(original.split(branch).length, 2, 'exactly one related branch must be mutated');
/** @param {string} command @param {string[]} args */
function run(command, args) {
  const result = spawnSync(command, args, { cwd: web, stdio: 'inherit' });
  if (result.error) throw result.error;
  assert.equal(result.signal, null, 'command must not be interrupted');
  return result.status;
}
try {
  writeFileSync(source, original.replace(branch, '&& false'));
  assert.equal(run('npm', ['run', 'build']), 0, 'mutant must build');
  for (let i = 1; i <= 5; i++) {
    rmSync(reportPath, { force: true });
    const status = run('npx', ['playwright', 'test', 'e2e/controls.spec.ts', '-g', 'BL-01']);
    const report = JSON.parse(readFileSync(reportPath, 'utf8'));
    assert.equal(status, 1, `run ${i}: mutant must fail`);
    assert.equal(report.complete, true, 'setup failure or truncation is not mutation evidence');
    assert.deepEqual(report.errors, [], 'non-test failures are not mutation evidence');
    assert.equal(report.cases.length, 1);
    const test = report.cases[0];
    assert.match(test.title, /BL-01/);
    assert.equal(test.status, 'failed');
    assert.ok(test.errors.some(/** @param {{ message?: string }} error */ error => /等你批准/.test(error.message ?? '')
      && /toContainText/.test(error.message ?? '')), 'must fail on stale dependency text');
    console.log(`related branch mutant rejected: ${i}/5`);
  }
} finally {
  writeFileSync(source, original);
  assert.equal(run('npm', ['run', 'build']), 0, 'restore production build');
}
