import { afterEach, expect, test, vi } from 'vitest';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { DatabaseSync } from 'node:sqlite';
import { openWebStore } from '../server/store.ts';
import { createNotifier } from '../server/notifier.ts';
import type { LedgerEvent } from '../server/eventbus-protocol.ts';
const cleanups: (() => void)[] = [];
afterEach(() => { for (const clean of cleanups.splice(0).reverse()) clean(); });
const event = (seq: number, subject = `T-${seq}`): LedgerEvent => ({ seq, subject, kind: 'task.updated', payload: { state: { state: 'active', node: 'await_merge' }, trigger: 'gate_done' } });
function fixture(events: LedgerEvent[] = [], cursor?: number) {
  const home = mkdtempSync(join(tmpdir(), 'notifier-'));
  let store = openWebStore(home);
  const publish = vi.fn(), log = vi.fn();
  let latest = events.at(-1)?.seq ?? 0;
  const call = vi.fn(async (method: string, params: Record<string, unknown>): Promise<unknown> => method === 'events.since' ? {
    events: events.filter(e => e.seq > Number(params.after_seq)).slice(0, Number(params.limit ?? 200)), latest_seq: latest,
  } : method === 'task.get' ? { project: { id: 'p1' }, task: { title: '任务正文' } } : { project: { id: 'p', default_branch: 'dev', display_name: '', path: '/repo/name' } });
  if (cursor !== undefined) store.commit({ cursor: { name: 'notifier', seq: cursor } });
  const make = () => createNotifier({ store, call, publish, log, pageSize: 2 });
  let notifier = make();
  cleanups.push(() => { notifier.close(); store.close(); rmSync(home, { recursive: true, force: true }); });
  return { home, get store() { return store; }, get notifier() { return notifier; }, call, publish, log,
    setEvents(next: LedgerEvent[], last = next.at(-1)?.seq ?? 0) { events = next; latest = last; },
    restart() { notifier.close(); store.close(); store = openWebStore(home); notifier = make(); },
  };
}
test('首次启动从 latest_seq 开始且空账本可初始化', async () => {
  for (const events of [[], [event(1), event(2)]]) {
    const f = fixture(events); await f.notifier.pull();
    expect(f.call.mock.calls[0]).toEqual(['events.since', { after_seq: Number.MAX_SAFE_INTEGER }]);
    expect(f.store.getCursor('notifier')).toBe(events.length); expect(f.store.list()).toHaveLength(0);
  }
});
test('停服前已处理的重启后不再生成', async () => {
  const f = fixture([event(1)], 0); await f.notifier.pull(); f.restart(); await f.notifier.pull();
  expect(f.store.list()).toHaveLength(1); expect(f.publish).toHaveBeenCalledTimes(1);
});
test('停服期间按 seq 补齐且各推一次', async () => {
  const f = fixture([event(1)], 0); await f.notifier.pull(); f.restart(); f.setEvents([event(3), event(2)], 3); await f.notifier.pull();
  expect(f.store.list().map(n => n.event_seq)).toEqual([3, 2, 1]); expect(f.publish.mock.calls.map(([m]) => m.task_id)).toEqual(['T-1', 'T-2', 'T-3']);
});
test('通知已写、游标未推进的人工状态重启不重不推', async () => {
  const f = fixture([event(1)], 0); await f.notifier.pull(); f.store.commit({ cursor: { name: 'notifier', seq: 0 } });
  f.restart(); await f.notifier.pull(); expect(f.store.list()).toHaveLength(1); expect(f.publish).toHaveBeenCalledTimes(1); expect(f.store.getCursor('notifier')).toBe(1);
});
test('注入游标写失败，事务回滚，重启补上且只一条', async () => {
  const f = fixture([event(1)], 0); const db = new DatabaseSync(join(f.home, 'state/web.sqlite3'));
  db.exec("CREATE TRIGGER fail_cursor BEFORE UPDATE ON event_cursor BEGIN SELECT RAISE(ABORT, '写盘失败'); END");
  await f.notifier.pull(); expect(f.store.list()).toHaveLength(0); expect(f.store.getCursor('notifier')).toBe(0); expect(f.publish).not.toHaveBeenCalled();
  db.exec('DROP TRIGGER fail_cursor'); db.close(); f.restart(); await f.notifier.pull(); expect(f.store.list()).toHaveLength(1); expect(f.publish).toHaveBeenCalledTimes(1);
});
test('换账本重置序号、状态记忆和补齐缓存', async () => {
  const f = fixture([event(10, 'T-1')], 0); await f.notifier.pull(); f.setEvents([], 0); await f.notifier.pull();
  expect(f.store.getCursor('notifier')).toBe(0); expect(f.store.list()[0].event_seq).toBeNull();
  expect(f.log).toHaveBeenCalledExactlyOnceWith('[通知] 账本已更换（最新序号 0 小于游标 10），从 0 继续');
  f.setEvents([event(1, 'T-1')]); const original = f.call.getMockImplementation()!;
  f.call.mockImplementation((method, params) => method === 'task.get' ? Promise.resolve({ project: { id: 'p2' }, task: { title: '新任务' } }) : original(method, params));
  await f.notifier.pull(); expect(f.store.list()[0]).toMatchObject({ project_id: 'p2', body: '新任务', event_seq: 1 }); expect(f.store.list()).toHaveLength(2);
});
test('补齐无 code 的离线/超时错误保留游标，恢复后同一事件仍生成通知', async () => {
  for (const error of [new Error('内核未连接'), Object.assign(new Error('超时'), { timeout: true })]) {
    const f = fixture([{ seq: 1, kind: 'task.created', subject: 'T-2', payload: {} }, event(2)], 0);
    const original = f.call.getMockImplementation()!; let failed = true;
    f.call.mockImplementation((method, params) => method === 'task.get' && failed ? Promise.reject(error) : original(method, params));
    await f.notifier.pull(); expect(f.store.getCursor('notifier')).toBe(1); expect(f.store.list()).toHaveLength(0);
    failed = false; await f.notifier.pull(); await f.notifier.pull();
    expect(f.store.list()).toHaveLength(1); expect(f.publish).toHaveBeenCalledTimes(1); expect(f.store.getCursor('notifier')).toBe(2);
  }
});
test('带 code 的拒绝仍生成通知、project_id 与 body 为 NULL', async () => {
  const f = fixture([event(1), { seq: 2, subject: 'p', kind: 'project.alert', payload: { kind: 'main_gate_failed' } }], 0);
  const original = f.call.getMockImplementation()!;
  f.call.mockImplementation((method, params) => method !== 'events.since' ? Promise.reject(Object.assign(new Error('不存在'), { code: 'not_found' })) : original(method, params));
  await f.notifier.pull(); expect(f.store.list()).toHaveLength(2);
  for (const row of f.store.list()) expect(row).toMatchObject({ project_id: null, body: null });
  expect(f.store.list()[0]).toMatchObject({ title: 'main 门禁失败', task_id: null }); expect(f.store.getCursor('notifier')).toBe(2);
});
test('分页拉完且并发 pull 合并为一轮，最多一个 events.since 在途', async () => {
  const f = fixture(Array.from({ length: 5 }, (_, i) => event(i + 1)), 0);
  const original = f.call.getMockImplementation()!; let release!: () => void;
  f.call.mockImplementationOnce(async (method, params) => { await new Promise<void>(r => { release = r; }); return original(method, params); });
  const first = f.notifier.pull(); expect(f.notifier.pull()).toBe(first); f.notifier.pull(); expect(f.call).toHaveBeenCalledTimes(1);
  release(); await first; expect(f.store.list()).toHaveLength(5);
  expect(f.call.mock.calls.filter(([m]) => m === 'events.since').map(([, p]) => p.after_seq)).toEqual([0, 2, 4, 5]);
});
test('唯一约束挡下不推送；消息恰五键；任务和项目补齐缓存', async () => {
  const alert: LedgerEvent = { seq: 1, subject: 'p', kind: 'project.alert', payload: { kind: 'main_gate_failed' } };
  const f = fixture([alert, { ...alert, seq: 2 }], 0); await f.notifier.pull();
  expect(f.store.list()[0]).toMatchObject({ project_id: 'p', task_id: null, title: 'dev 门禁失败', body: 'name： 合并后验证未通过' });
  expect(f.call.mock.calls.filter(([m]) => m === 'project.get')).toHaveLength(1);
  for (const [message] of f.publish.mock.calls) expect(Object.keys(message).sort()).toEqual(['id', 'kind', 'task_id', 'title', 'type']);
  f.store.commit({ cursor: { name: 'notifier', seq: 0 } }); f.restart(); await f.notifier.pull(); expect(f.publish).toHaveBeenCalledTimes(2);
  f.setEvents([event(3, 'T-1'), { ...event(4, 'T-1'), payload: { state: { state: 'done' } } }]); await f.notifier.pull();
  expect(f.call.mock.calls.filter(([m]) => m === 'task.get')).toHaveLength(1);
});
test('close 后不调 call，关闭时在途补齐结果不写库', async () => {
  const f = fixture([event(1)], 0); const original = f.call.getMockImplementation()!; let release!: () => void;
  f.call.mockImplementation(async (method, params) => { if (method === 'task.get') await new Promise<void>(r => { release = r; }); return original(method, params); });
  const pending = f.notifier.pull(); await vi.waitFor(() => expect(release).toBeTypeOf('function'));
  f.notifier.close(); release(); await pending; f.call.mockClear(); await f.notifier.pull(); expect(f.call).not.toHaveBeenCalled(); expect(f.store.list()).toHaveLength(0);
});
