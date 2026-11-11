import { afterEach, expect, test } from 'vitest';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { DatabaseSync } from 'node:sqlite';
import { openStore, migrate, MIGRATIONS, type NotificationInput } from '../server/store.ts';

const cleanup: (() => void)[] = [];
afterEach(() => { for (const close of cleanup.splice(0).reverse()) close(); });
function setup() {
  const home = mkdtempSync(join(tmpdir(), 'web-store-'));
  cleanup.push(() => rmSync(home, { recursive: true, force: true }));
  let store = openStore(home);
  cleanup.push(() => store.close());
  const db = new DatabaseSync(join(home, 'state/web.sqlite3'));
  cleanup.push(() => db.close());
  return { home, db, store, reopen() { store.close(); store = openStore(home); return store; } };
}
function input(seq = 1, task: string | null = 'T-1'): NotificationInput {
  return { kind: 'failed', project_id: 'p', task_id: task, title: '任务失败', body: null, event_seq: seq };
}
function add(store: ReturnType<typeof openStore>, seq = 1, task: string | null = 'T-1') {
  return store.commit({ cursor: { name: 'notifier', seq }, notification: input(seq, task) })!;
}

test('首次建库四表列名、WAL 与版本 1', () => {
  const { db, store } = setup();
  const tables = { schema_version: ['version'], event_cursor: ['name', 'seq'], notifications: ['id', 'kind', 'project_id', 'task_id', 'title', 'body', 'event_seq', 'created_at', 'read_at'], preferences: ['key', 'value'] };
  for (const [name, columns] of Object.entries(tables)) expect(db.prepare(`PRAGMA table_info(${name})`).all().map(r => r.name)).toEqual(columns);
  expect(db.prepare('PRAGMA journal_mode').get()?.journal_mode).toBe('wal');
  expect(db.prepare('SELECT version FROM schema_version').get()?.version).toBe(1);
  expect(store.getCursor('missing')).toBeUndefined();
  expect(store.list()).toEqual([]);
  const empty = new DatabaseSync(':memory:');
  try {
    empty.exec('CREATE TABLE schema_version (version INTEGER NOT NULL)');
    migrate(empty);
    expect(empty.prepare('SELECT version FROM schema_version').get()?.version).toBe(1);
  } finally { empty.close(); }
});
test('重开不重建不丢数据', () => {
  const ctx = setup(); const row = add(ctx.store); ctx.store.setPreference('k', 'v');
  const store = ctx.reopen(); expect(store.list()).toEqual([row]); expect(store.getCursor('notifier')).toBe(1); expect(store.getPreference('k')).toBe('v');
});
test('注入第二级只跑缺的那级', () => {
  const { db } = setup();
  migrate(db, [...MIGRATIONS, 'CREATE TABLE extra (value TEXT);']);
  migrate(db, ['INVALID SQL', 'INVALID SQL']);
  expect(db.prepare('SELECT version FROM schema_version').get()?.version).toBe(2);
  expect(db.prepare('SELECT * FROM extra').all()).toEqual([]);
});
test('某级失败整级回滚且版本停在上一级', () => {
  const { db } = setup();
  expect(() => migrate(db, [...MIGRATIONS, 'CREATE TABLE extra (v TEXT); INSERT INTO missing VALUES (1);'])).toThrow('面板库迁移到第 2 版失败');
  expect(db.prepare('SELECT version FROM schema_version').get()?.version).toBe(1);
  expect(db.prepare("SELECT name FROM sqlite_master WHERE name='extra'").get()).toBeUndefined();
});
test('库比代码新时抛中文错误且库不变', () => {
  const { db, home, store } = setup(); add(store);
  db.exec('UPDATE schema_version SET version=9');
  const before = db.prepare('SELECT * FROM notifications').all();
  expect(() => openStore(home)).toThrow(`面板库版本 9 比当前程序支持的 1 新，请升级 Autome，或移走 ${join(home, 'state/web.sqlite3')} 后重启`);
  expect(db.prepare('SELECT version FROM schema_version').get()?.version).toBe(9);
  expect(db.prepare('SELECT * FROM notifications').all()).toEqual(before);
});
test('同键去重并推进游标，异键仍写入', () => {
  const { store } = setup(); const row = add(store);
  expect(store.commit({ cursor: { name: 'notifier', seq: 2 }, notification: input() })).toBeUndefined();
  expect(store.getCursor('notifier')).toBe(2); expect(store.list()).toEqual([row]);
  expect(add(store, 1, 'T-2')).toBeDefined(); expect(add(store, 2)).toBeDefined();
});
test('task_id 为 NULL 时同 kind 同 event_seq 去重', () => {
  const { store } = setup(); const row = add(store, 1, null);
  expect(add(store, 1, null)).toBeUndefined(); expect(store.list()).toEqual([row]); expect(row.task_id).toBeNull();
  expect(add(store, 2, null)).toBeDefined();
});
test('commit 注入游标写失败时通知随之回滚', () => {
  const { store, db } = setup();
  store.commit({ cursor: { name: 'notifier', seq: 0 } });
  db.exec("CREATE TRIGGER fail_cursor BEFORE UPDATE ON event_cursor BEGIN SELECT RAISE(ABORT, '游标写失败'); END;");
  expect(() => add(store)).toThrow('游标写失败');
  expect(store.list()).toEqual([]); expect(store.getCursor('notifier')).toBe(0);
  db.exec('DROP TRIGGER fail_cursor'); expect(add(store)).toBeDefined();
});
test('列表新到旧、limit 边界与未读数', () => {
  const { store } = setup(); for (let i = 0; i < 201; i++) add(store, i);
  expect(store.list()).toHaveLength(50); expect(store.list({ limit: 999 })).toHaveLength(200);
  expect(store.list({ limit: 0 })).toEqual([]); expect(store.list({ limit: 1 })[0].event_seq).toBe(200);
  expect(store.list({ limit: 2 }).map(r => r.event_seq)).toEqual([200, 199]); expect(store.unreadCount()).toBe(201);
});
test('单条已读只改该条且不覆盖已有 read_at', () => {
  const { store } = setup(); const a = add(store); const b = add(store, 2);
  expect(store.markRead([])).toEqual([]); expect(store.markRead(['missing'])).toEqual([]);
  expect(store.markRead([a.id, a.id])).toEqual([a.id]); const readAt = store.list()[1].read_at;
  expect(readAt).not.toBeNull(); expect(store.markRead([a.id])).toEqual([]); expect(store.list()[1].read_at).toBe(readAt);
  expect(store.list()[0]).toEqual(b); expect(store.unreadCount()).toBe(1);
});
test('全部已读返回实际改动 id', () => {
  const { store } = setup(); const a = add(store); const b = add(store, 2); store.markRead([a.id]);
  expect(store.markAllRead()).toEqual([b.id]); expect(store.markAllRead()).toEqual([]); expect(store.unreadCount()).toBe(0);
});
test('resetLedger 置空旧序号，新账本同键可写入且失败原子回滚', () => {
  const { store, db } = setup(); const a = add(store); store.markRead([a.id]);
  db.exec("CREATE TRIGGER fail_cursor BEFORE UPDATE ON event_cursor BEGIN SELECT RAISE(ABORT, '重置失败'); END;");
  expect(() => store.resetLedger('notifier', 0)).toThrow('重置失败'); expect(store.list()[0].event_seq).toBe(1);
  db.exec('DROP TRIGGER fail_cursor'); store.resetLedger('notifier', 0);
  expect(store.getCursor('notifier')).toBe(0); expect(store.list()[0]).toMatchObject({ id: a.id, event_seq: null });
  expect(store.list()[0].read_at).not.toBeNull(); expect(add(store)).toBeDefined(); expect(store.list()).toHaveLength(2);
});
test('偏好缺省、空串、覆盖读写', () => {
  const { store } = setup(); expect(store.getPreference('')).toBeUndefined(); store.setPreference('', ''); expect(store.getPreference('')).toBe('');
  store.setPreference('', '中文'); expect(store.getPreference('')).toBe('中文');
});
test('unreadByTask 过滤 kind、已读、无任务与空集合', () => {
  const { store } = setup(); const a = add(store); add(store, 2); add(store, 3, null); add(store, 4, 'T-2');
  store.commit({ cursor: { name: 'notifier', seq: 5 }, notification: { ...input(5), kind: 'done' } });
  store.markRead([a.id]); expect(store.unreadByTask([])).toEqual([]); expect(store.unreadByTask(['unknown'])).toEqual([]);
  expect(store.unreadByTask(['failed'])).toEqual([{ task_id: 'T-1', count: 1 }, { task_id: 'T-2', count: 1 }]);
});
