import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { openWebStore } from '../server/store.ts';
import { attachNotifier, createNotificationApi } from '../server/notifier-api.ts';
let home: string, store: ReturnType<typeof openWebStore>, api: ReturnType<typeof createNotificationApi>;
const publish = vi.fn();
function seed(seq: number, kind = 'failed') { return store.commit({ cursor: { name: 'notifier', seq }, notification: { kind, title: '标题', body: '正文', task_id: 'T-1', project_id: 'p', event_seq: seq } })!; }
beforeEach(() => { home = mkdtempSync(join(tmpdir(), 'ntf-api-')); store = openWebStore(home); publish.mockReset(); api = createNotificationApi({ store, publish }); });
afterEach(() => { store.close(); rmSync(home, { recursive: true, force: true }); });
test('列表形状、级别、limit 默认上限与非法输入', () => {
  for (let i = 1; i <= 201; i++) seed(i, i === 201 ? 'done' : 'failed');
  const result = api.list(undefined); expect(result.status).toBe(200);
  if (!result.body.ok) throw new Error('列表失败');
  expect(result.body.payload.unread).toBe(201); expect(result.body.payload.items).toHaveLength(50);
  expect(Object.keys(result.body.payload.items[0]).sort()).toEqual(['id','kind','level','project_id','task_id','title','body','created_at','read_at'].sort());
  expect(result.body.payload.items[0].level).toBe('fyi'); expect(result.body.payload.items[1].level).toBe('need_you');
  for (const [limit, count] of [['1',1],['01',1],['200',200]] as const) { const page = api.list(limit); if (!page.body.ok) throw new Error('列表失败'); expect(page.body.payload.items).toHaveLength(count); }
  for (const value of ['', '0', '-1', '201', '1.5', '1e2', 'NaN', ' 1', [], {}, '9007199254740992']) expect(api.list(value)).toMatchObject({ status: 400 });
});
test('单条已读先落库再广播', () => { const a = seed(1); seed(2); publish.mockImplementation(() => expect(store.list().find(r => r.id === a.id)?.read_at).not.toBeNull()); expect(api.read({ ids: [a.id] }).body).toEqual({ ok: true, payload: { unread: 1, ids: [a.id] } }); expect(publish).toHaveBeenCalledWith({ type: 'notification-read', ids: [a.id] }); });
test('全部已读写库并广播实际变化', () => { const a = seed(1), b = seed(2); expect(api.read({ all: true }).body).toMatchObject({ payload: { unread: 0, ids: expect.arrayContaining([a.id,b.id]) } }); expect(publish).toHaveBeenCalledTimes(1); expect(store.list().every(r => r.read_at)).toBe(true); });
test('无变化、未知 id、重复已读均成功不广播，合法边界', () => { const a = seed(1); api.read({ ids: [a.id, a.id] }); const at = store.list()[0].read_at; publish.mockClear(); for (const body of [{ ids: [a.id] }, { all: true }, { ids: [''] }, { ids: Array(200).fill('x'.repeat(64)) }]) expect(api.read(body).status).toBe(200); expect(publish).not.toHaveBeenCalled(); expect(store.list()[0].read_at).toBe(at); });
test('非法请求体全部拒绝且不改库', () => { seed(1); const before = store.list(); for (const body of [null, [], '', {}, { ids: [] }, { ids: [3] }, { ids: 'a' }, { ids: Array(201).fill('a') }, { ids: ['a'.repeat(65)] }, { ids: ['a'], all: true }, { all: false }, { all: true, extra: 1 }]) { expect(api.read(body).status).toBe(400); expect(store.list()).toEqual(before); } expect(publish).not.toHaveBeenCalled(); });
test('tick 同步返回且 bus 完成后才 pull，慢通知不阻塞后续 tick', async () => {
  let finish!: () => void; const first = new Promise<void>(r => { finish = r; });
  const bus = { pull: vi.fn().mockReturnValueOnce(first).mockResolvedValue(undefined), setCoreStatus: vi.fn() };
  let release!: () => void; const notifier = { pull: vi.fn(() => new Promise<void>(r => { release = r; })) };
  const attached = attachNotifier({ bus, notifier }); expect(attached.onTick()).toBeUndefined(); expect(notifier.pull).not.toHaveBeenCalled(); finish(); await first; await Promise.resolve(); expect(notifier.pull).toHaveBeenCalledTimes(1);
  attached.onTick(); await Promise.resolve(); expect(bus.pull).toHaveBeenCalledTimes(2); expect(notifier.pull).toHaveBeenCalledTimes(2); release();
});
test('全部内核状态转发，只有 connected 触发补拉', () => { const bus = { pull: vi.fn(), setCoreStatus: vi.fn() }, notifier = { pull: vi.fn().mockResolvedValue(undefined) }; const attached = attachNotifier({ bus, notifier }); for (const state of ['starting','offline','restarting','connected','stopped'] as const) { const status = { state, attempts: 0 }; attached.onStatus(status); expect(bus.setCoreStatus).toHaveBeenLastCalledWith(status); } expect(notifier.pull).toHaveBeenCalledTimes(1); });
