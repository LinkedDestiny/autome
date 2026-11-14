import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';
import { stripVTControlCharacters } from 'node:util';

// Exercise the real runner: resolveConfig alone never checks worker conflicts.
// These existing files cover all three projects without starting a TCP listener.
for (const color of [false, true]) test(`Web runner starts unit, kernel and exclusive together in one invocation (color=${color})`, () => {
  const env = { ...process.env, FORCE_COLOR: color ? '1' : '0' };
  if (color) delete env.NO_COLOR;
  else env.NO_COLOR = '1';
  const result = spawnSync(process.execPath, [
    'node_modules/vitest/vitest.mjs', 'run', '--reporter=verbose',
    'test/paths.test.ts', 'test/ci-config.test.ts', 'test/load-context.test.ts',
  ], { cwd: resolve('apps/web'), env, encoding: 'utf8', timeout: 120_000 });
  const output = stripVTControlCharacters(result.stdout + result.stderr);
  assert.equal(result.status, 0, output);
  // Vitest uses pipe-delimited labels without color, background labels with it.
  for (const [project, file] of [['unit', 'paths'], ['kernel', 'ci-config'], ['exclusive', 'load-context']]) {
    assert.match(output, new RegExp(`(?:\\|${project}\\||\\b${project})\\s+test/${file}\\.test\\.ts`));
  }
  assert.match(output, /Test Files\s+3 passed/);
  assert.match(output, /Tests\s+31 passed \(31\)/);
  assert.doesNotMatch(output, /no tests|different 'maxWorkers'/);
});
