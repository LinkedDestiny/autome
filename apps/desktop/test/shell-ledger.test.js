const { test } = require('node:test');
const assert = require('node:assert/strict');
const { execFileSync } = require('node:child_process');
const { mkdtempSync, rmSync } = require('node:fs');
const { tmpdir } = require('node:os');
const { join, resolve } = require('node:path');
const { DatabaseSync } = require('node:sqlite');
const { seedNewerLedger, newerBuild } = require('../smoke/ledger-fixture');
test('审计回归：status 不建 kernel_mark，夹具必须插入且可覆盖已有行', () => {
  const home = mkdtempSync(join(tmpdir(), 'autome-shell-ledger-'));
  const path = join(home, 'state/automed.sqlite3');
  try {
    execFileSync(resolve(__dirname, '../../../target/debug/autome'), ['status'], {
      env: { ...process.env, PATH: '/usr/bin:/bin', AUTOME_HOME: home, AUTOMED_DB_PATH: path }, stdio: 'pipe',
    });
    const db = new DatabaseSync(path);
    try {
      assert.equal(db.prepare('select count(*) as n from kernel_mark').get().n, 0);
      seedNewerLedger(path);
      assert.equal(db.prepare('select build_id from kernel_mark where id=1').get()?.build_id, newerBuild);
      db.prepare("update kernel_mark set build_id='old' where id=1").run();
      seedNewerLedger(path);
      assert.equal(db.prepare('select build_id from kernel_mark where id=1').get().build_id, newerBuild);
      assert.equal(db.prepare('select count(*) as n from kernel_mark').get().n, 1);
    } finally { db.close(); }
  } finally { rmSync(home, { recursive: true, force: true }); }
});
