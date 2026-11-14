import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';

test('all repository paths are claimed by steps, globals or explicit no-gate inputs', () => {
  const result = spawnSync('sh', ['scripts/ci-local.sh', '--check-claims'], {
    cwd: resolve(import.meta.dirname, '../..'), encoding: 'utf8', timeout: 120_000,
  });
  assert.equal(result.status, 0, result.stdout + result.stderr);
});
