import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { DatabaseSync } from 'node:sqlite';

const dir = mkdtempSync(join(tmpdir(), 'autome-sqlite-'));
let db;
try {
  const file = join(dir, 'probe.sqlite');
  db = new DatabaseSync(file);
  const journalMode = db.prepare('PRAGMA journal_mode=WAL').get()?.journal_mode;
  assert.equal(journalMode, 'wal');
  db.exec('CREATE TABLE probe (value TEXT NOT NULL)');
  db.prepare('INSERT INTO probe VALUES (?)').run('中文 SQLite');
  db.close();
  db = new DatabaseSync(file);
  assert.equal(db.prepare('SELECT value FROM probe').get()?.value, '中文 SQLite');
  const sqlite = db.prepare('SELECT sqlite_version() AS version').get()?.version;
  console.log(JSON.stringify({ runtime: process.versions.electron ? 'electron' : 'node', node: process.versions.node,
    electron: process.versions.electron ?? null, sqlite, journalMode, ok: true }));
} finally {
  db?.close();
  rmSync(dir, { recursive: true, force: true });
}
