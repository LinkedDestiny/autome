import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, existsSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';

for (const state of ['missing', 'matching', 'changed', 'missing-stamp', 'install-fails']) {
  test(`dependency fingerprint: ${state}`, t => {
    const root = mkdtempSync(join(tmpdir(), 'ci install '));
    t.after(() => rmSync(root, { recursive: true, force: true }));
    mkdirSync(join(root, 'bin'));
    writeFileSync(join(root, 'package-lock.json'), '{}');
    const fingerprint = createHash('sha256').update('{}').digest('hex');
    const stamp = join(root, 'node_modules/.autome-lock.sha256');
    if (state !== 'missing') mkdirSync(join(root, 'node_modules'));
    if (['matching', 'changed', 'install-fails'].includes(state)) writeFileSync(stamp, state === 'matching' ? fingerprint + '\n' : 'old\n');
    writeFileSync(join(root, 'bin/npm'), '#!/bin/sh\ntest "$1" = ci || exit 99\nprintf called > called\n[ "$FAIL_INSTALL" = 0 ] || exit 37\nmkdir -p node_modules\n', { mode: 0o755 });
    const result = spawnSync('sh', [resolve('scripts/ci/install-node.sh'), root], {
      encoding: 'utf8', env: { ...process.env, PATH: `${join(root, 'bin')}:/usr/bin:/bin`, FAIL_INSTALL: state === 'install-fails' ? '1' : '0' },
    });
    assert.equal(result.status, state === 'install-fails' ? 37 : 0, result.stderr);
    assert.equal(existsSync(join(root, 'called')), state !== 'matching');
    assert.equal(readFileSync(stamp, 'utf8'), state === 'install-fails' ? 'old\n' : fingerprint + '\n');
  });
}
