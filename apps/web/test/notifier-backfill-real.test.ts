import { afterAll, beforeAll, expect, test } from 'vitest';
import { mkdtempSync, mkdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { randomUUID } from 'node:crypto';
import { DatabaseSync } from 'node:sqlite';
import { CoreSupervisor } from '../server/core/supervisor.ts';
import { MIGRATIONS, migrate, openWebStore } from '../server/store.ts';
import { createNotifier } from '../server/notifier.ts';
import type { LedgerEvent } from '../server/eventbus-protocol.ts';
import { dashboardDouble } from './fixtures/dashboard-double.ts';

const root = mkdtempSync(join(tmpdir(), 'backfill-real-'));
const home = join(root, 'autome-home'), wt = resolve(import.meta.dirname, '../../..');
const core = new CoreSupervisor({ home, binaryPath: join(wt, 'target/debug/automed') });
let store = openWebStore(home);
const messages: unknown[] = [], logs: string[] = [];
const make = () => createNotifier({ store, call: (m, p) => core.call(m, p), publish: m => messages.push(m), log: m => logs.push(m) });
let notifier = make();
const run = (phase: string) => promisify(execFile)('bash', [join(import.meta.dirname, 'fixtures/backfill-ledger.sh'), root, wt, phase], { timeout: 240000, maxBuffer: 4 * 1024 * 1024 });
type Page = { latest_seq: number; events: LedgerEvent[] };
type Task = { id: string; title: string; state: unknown };
const page = (after_seq = 0) => core.call('events.since', { after_seq, limit: 1000 }) as Promise<Page>;
const task = (task_id: string) => core.call('task.get', { task_id }) as Promise<{ task: Task; project: { id: string } }>;
function sourceRows() {
  const db = new DatabaseSync(join(home, 'state/web.sqlite3'));
  try { return db.prepare('SELECT source FROM notifications').all(); } finally { db.close(); }
}
// Only the synthetic no-op event is injected; real automed reads it over JSON-RPC.
async function sameStateEvent(id: string) {
  const before = await page(Number.MAX_SAFE_INTEGER);
  const db = new DatabaseSync(join(home, 'state/automed.sqlite3'));
  let seq: number;
  try {
    db.exec('PRAGMA busy_timeout=5000');
    const row = db.prepare("SELECT payload FROM events WHERE kind='task.updated' AND subject_id=? ORDER BY seq DESC LIMIT 1").get(id)!;
    const payload = JSON.parse(String(row.payload)); payload.trigger = 'same_state_probe';
    seq = Number(db.prepare('INSERT INTO events(event_id,kind,subject_id,payload,at) VALUES(?,?,?,?,?)').run(`evt-probe-${randomUUID()}`, 'task.updated', id, JSON.stringify(payload), new Date().toISOString()).lastInsertRowid);
  } finally { db.close(); }
  const read = await page(before.latest_seq);
  expect(read.latest_seq).toBe(seq);
  expect(read.events).toHaveLength(1);
  expect(read.events[0]).toMatchObject({ seq, kind: 'task.updated', subject: id, payload: { state: (await task(id)).task.state, trigger: 'same_state_probe' } });
  return seq;
}
function closePanel() { notifier.close(); store.close(); }
function reopenPanel() { store = openWebStore(home); notifier = make(); }
beforeAll(async () => { await core.start(); await run('setup'); }, 240000);
afterAll(async () => { closePanel(); await core.stop(); rmSync(root, { recursive: true, force: true }); });

test('真实账本一个 await_merge 任务，空面板库首次启动恰 1 条', async () => {
  const snapshot = await core.call('dashboard.get', {}) as ReturnType<typeof dashboardDouble>;
  const events = await page();
  const double = dashboardDouble(events.events);
  expect(Object.keys(snapshot).sort()).toEqual(Object.keys(double).sort());
  expect(snapshot.waiting).toHaveLength(1);
  expect(Object.keys(snapshot.waiting[0]).sort()).toEqual(Object.keys(double.waiting[0]).sort());
  for (const key of Object.keys(double.waiting[0].task)) expect(snapshot.waiting[0].task).toHaveProperty(key);
  expect(events.events.some(e => (e.payload as { state?: { node?: string } }).state?.node === 'await_design_approval')).toBe(true);
  await notifier.pull();
  const info = await task('T-1');
  expect(store.list()).toHaveLength(1);
  expect(store.list()[0]).toMatchObject({ kind: 'await_merge', task_id: 'T-1', title: 'T-1 可以合并了', body: info.task.title, project_id: info.project.id, event_seq: events.latest_seq });
  expect(sourceRows()).toEqual([{ source: 'backfill' }]);
  expect(messages).toEqual([{ type: 'notification', id: store.list()[0].id, kind: 'await_merge', task_id: 'T-1', title: 'T-1 可以合并了' }]);
  expect(logs).toEqual([]);
});
test('重启面板并追加一条不改状态的 task.updated，通知数不变', async () => {
  const rows = store.list(), pushed = [...messages];
  closePanel(); const seq = await sameStateEvent('T-1'); reopenPanel();
  await notifier.pull();
  expect(store.list()).toEqual(rows); expect(messages).toEqual(pushed);
  expect(store.getCursor('notifier')).toBe(seq); expect(logs).toEqual([]);
});
test('驳回后再次到 await_design_approval 新增 1 条', async () => {
  await run('redo'); await notifier.pull();
  expect(store.list()).toHaveLength(2);
  const cursor = store.getCursor('notifier')!;
  closePanel(); await run('reject'); reopenPanel();
  const replay = await page(cursor);
  const changes = replay.events.filter(e => e.kind === 'task.updated' && e.subject === 'T-2');
  expect(changes.some(e => (e.payload as { state: { node?: string } }).state.node === 'design')).toBe(true);
  const reentry = changes.filter(e => (e.payload as { state: { node?: string } }).state.node === 'await_design_approval').at(-1)!;
  expect(reentry).toBeDefined();
  await notifier.pull();
  expect(store.list()).toHaveLength(3);
  expect(store.list()[0]).toMatchObject({ task_id: 'T-2', kind: 'await_design_approval', event_seq: reentry.seq });
  expect(store.list().filter(row => row.task_id === 'T-1')).toHaveLength(1);
  expect(messages).toHaveLength(3); expect(logs).toEqual([]);
}, 240000);
test('旧版面板库升级后按真实历史播种，同状态事件不重发', async () => {
  const oldHome = join(root, 'old'); mkdirSync(join(oldHome, 'state'), { recursive: true });
  const db = new DatabaseSync(join(oldHome, 'state/web.sqlite3'));
  try {
    migrate(db, MIGRATIONS.slice(0, 1));
    db.prepare('INSERT INTO event_cursor(name,seq) VALUES(?,?)').run('notifier', (await page(Number.MAX_SAFE_INTEGER)).latest_seq);
  } finally { db.close(); }
  await sameStateEvent('T-1'); const seq = await sameStateEvent('T-2');
  const old = openWebStore(oldHome), pushed: unknown[] = [], errors: string[] = [];
  const upgraded = createNotifier({ store: old, call: (m, p) => core.call(m, p), publish: m => pushed.push(m), log: m => errors.push(m) });
  try {
    await upgraded.pull(); expect(old.list()).toEqual([]); expect(pushed).toEqual([]); expect(errors).toEqual([]);
    for (const id of ['T-1', 'T-2']) expect(old.getTaskState(id)?.state).toEqual((await task(id)).task.state);
    expect(old.getCursor('notifier_seed')).toBeUndefined(); expect(old.getCursor('notifier')).toBe(seq);
  } finally { upgraded.close(); old.close(); }
});
