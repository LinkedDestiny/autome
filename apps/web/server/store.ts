import { DatabaseSync } from 'node:sqlite';
import { mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { randomUUID } from 'node:crypto';
import { resolveAutomeHome } from './home.ts';

export const MIGRATIONS = [`
CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);
CREATE TABLE event_cursor (name TEXT PRIMARY KEY, seq INTEGER NOT NULL);
CREATE TABLE notifications (
  id TEXT PRIMARY KEY, kind TEXT NOT NULL, project_id TEXT, task_id TEXT,
  title TEXT NOT NULL, body TEXT, event_seq INTEGER, created_at TEXT NOT NULL, read_at TEXT);
CREATE UNIQUE INDEX notifications_dedupe ON notifications (kind, ifnull(task_id, ''), event_seq);
CREATE INDEX notifications_unread ON notifications (read_at, task_id);
CREATE TABLE preferences (key TEXT PRIMARY KEY, value TEXT NOT NULL);
`, `
ALTER TABLE notifications ADD COLUMN source TEXT NOT NULL DEFAULT 'event';
CREATE TABLE notifier_task_state (task_id TEXT PRIMARY KEY, state TEXT NOT NULL, seq INTEGER NOT NULL);
INSERT INTO event_cursor (name, seq) SELECT 'notifier_seed', seq FROM event_cursor WHERE name = 'notifier';
`];

function transaction<T>(db: DatabaseSync, action: () => T): T {
  db.exec('BEGIN IMMEDIATE');
  try {
    const result = action();
    db.exec('COMMIT');
    return result;
  } catch (error) {
    db.exec('ROLLBACK');
    throw error;
  }
}

export function migrate(db: DatabaseSync, migrations = MIGRATIONS, path = 'web.sqlite3') {
  const exists = db.prepare("SELECT name FROM sqlite_master WHERE name = 'schema_version'").get();
  const version = exists ? Number(db.prepare('SELECT version FROM schema_version').get()?.version ?? 0) : 0;
  if (version > migrations.length) {
    throw new Error(`面板库版本 ${version} 比当前程序支持的 ${migrations.length} 新，请升级 Autome，或移走 ${path} 后重启`);
  }
  for (let i = version; i < migrations.length; i++) {
    try {
      transaction(db, () => {
        db.exec(migrations[i]);
        db.exec('DELETE FROM schema_version');
        db.prepare('INSERT INTO schema_version VALUES (?)').run(i + 1);
      });
    } catch (error) {
      throw new Error(`面板库迁移到第 ${i + 1} 版失败：${String(error)}`, { cause: error });
    }
  }
}

export type NotificationInput = {
  kind: string; project_id: string | null; task_id: string | null;
  title: string; body: string | null; event_seq: number | null;
};
export type Notification = NotificationInput & { id: string; created_at: string; read_at: string | null };
type Cursor = { name: string; seq: number };
type TaskState = { task_id: string; state: unknown; seq: number };
type Backfill = { notifications: NotificationInput[]; taskStates: TaskState[] };

export function openWebStore(home = resolveAutomeHome(process.env)) {
  const path = join(home, 'state', 'web.sqlite3');
  mkdirSync(join(home, 'state'), { recursive: true });
  const db = new DatabaseSync(path);
  try {
    migrate(db, MIGRATIONS, path);
    db.exec('PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000');
  } catch (error) {
    db.close();
    throw error;
  }
  const writeCursor = ({ name, seq }: Cursor) => {
    db.prepare('INSERT INTO event_cursor VALUES (?, ?) ON CONFLICT(name) DO UPDATE SET seq=excluded.seq').run(name, seq);
  };
  const writeTaskState = ({ task_id, state, seq }: TaskState) => {
    db.prepare('INSERT INTO notifier_task_state VALUES (?, ?, ?) ON CONFLICT(task_id) DO UPDATE SET state=excluded.state, seq=excluded.seq')
      .run(task_id, JSON.stringify(state), seq);
  };
  const writeNotification = (notification: NotificationInput, source: 'event' | 'backfill'): Notification | undefined => {
    const row: Notification = { ...notification, id: `ntf-${randomUUID()}`, created_at: new Date().toISOString(), read_at: null };
    const result = db.prepare(`INSERT OR IGNORE INTO notifications
      (id, kind, project_id, task_id, title, body, event_seq, created_at, read_at, source) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`)
      .run(row.id, row.kind, row.project_id, row.task_id, row.title, row.body, row.event_seq, row.created_at, row.read_at, source);
    return result.changes ? row : undefined;
  };
  const writeBackfill = ({ notifications, taskStates }: Backfill): Notification[] => {
    const rows: Notification[] = [];
    for (const notification of notifications) {
      const row = writeNotification(notification, 'backfill');
      if (row) rows.push(row);
    }
    for (const state of taskStates) writeTaskState(state);
    return rows;
  };
  const readIds = (where: string, ids: string[] = []) => db.prepare(
    `UPDATE notifications SET read_at=? WHERE read_at IS NULL AND (${where}) RETURNING id`,
  ).all(new Date().toISOString(), ...ids).map(row => String(row.id));
  return {
    close: () => db.close(),
    getCursor(name: string): number | undefined {
      const row = db.prepare('SELECT seq FROM event_cursor WHERE name=?').get(name);
      return row ? Number(row.seq) : undefined;
    },
    getTaskState(task_id: string): { state: unknown; seq: number } | undefined {
      const row = db.prepare('SELECT state, seq FROM notifier_task_state WHERE task_id=?').get(task_id);
      return row ? { state: JSON.parse(String(row.state)), seq: Number(row.seq) } : undefined;
    },
    commit({ cursor, notification, taskState }: { cursor: Cursor; notification?: NotificationInput; taskState?: TaskState }): Notification | undefined {
      return transaction(db, () => {
        const inserted = notification ? writeNotification(notification, 'event') : undefined;
        if (taskState) writeTaskState(taskState);
        writeCursor(cursor);
        return inserted;
      });
    },
    backfill({ cursor, notifications, taskStates }: Backfill & { cursor: Cursor }): Notification[] {
      return transaction(db, () => {
        const rows = writeBackfill({ notifications, taskStates });
        writeCursor(cursor);
        return rows;
      });
    },
    seedTaskStates(taskStates: TaskState[]) {
      transaction(db, () => {
        for (const state of taskStates) writeTaskState(state);
        db.exec("DELETE FROM event_cursor WHERE name='notifier_seed'");
      });
    },
    resetLedger(name: string, latest: number, backfill?: Backfill): Notification[] {
      return transaction(db, () => {
        db.exec("UPDATE notifications SET event_seq=NULL; DELETE FROM notifier_task_state; DELETE FROM event_cursor WHERE name='notifier_seed'");
        const rows = backfill ? writeBackfill(backfill) : [];
        writeCursor({ name, seq: latest });
        return rows;
      });
    },
    list({ limit = 50 }: { limit?: number } = {}): Notification[] {
      return db.prepare('SELECT id, kind, project_id, task_id, title, body, event_seq, created_at, read_at FROM notifications ORDER BY rowid DESC LIMIT ?').all(Math.min(200, Math.max(0, Math.trunc(limit)))) as Notification[];
    },
    unreadCount: () => Number(db.prepare('SELECT count(*) AS count FROM notifications WHERE read_at IS NULL').get()!.count),
    markRead: (ids: string[]) => ids.length ? readIds(`id IN (${ids.map(() => '?').join(',')})`, ids) : [],
    markAllRead: () => readIds('1'),
    getPreference: (key: string) => db.prepare('SELECT value FROM preferences WHERE key=?').get(key)?.value as string | undefined,
    setPreference(key: string, value: string) {
      db.prepare('INSERT INTO preferences VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value=excluded.value').run(key, value);
    },
    unreadByTask(kinds: string[]): { task_id: string; count: number }[] {
      if (!kinds.length) return [];
      return db.prepare(`SELECT task_id, count(*) AS count FROM notifications WHERE read_at IS NULL
        AND task_id IS NOT NULL AND kind IN (${kinds.map(() => '?').join(',')}) GROUP BY task_id`).all(...kinds) as { task_id: string; count: number }[];
    },
  };
}
