import { createHash } from 'node:crypto';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { afterEach, beforeEach, expect, test } from 'vitest';

const script = resolve('../..', 'scripts/prepare-web.sh');
let root: string;
let web: string;
let log: string;
let fingerprint: string;
beforeEach(() => {
  root = mkdtempSync(join(tmpdir(), 'prepare-web-'));
  web = join(root, '中文 web');
  log = join(root, 'calls');
  fingerprint = join(web, 'node_modules/.autome-lock.sha256');
  mkdirSync(web);
  mkdirSync(join(root, 'bin'));
  writeFileSync(join(web, 'package.json'), '{}');
  writeFileSync(join(web, 'package-lock.json'), '{}');
  writeFileSync(join(root, 'bin/npm'), `#!/bin/sh
printf '%s|%s\\n' "$PWD" "$*" >> "$CALLS"
if [ "$1" = ci ]; then
  [ "$FAIL_CI" = 0 ] || exit 23
  mkdir -p node_modules
else
  [ "$FAIL_BUILD" = 0 ] || exit 29
fi
`, { mode: 0o755 });
});
afterEach(() => rmSync(root, { recursive: true, force: true }));
function run(env: Record<string, string> = {}) {
  return spawnSync('sh', [script, web], {
    encoding: 'utf8', env: { ...process.env, PATH: `${join(root, 'bin')}:${process.env.PATH}`, CALLS: log, FAIL_CI: '0', FAIL_BUILD: '0', ...env },
  });
}
function calls() { return readFileSync(log, 'utf8').trim().split('\n'); }
function hash() { return createHash('sha256').update(readFileSync(join(web, 'package-lock.json'))).digest('hex') + '\n'; }
function clearCalls() { writeFileSync(log, ''); }

test('clean directory installs then builds in the web directory and records the lock hash', () => {
  expect(run().status).toBe(0);
  expect(calls()).toEqual([`${web}|ci`, `${web}|run build`]);
  expect(readFileSync(fingerprint, 'utf8')).toBe(hash());
});
test('matching fingerprint always rebuilds without installing', () => {
  mkdirSync(join(web, 'node_modules'));
  writeFileSync(fingerprint, hash());
  expect(run().status).toBe(0);
  expect(calls()).toEqual([`${web}|run build`]);
});
test('changed lock or missing fingerprint reinstalls and updates the hash', () => {
  mkdirSync(join(web, 'node_modules'));
  expect(run().status).toBe(0);
  expect(calls()).toEqual([`${web}|ci`, `${web}|run build`]);
  clearCalls();
  writeFileSync(join(web, 'package-lock.json'), '{"lockfileVersion":3}');
  expect(run().status).toBe(0);
  expect(calls()).toEqual([`${web}|ci`, `${web}|run build`]);
  expect(readFileSync(fingerprint, 'utf8')).toBe(hash());
});
test('install failure preserves the exit code without recording success or building', () => {
  expect(run({ FAIL_CI: '1' }).status).toBe(23);
  expect(calls()).toEqual([`${web}|ci`]);
  expect(existsSync(fingerprint)).toBe(false);
});
test('build failure preserves the dependency fingerprint and retries only build', () => {
  expect(run({ FAIL_BUILD: '1' }).status).toBe(29);
  expect(calls()).toEqual([`${web}|ci`, `${web}|run build`]);
  expect(readFileSync(fingerprint, 'utf8')).toBe(hash());
  clearCalls();
  expect(run().status).toBe(0);
  expect(calls()).toEqual([`${web}|run build`]);
});
