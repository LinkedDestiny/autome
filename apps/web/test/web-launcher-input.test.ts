import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { launch, ports } from '../server/launcher.ts';
import { readInstance, writeInstance, judgeInstance, runningGates } from '../server/instance.ts';
let home: string;
let title: string;
beforeEach(async () => { home = await mkdtemp(join(tmpdir(), 'web-input-')); title = process.title; vi.stubEnv('AUTOME_HOME', home); vi.stubEnv('AUTOME_WEB_DEFAULT_PORT', '7420'); vi.spyOn(console, 'log').mockImplementation(() => {}); });
afterEach(async () => { process.title = title; vi.unstubAllEnvs(); vi.restoreAllMocks(); await rm(home, { recursive: true, force: true }); });
test('端口输入域：0、前导零、最大值与缺省末端回退', () => {
  expect(ports('0')).toEqual([0]); expect(ports('00080')).toEqual([80]); expect(ports('65535')).toEqual([65535]);
  expect(ports(undefined, '65535')).toEqual([65535, 0]); expect(ports(undefined, '0')).toEqual([0]);
  expect(ports(undefined)).toEqual([7420, 7421, 7422, 7423, 7424, 7425, 7426, 7427, 7428, 7429, 0]);
  for (const input of ['', '-1', '65536', '1.5', 'NaN', ' 80']) expect(() => ports(input)).toThrow('端口');
});
test('非法命令、重复参数、空值、未知参数与非法等待时间在启动前拒绝', async () => {
  for (const args of [['unknown'], ['start', '--bad'], ['start', '--port'], ['stop', '--instance', ''], ['status', '--json', '--json'], ['stop', '--wait', '-1'], ['stop', '--wait', '1.1'], ['stop', '--wait', '9007199254740992']]) await expect(launch(args)).rejects.toThrow();
});
test('无登记：status 退出 3，stop 退出 0；零秒与前导零等待合法', async () => {
  expect(await launch(['status'])).toBe(3); expect(await launch(['stop', '--wait', '0'])).toBe(0); expect(await launch(['stop', '--wait', '0002'])).toBe(0);
});
test('读盘失败及损坏登记不被视为不存在', async () => {
  const file = join(home, 'server.json'); await mkdir(file); await expect(readInstance(file)).rejects.toThrow(); await rm(file, { recursive: true });
  for (const text of ['', '{}', '{"pid":0,"port":0,"instance":""}', '{"pid":-123,"port":80,"instance":"x"}']) { await writeFile(file, text); await expect(readInstance(file)).rejects.toThrow(); }
});
test('写盘失败不留下登记', async () => {
  const parent = join(home, 'file'); await writeFile(parent, 'not a directory');
  await expect(writeInstance(join(parent, 'server.json'), { pid: process.pid, port: 80, instance: 'id', url: 'http://127.0.0.1:80', started_at: '', web_root: '', kernel: { path: '', source: '', build_id: '' } })).rejects.toThrow();
});
test('有 HTTP 回包但健康格式无效属于陈旧，授权探测失败不得接管', async () => {
  const value = { pid: process.pid, port: 80, instance: 'id', url: '', started_at: '', web_root: '', kernel: { path: '', source: '', build_id: '' } };
  expect(await judgeInstance(value, { alive: () => true, health: async () => { throw Object.assign(Error(), { code: 'EBADRESPONSE' }); } })).toBe('stale');
  await expect(judgeInstance(value, { alive: () => { throw Object.assign(Error(), { code: 'EPERM' }); } })).rejects.toMatchObject({ code: 'EPERM' });
});

test('全部门禁节点从真实 dashboard 的 task 层提取，普通节点不算门禁', () => {
  const nodes = ['gate', 'fix_gate', 'merge_gate', 'repro_check', 'main_check', 'implement', 'await_merge'];
  const gates = runningGates({ running: nodes.map((node, i) => ({ task: { id: 'T-' + i, title: node, state: { node } } })) });
  expect(gates.map(gate => gate.node)).toEqual(nodes.slice(0, 5));
  expect(gates[0]).toEqual({ task_id: 'T-0', title: 'gate', node: 'gate' });
  expect(runningGates({ running: [] })).toEqual([]);
});
