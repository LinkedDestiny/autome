const { DatabaseSync } = require('node:sqlite');
const newerBuild = '9999999-20990101T000000Z-ffffffffffff';
function seedNewerLedger(path) {
  const db = new DatabaseSync(path);
  try { db.prepare("INSERT INTO kernel_mark(id, build_id, updated_at) VALUES(1, ?, 'fixture') ON CONFLICT(id) DO UPDATE SET build_id=excluded.build_id, updated_at=excluded.updated_at").run(newerBuild); }
  finally { db.close(); }
}
module.exports = { seedNewerLedger, newerBuild };
