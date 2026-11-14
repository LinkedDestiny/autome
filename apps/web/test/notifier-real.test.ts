import { DatabaseSync } from 'node:sqlite';
import type { dashboardDouble } from './fixtures/dashboard-double.ts';
import { afterAll, beforeAll, expect, test } from 'vitest';
import { mkdtempSync, rmSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { CoreSupervisor } from '../server/core/supervisor.ts';
import { openWebStore } from '../server/store.ts';
import { createNotifier } from '../server/notifier.ts';
const root = mkdtempSync(join(tmpdir(), 'notifier-real-'));
const home = join(root, 'autome-home');
const wt = resolve(import.meta.dirname, '../../..');
const core = new CoreSupervisor({ home: home, binaryPath: join(wt, 'target/debug/automed') });
let store = openWebStore(home);
const messages: unknown[] = [], logs: string[] = [];
const make = () => createNotifier({ store, call: (m, p) => core.call(m, p), publish: m => messages.push(m), log: line => logs.push(line) });
let notifier = make();
// The seven-task ledger drives whole loops and merges; under CI load it exceeds 90s.
const ledgerTimeout = 300000;
const run = (extra = '') => promisify(execFile)('bash', [join(import.meta.dirname, 'fixtures/w4-ledger.sh'), root, wt, extra], { timeout: extra ? 90000 : ledgerTimeout, maxBuffer: 4 * 1024 * 1024 });
beforeAll(async () => { await core.start(); await notifier.pull(); }, 10000);
afterAll(async () => { notifier.close(); store.close(); await core.stop(); rmSync(root, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 }); });
test('真实账本七类通知计数与项目补齐，新面板库只补当前等待中的任务', async () => {
  await run(); await notifier.pull();
  const rows = store.list(); const counts: Record<string, number> = {};
  for (const row of rows) counts[row.kind] = (counts[row.kind] ?? 0) + 1;
  expect(logs).toEqual([]);
  expect(counts).toEqual({ await_design_approval: 3, await_merge: 2, done: 2, failed: 3, await_info: 2, project_alert: 1, not_a_bug: 1 });
  const project = JSON.parse(readFileSync(join(root, 'output/project-get.json'), 'utf8')).project;
  expect(rows.every(row => row.project_id === project.id)).toBe(true);
  expect(rows.find(row => row.kind === 'project_alert')).toMatchObject({ title: 'main 门禁失败', body: 'repo：T-2 合并后验证未通过' });
  expect(messages).toHaveLength(14);
  const snapshot = await core.call('dashboard.get', {}) as ReturnType<typeof dashboardDouble>;
  const latest = await core.call('events.since', { after_seq: Number.MAX_SAFE_INTEGER }) as { latest_seq: number };
  const freshMessages: unknown[] = [];
  const freshStore = openWebStore(join(root, 'fresh'));
  const fresh = createNotifier({ store: freshStore, call: (m, p) => core.call(m, p), publish(m) { freshMessages.push(m); }, log: line => { throw new Error(line); } });
  try {
    await fresh.pull(); const backfill = freshStore.list();
    expect(backfill).toHaveLength(5);
    expect(backfill.map(r => r.task_id).sort()).toEqual(snapshot.waiting.map(w => w.task.id).sort());
    for (const row of backfill) {
      const eventRow = rows.find(r => r.kind === row.kind && r.task_id === row.task_id)!;
      expect(eventRow).toBeDefined();
      expect(row).toMatchObject({ title: eventRow.title, body: eventRow.body, project_id: eventRow.project_id, event_seq: latest.latest_seq });
      expect(['done', 'project_alert']).not.toContain(row.kind);
    }
    const db = new DatabaseSync(join(root, 'fresh/state/web.sqlite3'));
    try { expect(db.prepare('SELECT DISTINCT source FROM notifications').all()).toEqual([{ source: 'backfill' }]); } finally { db.close(); }
    expect(freshMessages).toHaveLength(backfill.length);
    expect(freshMessages).toEqual(expect.arrayContaining(backfill.map(row => ({ type: 'notification', id: row.id, kind: row.kind, task_id: row.task_id, title: row.title }))));
    expect(freshStore.getCursor('notifier')).toBe(latest.latest_seq);
  }
  finally { fresh.close(); freshStore.close(); }
}, ledgerTimeout + 10000);
test('关库期间真实账本新增失败，重开只补一条', async () => {
  const ids = store.list().map(row => row.id); expect(ids).toHaveLength(14);
  notifier.close(); store.close(); await run('extra'); store = openWebStore(home); notifier = make();
  await notifier.pull(); expect(logs).toEqual([]);
  const rows = store.list(); expect(rows).toHaveLength(15); expect(rows.slice(1).map(row => row.id)).toEqual(ids);
  expect(rows[0]).toMatchObject({ kind: 'failed', title: 'T-8 停在失败上：会话异常退出（会话退出码 3）' }); expect(messages).toHaveLength(15);
}, 100000);
