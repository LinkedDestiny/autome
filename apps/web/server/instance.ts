import { randomUUID } from 'node:crypto';
import { readFile, writeFile, mkdir, link, unlink } from 'node:fs/promises';
import { dirname } from 'node:path';
import { get } from 'node:http';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { setTimeout as delay } from 'node:timers/promises';

export type Instance = { port: number; pid: number; instance: string; url: string; started_at: string; web_root: string; kernel: { path: string; source: string; build_id: string } };
export type InstanceStatus = Instance & { drivers?: import('./drivers.ts').DriverStatus; web_version: string; kernel: Instance['kernel'] & { may_advance: boolean }; gates: { task_id: string; title: string; node: string }[]; other_driver: { skipped_last_minute: number; last_skipped_at: string | null } };
export function runningGates(dashboard: { running: { task: { id: string; title: string; state: { node?: string } } }[] }) {
  return dashboard.running.map(entry => entry.task)
    .filter(task => ['gate', 'fix_gate', 'merge_gate', 'repro_check', 'main_check'].includes(task.state.node ?? ''))
    .map(task => ({ task_id: task.id, title: task.title, node: task.state.node! }));
}
export type Verdict = 'live' | 'stale' | 'unresponsive';
export function pidAlive(pid: number) {
  try { process.kill(pid, 0); return true; } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ESRCH') return false;
    throw error;
  }
}
export async function readInstance(file: string): Promise<Instance | null> {
  try {
    const value = JSON.parse(await readFile(file, 'utf8')) as Instance;
    if (!Number.isInteger(value.pid) || value.pid <= 1 || !Number.isInteger(value.port) || value.port < 1 || value.port > 65535 || typeof value.instance !== 'string' || !value.instance) throw new Error('面板登记文件无效，请检查 server.json');
    return value;
  } catch (error) { if ((error as NodeJS.ErrnoException).code === 'ENOENT') return null; throw error; }
}
export async function removeInstance(file: string, instance: string) {
  if ((await readInstance(file))?.instance === instance) await unlink(file).catch(error => { if (error.code !== 'ENOENT') throw error; });
}
export async function writeInstance(file: string, value: Instance) {
  await mkdir(dirname(file), { recursive: true });
  const temp = `${file}.${process.pid}.${randomUUID()}.tmp`;
  try { await writeFile(temp, JSON.stringify(value) + '\n', { mode: 0o600, flag: 'wx' }); await link(temp, file); }
  finally { await unlink(temp).catch(error => { if (error.code !== 'ENOENT') throw error; }); }
}
export function requestJson(port: number, path: string, timeout = 2000): Promise<unknown> {
  return new Promise((resolve, reject) => {
    const request = get({ hostname: '127.0.0.1', port, path }, response => {
      let body = '';
      response.on('data', chunk => { body += chunk; if (body.length > 1024 * 1024) request.destroy(new Error('状态响应过大')); });
      response.on('end', () => {
        try { if (response.statusCode !== 200) throw new Error(`状态接口 HTTP ${response.statusCode}`); resolve(JSON.parse(body)); } catch (error) { reject(Object.assign(error instanceof Error ? error : new Error(String(error)), { code: 'EBADRESPONSE' })); }
      });
      response.on('error', reject);
    });
    const timer = setTimeout(() => request.destroy(Object.assign(new Error('面板服务没有响应'), { code: 'ETIMEDOUT' })), timeout);
    request.on('error', reject);
    request.on('close', () => clearTimeout(timer));
  });
}
async function commandLine(pid: number) {
  try { return (await promisify(execFile)('ps', ['-o', 'command=', '-p', String(pid)], { timeout: 2000 })).stdout; } catch { return null; }
}
export async function judgeInstance(value: Instance, { alive = pidAlive, health = (port: number) => requestJson(port, '/healthz'), command = commandLine } = {}): Promise<Verdict> {
  if (!alive(value.pid)) return 'stale';
  try {
    const result = await health(value.port) as { ok?: boolean; pid?: number; instance?: string } | null;
    return result?.ok === true && result.pid === value.pid && result.instance === value.instance ? 'live' : 'stale';
  } catch (error) {
    if (['ECONNREFUSED', 'EBADRESPONSE'].includes((error as NodeJS.ErrnoException).code ?? '')) return 'stale';
    const cmd = await command(value.pid);
    return cmd !== null && !cmd.includes('autome-web') ? 'stale' : 'unresponsive';
  }
}
export async function verifiedCommand(pid: number) { return (await commandLine(pid))?.includes('autome-web') === true; }
export function unresponsive(value: Instance) { return `面板服务（pid ${value.pid}，端口 ${value.port}）暂时没有响应，没有另起服务。等它恢复后重试，或运行 autome web stop --force`; }

// Reaping is serialized separately; an old lock is never stolen from a live owner.
export async function acquireLaunchLock(file: string, { timeout = 30000, alive = pidAlive, inspect = async () => false } = {}) {
  await mkdir(dirname(file), { recursive: true });
  const token = randomUUID();
  const deadline = Date.now() + timeout;
  const release = async () => {
    try { if (JSON.parse(await readFile(file, 'utf8')).token === token) await unlink(file); }
    catch (error) { if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error; }
  };
  let owner = 0;
  for (;;) {
    try { await writeFile(file, JSON.stringify({ pid: process.pid, token }), { flag: 'wx', mode: 0o600 }); return release; }
    catch (error) { if ((error as NodeJS.ErrnoException).code !== 'EEXIST') throw error; }
    if (await inspect()) return null;
    try {
      owner = JSON.parse(await readFile(file, 'utf8')).pid;
      if (Number.isInteger(owner) && owner > 1 && !alive(owner)) {
        const reap = file + '.reap';
        let held = false;
        try {
          await writeFile(reap, String(process.pid), { flag: 'wx', mode: 0o600 }); held = true;
          const current = JSON.parse(await readFile(file, 'utf8'));
          if (current.pid === owner && !alive(owner)) await unlink(file);
        } catch (error) { if (!['EEXIST', 'ENOENT'].includes((error as NodeJS.ErrnoException).code ?? '')) throw error; }
        finally { if (held) await unlink(reap); }
      }
    } catch (error) { if (!(error instanceof SyntaxError) && (error as NodeJS.ErrnoException).code !== 'ENOENT') throw error; }
    if (Date.now() >= deadline) throw new Error(`另一个面板正在启动（pid ${owner}）；若它卡住，结束该进程后重试`);
    await delay(200);
  }
}
