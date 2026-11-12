import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { test } from 'node:test';

test('Web typecheck accepts the checked-in regression scripts', () => {
  const result = spawnSync('npm', ['run', 'typecheck'], {
    cwd: new URL('../', import.meta.url),
    encoding: 'utf8',
    timeout: 120_000,
  });
  assert.ifError(result.error);
  assert.equal(result.signal, null, 'typecheck must finish without a signal');
  assert.equal(result.status, 0,
    `Web typecheck failed:\n${result.stdout}\n${result.stderr}`);
});
