import { afterEach, expect, test, vi } from 'vitest';
import { mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { DatabaseSync } from 'node:sqlite';
import { dashboardDouble } from './fixtures/dashboard-double.ts';
import { migrate, MIGRATIONS, openWebStore } from '../server/store.ts';
import { createNotifier } from '../server/notifier.ts';
import type { LedgerEvent } from '../server/eventbus-protocol.ts';
const cleanups: (() => void)[] = [];
afterEach(() => { for (const clean of cleanups.splice(0).reverse()) clean(); });
const event = (seq: number, subject = `T-${seq}`): LedgerEvent => ({ seq, subject, kind: 'task.updated', payload: { state: { state: 'active', node: 'await_merge' }, trigger: 'gate_done' } });
function fixture(events: LedgerEvent[] = [], cursor?: number, legacy = false) {
  const home = mkdtempSync(join(tmpdir(), 'notifier-'));
  if (legacy) {
    mkdirSync(join(home, 'state'));
    const db = new DatabaseSync(join(home, 'state/web.sqlite3'));
    migrate(db, MIGRATIONS.slice(0, 1));
    db.prepare('INSERT INTO event_cursor VALUES (?, ?)').run('notifier', cursor!);
    db.close();
  }
  let store = openWebStore(home);
  const publish = vi.fn(), log = vi.fn();
  let latest = events.at(-1)?.seq ?? 0;
  const call = vi.fn(async (method: string, params: Record<string, unknown>): Promise<unknown> => method === 'events.since' ? {
    events: events.filter(e => e.seq > Number(params.after_seq)).slice(0, Number(params.limit ?? 200)), latest_seq: latest,
  } : method === 'dashboard.get' ? dashboardDouble(events) : method === 'task.get' ? { project: { id: 'p1' }, task: { title: '任务正文' } } : { project: { id: 'p', default_branch: 'dev', display_name: '', path: '/repo/name' } });
  if (cursor !== undefined) store.commit({ cursor: { name: 'notifier', seq: cursor } });
  const make = () => createNotifier({ store, call, publish, log, pageSize: 2 });
  let notifier = make();
  cleanups.push(() => { notifier.close(); store.close(); rmSync(home, { recursive: true, force: true }); });
  return { home, get store() { return store; }, get notifier() { return notifier; }, call, publish, log,
    setEvents(next: LedgerEvent[], last = next.at(-1)?.seq ?? 0) { events = next; latest = last; },
    restart() { notifier.close(); store.close(); store = openWebStore(home); notifier = make(); },
  };
}
test('首次启动只补当前等待中的任务、不补历史事件，空账本可初始化', async () => {
  for (const events of [[], [event(1, 'T-1'), event(2, 'T-2'), transition(3, 'done', 'T-2')]]) {
    const f = fixture(events); await f.notifier.pull();
    expect(f.call.mock.calls.slice(0, 3)).toEqual([
      ['events.since', { after_seq: Number.MAX_SAFE_INTEGER }], ['dashboard.get', {}], ['events.since', { after_seq: Number.MAX_SAFE_INTEGER }],
    ]);
    expect(f.store.getCursor('notifier')).toBe(events.length);
    expect(f.store.list()).toHaveLength(events.length ? 1 : 0);
    expect(f.call.mock.calls.some(([method]) => method === 'task.get')).toBe(false);
    if (events.length) {
      expect(f.store.list()[0]).toMatchObject({ task_id: 'T-1', event_seq: 3, title: 'T-1 可以合并了', body: '任务正文', project_id: 'p1' });
      expect(sources(f.home)).toEqual(['backfill']);
      expect(Object.keys(f.publish.mock.calls[0][0]).sort()).toEqual(['id', 'kind', 'task_id', 'title', 'type']);
    }
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

function transition(seq: number, node: string, subject = 'T-1'): LedgerEvent {
  return { ...event(seq, subject), payload: { state: ['done', 'cancelled', 'failed'].includes(node) ? { state: node } : { state: 'active', node } } };
}
function sources(home: string) {
  const db = new DatabaseSync(join(home, 'state/web.sqlite3'));
  try { return db.prepare('SELECT source FROM notifications ORDER BY rowid').all().map(row => row.source); } finally { db.close(); }
}
test('N1 重启后同状态新序号不重复', async () => {
  const boundary = fixture([event(Number.MAX_SAFE_INTEGER, 'T-001')]); await boundary.notifier.pull();
  expect(boundary.store.getCursor('notifier')).toBe(Number.MAX_SAFE_INTEGER);
  expect(boundary.store.list()[0]).toMatchObject({ task_id: 'T-001', event_seq: Number.MAX_SAFE_INTEGER });
  const states = ['await_merge', 'await_design_approval', 'await_info', 'not_a_bug', 'failed', 'implement', 'cancelled', 'done'];
  const f = fixture(states.map((state, i) => transition(i + 1, state, 'T-' + i)));
  await f.notifier.pull(); expect(f.store.list()).toHaveLength(5);
  expect(f.store.list().map(row => row.kind).sort()).toEqual(states.slice(0, 5).sort());
  f.restart(); await f.notifier.pull(); expect(f.publish).toHaveBeenCalledTimes(5);
  for (const initial of [undefined, 0]) {
    const f = fixture([event(1, 'T-1')], initial); await f.notifier.pull(); f.restart();
    f.setEvents([{ ...event(2, 'T-1'), payload: { state: { node: 'await_merge', state: 'active' } } }]);
    await f.notifier.pull(); expect(f.store.list()).toHaveLength(1); expect(f.publish).toHaveBeenCalledTimes(1);
    expect(f.store.getCursor('notifier')).toBe(2); expect(f.store.getTaskState('T-1')?.seq).toBe(1);
  }
});
test('N2 离开再进入重新通知', async () => {
  for (const restart of [false, true]) for (const middle of ['implement', 'await_info', 'await_design_approval', 'not_a_bug', 'failed']) {
    const f = fixture([event(1, 'T-1')]); await f.notifier.pull(); if (restart) f.restart();
    f.setEvents([transition(2, middle), event(3, 'T-1')]); await f.notifier.pull();
    expect(f.store.list()).toHaveLength(middle === 'implement' ? 2 : 3);
    expect(f.store.getTaskState('T-1')?.seq).toBe(3);
  }
});
test('N3 首次启动快照与序号交错不漏不重', async () => {
  for (const mode of ['review', 'waiting-change', 'after-leave-enter', 'after-enter', 'state-first', 'leave', 'after-waiting', 'unstable']) {
    const initial = mode === 'after-enter' ? transition(1, 'implement') : event(1, 'T-1');
    const f = fixture([initial]); const original = f.call.getMockImplementation()!;
    let maxReads = 0, changed = false;
    f.call.mockImplementation(async (method, params) => {
      const result = await original(method, params);
      if (method === 'events.since' && params.after_seq === Number.MAX_SAFE_INTEGER) {
        maxReads++;
        if ((mode === 'review' || mode === 'waiting-change' || mode === 'leave') && maxReads === 1) {
          f.setEvents(mode === 'leave' ? [transition(2, 'implement')] : [transition(2, 'merge_gate'), transition(3, mode === 'review' ? 'await_merge' : 'await_info')]);
        }
        if (mode === 'unstable' && maxReads % 2 === 1) f.setEvents([event(maxReads + 1, 'T-1')]);
        if (!changed && maxReads === 2 && mode.startsWith('after-')) {
          changed = true;
          f.setEvents(mode === 'after-leave-enter' ? [transition(2, 'implement'), event(3, 'T-1')] : [transition(2, mode === 'after-waiting' ? 'await_info' : 'await_merge')]);
        }
        if (mode === 'state-first' && maxReads === 2) f.setEvents([event(2, 'T-1')]);
      }
      return result;
    });
    await f.notifier.pull();
    const count = mode === 'unstable' || mode === 'leave' ? 0 : mode === 'after-leave-enter' || mode === 'after-waiting' ? 2 : 1;
    expect(f.store.list(), mode).toHaveLength(count);
    if (mode === 'unstable') {
      expect(f.store.getCursor('notifier')).toBeUndefined(); expect(f.publish).not.toHaveBeenCalled();
      expect(f.log).toHaveBeenCalledWith('[通知] 拉取失败：快照与账本序号不一致');
      f.call.mockImplementation(original); await f.notifier.pull(); expect(f.store.list()).toHaveLength(1);
    }
    if (mode === 'review' || mode === 'waiting-change') expect(f.store.list()[0].event_seq).toBe(3);
  }
});
test('N4 首次补发失败或关闭不写半截', async () => {
  for (const step of [1, 2, 3]) for (const action of ['close', 'error']) {
    const f = fixture([event(1)]); const original = f.call.getMockImplementation()!; let calls = 0;
    f.call.mockImplementation(async (method, params) => {
      const result = await original(method, params);
      if (++calls === step) { if (action === 'close') f.notifier.close(); else throw new Error('离线/超时'); }
      return result;
    });
    await f.notifier.pull(); expect(f.store.getCursor('notifier')).toBeUndefined(); expect(f.store.list()).toHaveLength(0); expect(f.publish).not.toHaveBeenCalled();
    f.call.mockImplementation(original); f.restart(); await f.notifier.pull(); expect(f.store.list()).toHaveLength(1);
  }
  const f = fixture([event(1), event(2)]); const db = new DatabaseSync(join(f.home, 'state/web.sqlite3'));
  db.exec("CREATE TRIGGER fail_cursor BEFORE INSERT ON event_cursor BEGIN SELECT RAISE(ABORT, '写盘失败'); END");
  await f.notifier.pull(); expect(f.store.list()).toHaveLength(0); expect(f.store.getTaskState('T-1')).toBeUndefined(); expect(f.publish).not.toHaveBeenCalled();
  db.exec('DROP TRIGGER fail_cursor'); db.close(); f.restart(); await f.notifier.pull(); expect(f.store.list()).toHaveLength(2);
});
test('N5 已有 v1 面板库升级：按游标前历史播种，不重发、不漏发', async () => {
  const history = [event(1, 'T-1'), transition(2, 'implement', 'T-2'), event(3, 'T-3'), transition(4, 'implement', 'T-3')];
  const f = fixture(history, 4, true);
  f.setEvents([...history, event(5, 'T-1'), event(6, 'T-2'), event(7, 'T-3')]);
  await f.notifier.pull();
  expect(f.store.list().map(row => row.task_id).sort()).toEqual(['T-2', 'T-3']);
  expect(f.store.getTaskState('T-1')?.seq).toBe(1); expect(f.store.getCursor('notifier_seed')).toBeUndefined();
  expect(f.call.mock.calls.some(([method]) => method === 'dashboard.get')).toBe(false); expect(sources(f.home)).toEqual(['event', 'event']);
  for (const fault of ['close', 'rpc', 'write']) {
    const retry = fixture(history, 4, true); const original = retry.call.getMockImplementation()!; let reads = 0;
    const db = new DatabaseSync(join(retry.home, 'state/web.sqlite3'));
    if (fault === 'write') db.exec("CREATE TRIGGER fail_seed BEFORE DELETE ON event_cursor WHEN OLD.name='notifier_seed' BEGIN SELECT RAISE(ABORT, '写盘失败'); END");
    retry.call.mockImplementation(async (method, params) => {
      const result = await original(method, params);
      if (++reads === 2) { if (fault === 'close') retry.notifier.close(); if (fault === 'rpc') throw new Error('读盘失败'); }
      return result;
    });
    await retry.notifier.pull(); expect(retry.store.getCursor('notifier_seed')).toBe(4);
    expect(retry.store.getTaskState('T-1')).toBeUndefined(); expect(retry.publish).not.toHaveBeenCalled();
    if (fault === 'write') db.exec('DROP TRIGGER fail_seed'); db.close();
    retry.call.mockImplementation(original); retry.restart(); await retry.notifier.pull();
    expect(retry.store.getCursor('notifier_seed')).toBeUndefined(); expect(retry.store.list()).toHaveLength(0);
  }
  const swapped = fixture([event(1)], 4, true); await swapped.notifier.pull();
  expect(swapped.store.getCursor('notifier_seed')).toBeUndefined(); expect(sources(swapped.home)).toEqual(['backfill']);
});
test('N6 换账本后对新账本补发，旧状态不抑制', async () => {
  const f = fixture([event(10, 'T-1')], 0); await f.notifier.pull(); f.setEvents([event(1, 'T-1')]);
  const original = f.call.getMockImplementation()!; let changed = false;
  f.call.mockImplementation(async (method, params) => { if (method === 'dashboard.get') throw new Error('离线'); return original(method, params); });
  await f.notifier.pull(); expect(f.store.getCursor('notifier')).toBe(10); expect(f.store.list()[0].event_seq).toBe(10);
  expect(f.store.getTaskState('T-1')?.seq).toBe(10); expect(f.publish).toHaveBeenCalledTimes(1);
  f.call.mockImplementation(async (method, params) => {
    const result = await original(method, params);
    if (!changed && method === 'events.since' && params.after_seq === Number.MAX_SAFE_INTEGER) {
      changed = true; f.setEvents([event(1, 'T-1'), event(2, 'T-1')]);
    }
    return result;
  });
  await f.notifier.pull(); expect(f.store.list().map(row => row.event_seq)).toEqual([2, null]);
  expect(f.store.getTaskState('T-1')?.seq).toBe(2); expect(sources(f.home)).toEqual(['event', 'backfill']);
});
