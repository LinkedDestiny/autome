import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, readFileSync, existsSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

for (const version of [true, false]) test(`pidfile wrapper ${version ? 'version probe preserves existing pid' : 'kernel records exec pid and forwards arguments'}`, t => {
  const home = mkdtempSync(join(tmpdir(), 'web-pidfile-'));
  t.after(() => rmSync(home, { recursive: true, force: true }));
  const binary = join(home, 'kernel');
  writeFileSync(binary, '#!/bin/sh\nprintf "%s\\n" "$$" "$@"\n', { mode: 0o755 });
  const pidfile = join(home, 'automed.pid');
  const args = version ? ['--version'] : ['argument with spaces', ''];
  const run = () => spawnSync('sh', [resolve('apps/web/test/fixtures/automed-pidfile.sh'), ...args], {
    env: { ...process.env, AUTOME_HOME: home, W3_REAL_AUTOMED: binary }, encoding: 'utf8',
  });
  if (version) {
    const first = run(); assert.equal(first.status, 0); assert.equal(existsSync(pidfile), false);
    writeFileSync(pidfile, '12345\n');
  }
  const result = run(); assert.equal(result.status, 0, result.stderr);
  const [pid, ...forwarded] = result.stdout.trimEnd().split('\n');
  if (version) { assert.equal(readFileSync(pidfile, 'utf8'), '12345\n'); assert.deepEqual(forwarded, args); }
  else { assert.equal(readFileSync(pidfile, 'utf8'), `${pid}\n`); assert.equal(result.stdout, `${pid}\nargument with spaces\n\n`); }
});
