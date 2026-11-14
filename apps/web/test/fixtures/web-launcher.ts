import { execFile, execFileSync, spawn, type ChildProcess } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, existsSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { promisify } from 'node:util';
import { setTimeout as delay } from 'node:timers/promises';
import { expect } from 'vitest';
import { createServer } from 'node:net';
import { readInstance, type Instance } from '../../server/instance.ts';
export const web = resolve(import.meta.dirname, '../..');
export const wt = resolve(web, '../..');
export const binary = resolve(wt, 'target/debug/automed');
export const cli = resolve(wt, 'target/debug/autome');
export const execute = promisify(execFile);
export function processes(root: string) {
  const rows = execFileSync('ps', ['-axo', 'pid=,ppid=,command='], { encoding: 'utf8' }).trim().split('\n').map(line => {
    const match = line.trim().match(/^(\d+)\s+(\d+)\s+(.*)$/)!;
    return { pid: Number(match[1]), parent: Number(match[2]), command: match[3] };
  });
  const owned = new Set(rows.filter(row => row.command.includes(root)).map(row => row.pid));
  let size = -1;
  while (size !== owned.size) { size = owned.size; for (const row of rows) if (owned.has(row.parent)) owned.add(row.pid); }
  return rows.filter(row => owned.has(row.pid));
}
export function harness() {
  processes('preflight-no-process'); // Fail before creating any child if ps is forbidden.
  const root = mkdtempSync(join(tmpdir(), 'web-launcher-'));
  const home = join(root, 'autome-home');
  const file = join(home, 'web/server.json');
  mkdirSync(root, { recursive: true });
  const wrapper = join(root, 'core');
  writeFileSync(wrapper, `#!/bin/sh\nif [ "$1" = --version ]; then exec "${cli}" --version; fi\nexec "${cli}" core "$@"\n`, { mode: 0o755 });
  const opener = join(root, 'opener');
  writeFileSync(opener, `#!/bin/sh\nprintf '%s\\n' "$1" >> "${root}/opened"\n`, { mode: 0o755 });
  const env: NodeJS.ProcessEnv = { ...process.env, AUTOME_HOME: home, AUTOMED_BIN: wrapper, AUTOME_WEB_DEFAULT_PORT: '0', AUTOME_WEB_OPENER: opener };
  delete env.NODE_ENV; delete env.AUTOME_WEB_PORT;
  const children: ChildProcess[] = [];
  const args = (args: string[]) => [join(root, 'autome-web.js'), ...args];
  async function run(command: string[], extra: NodeJS.ProcessEnv = {}) {
    try { const result = await execute(process.execPath, args(command), { env: { ...env, ...extra }, timeout: 45000 }); return { ...result, code: 0 }; }
    catch (error) { const result = error as Error & { code: number; stdout: string; stderr: string }; return { stdout: result.stdout, stderr: result.stderr, code: result.code }; }
  }
  function foreground() { const child = spawn(process.execPath, args(['serve']), { env, stdio: 'ignore' }); children.push(child); return child; }
  async function registration(): Promise<Instance> { await expect.poll(() => readInstance(file), { timeout: 15000 }).toBeTruthy(); return (await readInstance(file))!; }
  function servers() { return processes(root).filter(row => row.command.includes('autome-web') && row.command.includes('serve')); }
  async function cleanup() {
    if (existsSync(file)) { const value = await readInstance(file); if (value) { try { process.kill(value.pid, 'SIGCONT'); } catch { /* Already gone. */ } await run(['stop', '--force']); } }
    for (const child of children) if (child.exitCode === null && child.signalCode === null) child.kill('SIGKILL');
    for (const row of processes(root).reverse()) { try { process.kill(row.pid, 'SIGKILL'); } catch { /* Already gone. */ } }
    // cleanup also runs in afterAll, where expect.poll is not supported.
    const deadline = Date.now() + 3000;
    let remaining = processes(root);
    while (remaining.length && Date.now() < deadline) {
      await delay(50);
      remaining = processes(root);
    }
    expect(remaining).toEqual([]);
    rmSync(root, { recursive: true, force: true });
  }
  // A private path in the launcher's argv makes both serve and descendants observable.
  const entry = join(root, 'autome-web.js');
  writeFileSync(entry, `import ${JSON.stringify(join(web, 'bin/autome-web.js'))};\n`);
  return { root, home, file, env, run, foreground, registration, servers, cleanup, opened: () => readFileSync(join(root, 'opened'), 'utf8').trim().split('\n') };
}
export async function occupiedPort() {
  const server = createServer();
  await new Promise<void>((resolve, reject) => { server.once('error', reject); server.listen(0, '127.0.0.1', resolve); });
  const address = server.address();
  if (!address || typeof address === 'string') throw new Error('没有监听地址');
  return { port: address.port, close: () => new Promise<void>((resolve, reject) => server.close(error => error ? reject(error) : resolve())) };
}
