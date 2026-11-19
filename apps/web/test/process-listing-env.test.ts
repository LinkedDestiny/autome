import { execFileSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { afterEach, expect, test } from 'vitest';
import { harness } from './fixtures/web-launcher.ts';

// Regression: the panel server sets process.title, which on macOS overwrites the
// original argv block in place. A title shorter than the old argv makes tools that
// read the raw block (`pgrep -fl`) run on into the environment strings behind it.
const active: ReturnType<typeof harness>[] = [];
afterEach(async () => { for (const h of active.splice(0)) await h.cleanup(); });

function listing(command: string, args: string[], pid: number) {
  const out = execFileSync(command, args, { encoding: 'utf8' });
  return out.split('\n').filter(line => new RegExp(`^\\s*${pid}\\s`).test(line) || (args.includes('-p') && line.trim() !== ''));
}

test('面板服务运行时，进程列表里看不到它的环境变量', async () => {
  const h = harness(); active.push(h);
  const sentinel = `leak-${randomUUID()}`;
  // The overwritten argv block runs on into the environment strings in key order,
  // so the sentinel goes first: it is the first value a leak exposes.
  const rest = { ...h.env };
  for (const key of Object.keys(h.env)) delete h.env[key];
  Object.assign(h.env, { AUTOME_LEAK_SENTINEL: sentinel }, rest);
  const started = await h.run(['start']);
  expect(started.code, started.stderr).toBe(0);
  const { pid } = await h.registration();

  const views = {
    'pgrep -fl autome-web': listing('pgrep', ['-fl', 'autome-web'], pid),
    'ps -axo pid,command': listing('ps', ['-axo', 'pid,command'], pid),
    'ps -o args= -p <pid>': listing('ps', ['-o', 'args=', '-p', String(pid)], pid),
  };
  // Reached the scenario: every view found the server's own line.
  for (const [name, lines] of Object.entries(views)) {
    expect(lines.length, `${name} 里找不到面板服务进程 ${pid}`).toBeGreaterThan(0);
    expect(lines.join('\n'), name).toContain('autome-web');
  }
  for (const [name, lines] of Object.entries(views)) {
    const hits = lines.filter(line => line.includes(sentinel));
    expect(hits.length, `${name}：${hits.length} 行含环境变量哨兵值`).toBe(0);
    // Any other variable of the launching shell counts too; report names only.
    const leaked = Object.entries(h.env).filter(([key, value]) => value && value.length >= 12 && /^[A-Z][A-Z0-9_]*$/.test(key) && lines.some(line => line.includes(`${key}=${value}`))).map(([key]) => key);
    expect(leaked, `${name} 里出现了这些环境变量的值`).toEqual([]);
  }

  const status = await h.run(['status']);
  expect(status.code, status.stdout).toBe(0);
  const stopped = await h.run(['stop']);
  expect(stopped.code, stopped.stderr).toBe(0);
  expect(h.servers()).toEqual([]);
});
