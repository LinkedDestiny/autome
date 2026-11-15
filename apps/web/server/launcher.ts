import { driverSummary } from './drivers.ts';
import { service } from './service.ts';
import { existsSync } from 'node:fs';
import { mkdir, open, readFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { spawn } from 'node:child_process';
import { setTimeout as delay } from 'node:timers/promises';
import { resolveAutomeHome } from './home.ts';
import { resolvePort } from './config.ts';
import { acquireLaunchLock, judgeInstance, readInstance, removeInstance, requestJson, pidAlive, unresponsive, verifiedCommand, type Instance, type InstanceStatus } from './instance.ts';
import { openPanel } from './opener.ts';

const root = resolve(import.meta.dirname, '..');
function instanceUrl(value: Instance) { return `http://127.0.0.1:${value.port}`; }
export function ports(explicit: string | undefined, fallback = process.env.AUTOME_WEB_DEFAULT_PORT) {
  if (explicit !== undefined) return [resolvePort(explicit)];
  const first = resolvePort(fallback);
  return first === 0 ? [0] : [...Array.from({ length: Math.min(10, 65536 - first) }, (_, i) => first + i), 0];
}
async function current(file: string) {
  const value = await readInstance(file);
  return value ? { value, verdict: await judgeInstance(value) } : null;
}
async function existing(file: string) {
  const found = await current(file);
  if (!found || found.verdict === 'stale') return false;
  console.log(found.verdict === 'live' ? `已有面板在运行：${instanceUrl(found.value)}` : unresponsive(found.value));
  return true;
}
async function serve(file: string, explicit: string | undefined) {
  const unlock = await acquireLaunchLock(join(resolveAutomeHome(process.env), 'web/launch.lock'), { inspect: () => existing(file) });
  if (!unlock) return 0;
  try {
    if (await existing(file)) return 0;
    if (!existsSync(join(root, 'build/server/index.js'))) throw new Error('先 npm run build（在 apps/web 中执行）');
    const old = await readInstance(file);
    if (old) await removeInstance(file, old.instance);
    const { runFromEnv } = await import('./main.ts');
    for (const port of ports(explicit)) {
      try { await runFromEnv({ port, mode: 'production', instanceFile: true }); return 0; }
      catch (error) {
        const code = (error as NodeJS.ErrnoException).code;
        if (code === 'EADDRINUSE') {
          if (explicit !== undefined) throw new Error(`端口 ${port} 已被占用`, { cause: error });
          continue;
        }
        if (code === 'EEXIST') {
          if (await existing(file)) return 0;
          const stale = await readInstance(file);
          if (stale) await removeInstance(file, stale.instance);
          await runFromEnv({ port, mode: 'production', instanceFile: true }); return 0;
        }
        throw error;
      }
    }
    throw new Error('无法找到可用端口');
  } finally { await unlock(); }
}
async function ensure(file: string, explicit: string | undefined) {
  const found = await current(file);
  if (found?.verdict === 'unresponsive') throw new Error(unresponsive(found.value));
  if (found?.verdict === 'live') return { ...found.value, reused: true };
  const log = join(resolveAutomeHome(process.env), 'web/logs/server.log');
  await mkdir(join(resolveAutomeHome(process.env), 'web/logs'), { recursive: true });
  const fd = await open(log, 'a', 0o600);
  const child = spawn(process.execPath, [join(root, 'bin/autome-web.js'), 'serve', ...(explicit === undefined ? [] : ['--port', explicit])], { detached: true, stdio: ['ignore', fd.fd, fd.fd] });
  let failure: Error | undefined;
  child.on('error', error => { failure = error; });
  child.unref();
  await fd.close();
  const deadline = Date.now() + 20000;
  do {
    const active = await current(file);
    if (active?.verdict === 'live') return { ...active.value, reused: active.value.pid !== child.pid };
    if (active?.verdict === 'unresponsive') throw new Error(unresponsive(active.value));
    if (failure || (child.exitCode !== null && child.exitCode !== 0)) break;
    await delay(100);
  } while (Date.now() < deadline);
  throw new Error(`面板启动失败或超时：${failure?.message ?? ''}\n${(await readFile(log, 'utf8')).trim().split('\n').slice(-20).join('\n')}`);
}
async function status(file: string, json: boolean) {
  const found = await current(file);
  if (!found || found.verdict === 'stale') {
    if (found) await removeInstance(file, found.value.instance);
    console.log('面板没有在运行'); return 3;
  }
  const value = found.value;
  if (found.verdict === 'unresponsive') {
    console.log(json ? JSON.stringify({ unresponsive: true, pid: value.pid, port: value.port, instance: value.instance, url: instanceUrl(value) }) : `面板服务无响应（pid ${value.pid}，端口 ${value.port}，启动于 ${value.started_at}）`); return 4;
  }
  const info = await instanceInfo(value);
  console.log(json ? JSON.stringify(info) : `${instanceUrl(value)}\npid ${value.pid}，启动于 ${value.started_at}\n内核：${info.kernel.path}，来源：${info.kernel.source}，构建：${info.kernel.build_id}${info.kernel.may_advance ? '' : '（只读：内核比账本旧）'}\n门禁：${info.gates.map(gate => `${gate.task_id} ${gate.node}`).join('、') || '无'}\n${driverSummary(info.drivers, info.other_driver)}`);
  return 0;
}
async function instanceInfo(value: Instance) {
  const info = await requestJson(value.port, '/api/instance') as InstanceStatus;
  if (info.instance !== value.instance || info.pid !== value.pid) throw new Error('面板实例已经改变，没有停止');
  return info;
}
export async function stop(file: string, { force = false, wait = 1800, instance }: { force?: boolean; wait?: number; instance?: string } = {}) {
  const found = await current(file);
  if (!found) { console.log('面板没有在运行'); return 0; }
  const value = found.value;
  if (instance !== undefined && value.instance !== instance) { console.log('当前运行的不是指定的那个面板服务，已保留'); return 0; }
  if (found.verdict === 'stale') { await removeInstance(file, value.instance); console.log('面板没有在运行，已清理过期文件'); return 0; }
  if (found.verdict === 'unresponsive') {
    if (!force || !await verifiedCommand(value.pid)) { console.log(`面板服务（pid ${value.pid}）没有响应，没有停止。确认要停掉它就加 --force`); return 1; }
  } else {
    let info = await instanceInfo(value);
    const deadline = Date.now() + wait * 1000;
    if (info.gates.length && !force) console.log(`${info.gates.map(g => `${g.task_id}（${g.node}）`).join('、')} 正在跑宿主门禁。门禁在独立进程里，停掉面板不会杀掉它，但在下一个驱动接手前没人收取结果。正在等它结束…（加 --force 立即停止）`);
    while (info.gates.length && !force) {
      if (Date.now() >= deadline) { console.log('门禁还在跑，没有停止；加 --force 立即停止'); return 1; }
      await delay(Math.min(2000, Math.max(1, deadline - Date.now())));
      info = await instanceInfo(value);
    }
    if (force && info.gates.length) console.log(`门禁 ${info.gates.map(g => g.task_id).join('、')} 继续在后台运行，下一个驱动（autome run 或再次 autome web）会收取结果`);
  }
  const matches = async () => { const now = await readInstance(file); return now?.instance === value.instance && now.pid === value.pid; };
  if (!await matches()) return 0;
  const verdict = await judgeInstance(value);
  if (verdict === 'stale') { await removeInstance(file, value.instance); return 0; }
  if (verdict === 'unresponsive' && (!force || !await verifiedCommand(value.pid))) return 1;
  try { process.kill(value.pid, 'SIGTERM'); } catch (error) { if ((error as NodeJS.ErrnoException).code !== 'ESRCH') throw error; }
  const deadline = Date.now() + 15000;
  while (pidAlive(value.pid) && Date.now() < deadline) await delay(100);
  if (pidAlive(value.pid) && await matches()) process.kill(value.pid, 'SIGKILL');
  await removeInstance(file, value.instance);
  if (existsSync(join(process.env.AUTOME_LAUNCH_AGENTS_DIR ?? join(process.env.HOME ?? '', 'Library/LaunchAgents'), 'com.autome.web.plist'))) console.log('launchd 会在下次登录时再起它');
  console.log('面板已停止'); return 0;
}
export async function launch(args: string[]) {
  const action = args[0]?.startsWith('-') ? 'open' : args.shift() ?? 'open';
  process.title = `autome-web ${resolveAutomeHome(process.env)} ${action}`;
  if (action === 'service') {
    if (args.length !== 1) throw new Error('用法：service install|uninstall');
    return service(args[0]);
  }
  const allowed: Record<string, string[]> = { open: ['--port'], start: ['--port', '--json'], serve: ['--port'], status: ['--json'], stop: ['--force', '--wait', '--instance'] };
  if (!allowed[action]) throw new Error(`未知面板命令：${action}`);
  const options = new Map<string, string>();
  while (args.length) {
    const key = args.shift()!;
    if (!allowed[action].includes(key) || options.has(key)) throw new Error(`无效参数：${key}`);
    const value = ['--port', '--wait', '--instance'].includes(key) ? args.shift() : 'true';
    if (!value || value.startsWith('--')) throw new Error(`参数 ${key} 缺少值`);
    options.set(key, value);
  }
  const explicit = options.get('--port') ?? process.env.AUTOME_WEB_PORT;
  if (['serve', 'start', 'open'].includes(action)) ports(explicit);
  const file = join(resolveAutomeHome(process.env), 'web/server.json');
  if (action === 'serve') return serve(file, explicit);
  if (action === 'status') return status(file, options.has('--json'));
  if (action === 'stop') {
    const seconds = options.get('--wait') ?? '1800';
    if (!/^\d+$/.test(seconds) || !Number.isSafeInteger(Number(seconds))) throw new Error('--wait 必须是非负整数秒');
    return stop(file, { force: options.has('--force'), wait: Number(seconds), instance: options.get('--instance') });
  }
  const value = await ensure(file, explicit);
  const url = instanceUrl(value);
  console.log(options.has('--json') ? JSON.stringify({ url, pid: value.pid, instance: value.instance, reused: value.reused }) : url);
  if (action === 'open') await openPanel(url);
  return 0;
}
