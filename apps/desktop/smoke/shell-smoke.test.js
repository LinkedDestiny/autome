const { test, after } = require('node:test');
const assert = require('node:assert/strict');
const { execFile, execFileSync, spawn } = require('node:child_process');
const { promisify } = require('node:util');
const { mkdtempSync, readFileSync, rmSync } = require('node:fs');
const { join, resolve } = require('node:path');
const { tmpdir } = require('node:os');
const { setTimeout: delay } = require('node:timers/promises');
const { isPanelProcess } = require('../src/shell/smoke');
const { seedNewerLedger } = require('./ledger-fixture');
const run = promisify(execFile);
const desktop = resolve(__dirname, '..');
const packaged = process.env.AUTOME_SMOKE_APP && resolve(process.env.INIT_CWD || process.cwd(), process.env.AUTOME_SMOKE_APP);
const electron = packaged ? join(packaged, 'Contents/MacOS/Autome') : require('electron');
const web = packaged ? join(packaged, 'Contents/Resources/web') : join(desktop, 'web');
const core = packaged ? join(packaged, 'Contents/Resources/core/autome') : resolve(desktop, '../../target/debug/autome');
const fixtures = [];
function fixture() {
  const home = mkdtempSync(join(tmpdir(), 'autome-shell-smoke-'));
  const env = { ...process.env, AUTOME_HOME: home, AUTOMED_DB_PATH: join(home, 'state/automed.sqlite3'), PATH: '/usr/bin:/bin', AUTOME_WEB_PORT: '0', AUTOME_NODE: electron };
  for (const key of ['AUTOME_BUNDLED_CORE', 'AUTOMED_BIN', 'AUTOME_TEST_BUILD_ID', 'AUTOME_ALLOW_OLDER_KERNEL', 'ELECTRON_RUN_AS_NODE', 'AUTOME_SHELL_SMOKE_BANNER']) delete env[key];
  const f = { home, env, file: join(home, 'web/server.json'), pids: new Set() }; fixtures.push(f); return f;
}
function launcher(f, args) { return run(electron, [join(web, 'bin/autome-web.js'), ...args], { env: { ...f.env, ELECTRON_RUN_AS_NODE: '1' }, timeout: 35000 }); }
function scan(f) {
  const rows = execFileSync('ps', ['-axo', 'pid=,ppid=,command='], { encoding: 'utf8' }).split('\n').flatMap(line => {
    const m = line.trim().match(/^(\d+)\s+(\d+)\s+(.*)$/); return m ? [{ pid: +m[1], parent: +m[2], command: m[3] }] : [];
  });
  try { f.pids.add(JSON.parse(readFileSync(f.file, 'utf8')).pid); } catch (error) { if (error.code !== 'ENOENT') throw error; }
  const ids = new Set([...f.pids, ...rows.filter(r => r.command.includes(f.home)).map(r => r.pid)]);
  let changed; do { changed = false; for (const row of rows) if (ids.has(row.parent) && !ids.has(row.pid)) { ids.add(row.pid); changed = true; } } while (changed);
  for (const pid of ids) f.pids.add(pid);
  return rows.filter(r => ids.has(r.pid) && r.pid !== process.pid);
}
async function cleanup(f) {
  let scanError; try { scan(f); } catch (error) { scanError = error; }
  try { await launcher(f, ['stop', '--force']); } catch { /* Still inspect and reap below. */ }
  if (scanError) throw scanError;
  for (const row of scan(f)) { try { process.kill(row.pid, 'SIGTERM'); } catch { /* Already exited. */ } }
  const deadline = Date.now() + 5000;
  while (scan(f).length && Date.now() < deadline) await delay(50);
  for (const row of scan(f)) { try { process.kill(row.pid, 'SIGKILL'); } catch { /* Already exited. */ } }
  assert.deepEqual(scan(f), [], `进程残留：${f.home}`);
}
after(async () => {
  const results = await Promise.allSettled(fixtures.map(async f => { await cleanup(f); rmSync(f.home, { recursive: true, force: true }); }));
  const failures = results.filter(result => result.status === 'rejected').map(result => result.reason);
  if (failures.length) throw new AggregateError(failures, '进程清理未通过，保留对应临时目录');
});
function shell(f, inspect) {
  return new Promise((resolveResult, reject) => {
    const child = spawn(electron, packaged ? [] : ['.'], { cwd: desktop, env: { ...f.env, AUTOME_SHELL_SMOKE: '1' }, stdio: ['pipe', 'pipe', 'pipe'] });
    if (child.pid) f.pids.add(child.pid);
    let stdout = '', stderr = '', result, check = Promise.resolve();
    const timeout = setTimeout(() => { child.kill('SIGKILL'); reject(Error('Electron 冒烟超时：' + stderr)); }, 45000);
    child.stdout.on('data', chunk => {
      stdout += chunk;
      if (!result) {
        const line = stdout.split('\n').find(line => line.startsWith('{"url":') && line.endsWith('}'));
        if (line) { result = JSON.parse(line); check = Promise.resolve().then(() => { scan(f); return inspect?.(result); }).finally(() => { if (!child.stdin.destroyed) child.stdin.end('done\n'); }); check.catch(() => {}); }
      }
    });
    child.stderr.on('data', chunk => { stderr += chunk; });
    child.on('error', error => { clearTimeout(timeout); reject(error); });
    child.on('close', async (code, signal) => {
      clearTimeout(timeout);
      try { assert.equal(code, 0, `signal=${signal}; ${stderr}`); assert.ok(result, stdout + stderr); await check; resolveResult(result); } catch (error) { reject(error); }
    });
  });
}
test('最小 PATH 用 Electron Node 启动；登记地址和 healthz 一致', async () => {
  const f = fixture();
  const result = await shell(f, async value => {
    const registration = JSON.parse(readFileSync(f.file, 'utf8'));
    assert.equal(value.url, registration.url); assert.equal(value.serverPid, registration.pid);
    assert.equal(registration.kernel.path, core); assert.equal(registration.kernel.source, 'bundle');
    const health = await (await fetch(value.url + '/healthz')).json(); assert.equal(health.ok, true); assert.equal(health.pid, value.serverPid);
    const info = await (await fetch(value.url + '/api/instance')).json();
    assert.equal(info.web_version, require('../../web/package.json').version);
  });
  assert.equal(result.reused, false);
});
test('已有 autome web 服务只复用，退出后仍存活', async () => {
  const f = fixture();
  const started = await run(core, ['web', 'start', '--json'], { env: { ...f.env, AUTOME_WEB_ROOT: web, ELECTRON_RUN_AS_NODE: '1' }, timeout: 35000 });
  const existing = JSON.parse(started.stdout);
  const value = await shell(f); assert.equal(value.reused, true); assert.equal(value.serverPid, existing.pid);
  assert.equal((await (await fetch(value.url + '/healthz')).json()).pid, existing.pid);
  assert.equal(scan(f).filter(row => isPanelProcess(row.command, f.home)).length, 1);
});
test('更新账本拒绝旧同包内核并显示横幅', async () => {
  const f = fixture();
  await run(core, ['status'], { env: f.env, timeout: 10000 });
  seedNewerLedger(f.env.AUTOMED_DB_PATH);
  if (!packaged) f.env.AUTOME_TEST_BUILD_ID = '0000001-20200101T000000Z-000000000000';
  f.env.AUTOME_SHELL_SMOKE_BANNER = '比推进过账本的内核';
  const value = await shell(f); assert.match(value.banner, /比推进过账本的内核/);
});
test('所有临时服务及内核均清理，无残留', async () => { for (const f of fixtures) { await cleanup(f); assert.deepEqual(scan(f), []); } });
