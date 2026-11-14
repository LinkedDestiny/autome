import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';

// Exercise the real runner: resolveConfig alone never checks worker conflicts.
// These existing files cover all three projects without starting a TCP listener.
test('Web runner starts unit, kernel and exclusive together in one invocation', () => {
  const result = spawnSync(process.execPath, [
    'node_modules/vitest/vitest.mjs', 'run', '--reporter=verbose',
    'test/paths.test.ts', 'test/ci-config.test.ts', 'test/load-context.test.ts',
  ], { cwd: resolve('apps/web'), encoding: 'utf8', timeout: 120_000 });
  const output = result.stdout + result.stderr;
  assert.equal(result.status, 0, output);
  for (const project of ['unit', 'kernel', 'exclusive']) {
    assert.ok(output.includes(`|${project}|`), output);
  }
  assert.match(output, /Test Files\s+3 passed/);
  assert.doesNotMatch(output, /no tests|different 'maxWorkers'/);
});
