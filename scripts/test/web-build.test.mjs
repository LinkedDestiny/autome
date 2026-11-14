import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { spawnSync } from 'node:child_process';

for (const fails of [false, true]) test(`shared Web build ${fails ? 'failure propagates and does not publish marker' : 'runs once, including inherited nested runner marker'}`, t => {
  const root = mkdtempSync(join(tmpdir(), 'web-build-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const log = join(root, 'calls');
  writeFileSync(join(root, 'npm'), '#!/bin/sh\necho build >> "$BUILD_CALLS"\nexit "$BUILD_EXIT"\n', { mode: 0o755 });
  const module = pathToFileURL(resolve('apps/web/test/fixtures/ensure-built.ts')).href;
  const script = `import assert from 'node:assert/strict'; import { spawnSync } from 'node:child_process'; import { ensureBuilt } from ${JSON.stringify(module)};
    ${fails ? "assert.throws(() => ensureBuilt()); assert.equal(process.env.AUTOME_WEB_TEST_BUILD, undefined);" : `ensureBuilt(); ensureBuilt(); assert.ok(process.env.AUTOME_WEB_TEST_BUILD); const child = spawnSync(process.execPath, ['--input-type=module', '-e', ${JSON.stringify(`import { ensureBuilt } from ${JSON.stringify(module)}; ensureBuilt();`)}], { env: process.env }); assert.equal(child.status, 0, child.stderr.toString());`}`;
  const env = { ...process.env, PATH: `${root}:${process.env.PATH}`, BUILD_CALLS: log, BUILD_EXIT: fails ? '23' : '0' };
  delete env.AUTOME_WEB_TEST_BUILD;
  const result = spawnSync(process.execPath, ['--input-type=module', '-e', script], { env, encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(readFileSync(log, 'utf8'), 'build\n');
});
