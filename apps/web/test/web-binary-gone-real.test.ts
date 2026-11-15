import { expect, test } from 'vitest';
import { spawn } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { leftoverProcesses, teardown } from './fixtures/process-leak.ts';
import { readInstance } from '../server/instance.ts';

test('删除内核副本目录后面板在 15 秒内以 0 退出、删除实例并停掉内核', async () => {
  leftoverProcesses('binary-gone-preflight');
  const root = mkdtempSync(join(tmpdir(), 'web-binary-gone-'));
  const home = join(root, 'home');
  const bin = join(root, 'bin');
  mkdirSync(bin);
  const binary = join(bin, 'automed');
  copyFileSync(resolve(import.meta.dirname, '../../../target/debug/automed'), binary);
  const entry = join(root, 'serve.mjs');
  writeFileSync(entry, `import { runFromEnv } from ${JSON.stringify(resolve(import.meta.dirname, '../server/main.ts'))};\nawait runFromEnv({mode: 'production', instanceFile: true, port: 0});\n`);
  const child = spawn(process.execPath, [entry], { env: { ...process.env, HOME: home, AUTOME_HOME: home, AUTOMED_BIN: binary }, stdio: ['ignore', 'pipe', 'pipe'] });
  let output = '';
  child.stdout!.on('data', chunk => { output += String(chunk); });
  child.stderr!.on('data', chunk => { output += String(chunk); });
  const file = join(home, 'web/server.json');
  try {
    await expect.poll(async () => {
      if (child.exitCode !== null) throw Error(output);
      return readInstance(file);
    }, { timeout: 15000 }).toBeTruthy();
    const tracked = leftoverProcesses(root).map(row => row.pid);
    expect(tracked).toContain(child.pid);
    expect(tracked.length).toBeGreaterThan(1);
    rmSync(bin, { recursive: true });
    await expect.poll(() => child.exitCode, { timeout: 15000 }).toBe(0);
    expect(child.signalCode).toBeNull();
    expect(output).toContain('面板服务退出');
    expect(existsSync(file)).toBe(false);
    expect(leftoverProcesses(root, tracked)).toEqual([]);
  } finally {
    await teardown({ root });
  }
});
