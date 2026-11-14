

// The stdio contract, verified from the actual Node module Electron Main will
// use: spawn the real `automed` binary, round-trip a command over real OS
// pipes, and check the decoded Reply and Event.
//
// Requires `cargo build -p automed` to have produced the binary first; this
// test deliberately does not build it itself.
//
// Every sidecar here gets its own `AUTOME_HOME` as well as its own database.
// Without it these tests would read and write the developer's real
// `~/.autome/config.toml`, which is both a wrong result and a surprising side
// effect of running the test suite.

import { test, afterEach } from 'vitest';
import { execFileSync } from 'node:child_process';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import crypto from 'node:crypto';
import { AutomedSidecar, killIfRunning, type Outbound, type SidecarOptions } from '../server/core/sidecar.ts';
const binaryPath = process.env.AUTOMED_BIN || path.resolve(import.meta.dirname, '../../../target/debug/automed');

// Everything these helpers hand out, removed when the process ends.
//
// Cleaning up at the end of each test was the rule, and the rule was kept
// about half the time: four tests deleted their database and not their home,
// so every run since left four directories behind — 636 of them by the time
// anyone counted. A test that throws halfway leaves its own, too. Registering
// the path where it is created is the only version of this that cannot be
// forgotten, so the per-test deletions are gone: there is one mechanism.
const scratch: string[] = [];
const running: AutomedSidecar[] = [];
afterEach(async () => { for (const sidecar of running.splice(0)) await sidecar.stop(); });
afterEach(() => {
  for (const target of scratch.splice(0)) fs.rmSync(target, { recursive: true, force: true });
});

function tempDbPath(label: string) {
  const file = path.join(
    os.tmpdir(),
    `automed-desktop-e2e-${label}-${crypto.randomUUID()}.sqlite3`
  );
  // SQLite's WAL and shared-memory files sit beside it and outlive it.
  scratch.push(file, `${file}-wal`, `${file}-shm`);
  return file;
}

function tempHome(label: string) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), `automed-web-home-${label}-`));
  scratch.push(dir);
  return dir;
}

/// A write that needs no prior state and emits exactly one event. Used as the
/// round-trip payload throughout: what is under test is the pipe, the framing
/// and the correlation, not this particular command.
function aWrite(requestId = 'req-1', commandId = 'cmd-1') {
  return {
    request_id: requestId,
    command_id: commandId,
    expected_revision: null,
    protocol_version: 1,
    method: 'config.set_loop',
    params: { parallel: 3 },
  };
}

/// Starts a sidecar with its own database *and* its own config root.
function startSidecar({ dbPath, home, onEvent }: { dbPath: string; home: string; onEvent?: SidecarOptions["onEvent"] }) {
  const sidecar = new AutomedSidecar({
    binaryPath,
    dbPath,
    env: { AUTOME_HOME: home },
    onEvent,
  }).start();
  running.push(sidecar);
  return sidecar;
}

test('no test starts a real core against the developer own ~/.autome', () => {
  const offenders: string[] = [];
  for (const file of fs.readdirSync(import.meta.dirname).filter(f => f.endsWith('.ts'))) {
    const source = fs.readFileSync(path.join(import.meta.dirname, file), 'utf8');
    for (const match of source.matchAll(/(?:new (?:AutomedSidecar|CoreSupervisor)\(|startServer\(|spawn\('npm',)\s*\{?/g)) {
      const block = source.slice(Math.max(0, match.index - 350), match.index + 450);
      if (!/AUTOME_HOME|home[,:]|binaryPath: path.join\(os.tmpdir\(\), `no-such-automed/.test(block)) offenders.push(file);
    }
  }
  assert.deepEqual(offenders, []);
});

test('automed binary is built before running sidecar e2e tests', () => {
  assert.ok(
    fs.existsSync(binaryPath),
    `缺少内核 ${binaryPath}；请先运行 \`cargo build -p automed\``
  );
});

test('a command round-trips through the real binary and emits its event', async () => {
  const dbPath = tempDbPath('roundtrip');
  const home = tempHome('roundtrip');
  const events: Outbound[] = [];
  const sidecar = startSidecar({ dbPath, home, onEvent: (e) => events.push(e) });

  sidecar.send(aWrite());
  await waitFor(() => events.length === 1);

  assert.equal(events[0].event_type, 'config.changed');
  assert.equal(events[0].event_seq, 1);

  await sidecar.stop();
});

test('the event stream continues across a restart against the same database', async () => {
  // The sequence number is the Renderer's resync anchor, so it must keep
  // counting rather than restarting at 1 when the core comes back.
  const dbPath = tempDbPath('restart');
  const home = tempHome('restart');

  const firstEvents: Outbound[] = [];
  const first = startSidecar({ dbPath, home, onEvent: (e) => firstEvents.push(e) });
  first.send(aWrite());
  await waitFor(() => firstEvents.length === 1);
  assert.equal(firstEvents[0].event_seq, 1);
  await first.stop();

  const secondEvents: Outbound[] = [];
  const second = startSidecar({ dbPath, home, onEvent: (e) => secondEvents.push(e) });
  second.send(aWrite());
  await waitFor(() => secondEvents.length === 1);
  assert.equal(secondEvents[0].event_seq, 2, 'the stream position survived the restart');

  await second.stop();
});

test('request() resolves with the Reply correlated by request_id', async () => {
  const dbPath = tempDbPath('request-ok');
  const home = tempHome('request-ok');
  const events: Outbound[] = [];
  const sidecar = startSidecar({ dbPath, home, onEvent: (e) => events.push(e) });
  const cmd = aWrite();

  const reply = await sidecar.request(cmd);

  assert.equal(reply.frame, 'reply');
  assert.equal(reply.request_id, cmd.request_id);
  assert.equal(reply.command_id, cmd.command_id);
  assert.equal(reply.outcome.status, 'ok');
  await waitFor(() => events.length === 1);

  await sidecar.stop();
});

test('an unknown method still produces exactly one reply, so a caller never hangs', async () => {
  const dbPath = tempDbPath('request-unknown');
  const home = tempHome('request-unknown');
  const sidecar = startSidecar({ dbPath, home });

  const reply = await sidecar.request({
    ...aWrite('req-unknown', 'cmd-unknown'),
    method: 'does.not.exist',
  });

  assert.equal(reply.outcome.status, 'error');
  assert.equal(reply.outcome.code, 'unknown_method');

  await sidecar.stop();
});

test('two in-flight request() calls each resolve to their own reply even if replies arrive out of order', async () => {
  const dbPath = tempDbPath('request-out-of-order');
  const home = tempHome('request-out-of-order');
  const sidecar = startSidecar({ dbPath, home });
  const first = aWrite('req-ooo-1', 'cmd-ooo-1');
  const second = aWrite('req-ooo-2', 'cmd-ooo-2');

  // Fire both requests concurrently; the sidecar must correlate each
  // resolved Promise to its own request_id regardless of which order the
  // two replies actually land on the wire in.
  const [replyFirst, replySecond] = await Promise.all([
    sidecar.request(first),
    sidecar.request(second),
  ]);

  assert.equal(replyFirst.request_id, 'req-ooo-1');
  assert.equal(replySecond.request_id, 'req-ooo-2');
  const reversed = new AutomedSidecar({
    binaryPath: path.join(import.meta.dirname, 'fixtures/fake-core.mjs'),
    env: { AUTOME_HOME: tempHome('reversed') },
  }).start();
  running.push(reversed);
  const order: string[] = [];
  const slow = reversed.request({ ...first, params: { delayMs: 100 } }).then(reply => { order.push(reply.request_id); return reply; });
  const fast = reversed.request({ ...second, params: { delayMs: 0 } }).then(reply => { order.push(reply.request_id); return reply; });
  const replies = await Promise.all([slow, fast]);
  assert.deepEqual(order, ['req-ooo-2', 'req-ooo-1']);
  assert.deepEqual(replies.map(reply => reply.request_id), ['req-ooo-1', 'req-ooo-2']);

  await sidecar.stop();
});

test('request() rejects a second call reusing an in-flight request_id', async () => {
  const dbPath = tempDbPath('request-dup');
  const home = tempHome('request-dup');
  const sidecar = startSidecar({ dbPath, home });
  sidecar.send = () => {}; // swallow the frame so the first request never resolves on its own
  const cmd = aWrite();

  const firstPending = sidecar.request(cmd, { timeoutMs: 5000 });
  await assert.rejects(sidecar.request(cmd, { timeoutMs: 5000 }), /已在等待回复/);

  firstPending.catch(() => {}); // it will reject once stop() below tears the process down
  await sidecar.stop();
});

test('request() rejects when no reply arrives before the timeout', async () => {
  const dbPath = tempDbPath('request-timeout');
  const home = tempHome('request-timeout');
  const sidecar = startSidecar({ dbPath, home });
  sidecar.send = () => {}; // swallow the frame so no reply can ever arrive

  await assert.rejects(
    sidecar.request(aWrite(), { timeoutMs: 20 }),
    /超时/
  );

  await sidecar.stop();
});

test('a pending request() is rejected if the process exits before replying', async () => {
  const dbPath = tempDbPath('request-exit');
  const home = tempHome('request-exit');
  const sidecar = startSidecar({ dbPath, home });
  sidecar.send = () => {}; // swallow the frame so the process has nothing to reply to

  const pending = sidecar.request(aWrite(), { timeoutMs: 5000 });
  const rejected = assert.rejects(pending, /回复前退出/);
  await sidecar.stop(); // closes stdin with nothing written; Core sees EOF and exits cleanly

  await rejected;
});

test('with no database named, the core puts one under its own home', async () => {
  // The shell stopped naming a path: the core's state belongs with the rest
  // of the core's state, and Electron's `userData` was a second installation
  // that nothing but the app could see. Verified against the real binary,
  // with `AUTOME_HOME` pointed at a temporary directory — which is also what
  // keeps this test off the developer's real `~/.autome`.
  const home = tempHome('own-home');
  const sidecar = new AutomedSidecar({
    binaryPath,
    env: { AUTOME_HOME: home },
    onEvent: () => {},
  }).start();

  const expected = path.join(home, 'state', 'automed.sqlite3');
  await waitFor(() => fs.existsSync(expected));
  const reply = await sidecar.request(aWrite('req-home', 'cmd-home'));
  assert.equal(reply.outcome.status, 'ok', 'the core is usable at its own default path');

  await sidecar.stop();
});

test('a binary that is not there is reported, not thrown past Main', async () => {
  // Node reports a failed spawn as an `error` event and never emits `exit`.
  // Unhandled, that event throws out of the event loop — in production, out
  // of Electron Main, which is the process that would have reported it.
  const exits: { code: number | null; signal: NodeJS.Signals | null }[] = [];
  const sidecar = new AutomedSidecar({
    binaryPath: path.join(os.tmpdir(), `no-such-automed-${crypto.randomUUID()}`),
    dbPath: tempDbPath('missing-binary'),
    onExit: (code, signal) => exits.push({ code, signal }),
  }).start();

  await waitFor(() => exits.length === 1);
  assert.match(sidecar.failureReason()!, /无法启动内核/);
  assert.equal(exits.length, 1, 'exactly one notification, whichever way the child ended');
});

test('a core that refuses to start is quoted, not merely counted', async () => {
  // The real failure: a database at the app's path that another program
  // wrote. The core says so on stderr and exits; that sentence is the only
  // thing the window can show the user, so the sidecar has to keep it.
  const conn = tempDbPath('foreign');
  execFileSync('sqlite3', [conn, 'CREATE TABLE projects (id TEXT PRIMARY KEY);']);

  const exits: { code: number | null; signal: NodeJS.Signals | null }[] = [];
  const sidecar = new AutomedSidecar({
    binaryPath,
    dbPath: conn,
    env: { AUTOME_HOME: tempHome('foreign-db') },
    onExit: (code, signal) => exits.push({ code, signal }),
  }).start();

  await waitFor(() => exits.length === 1);
  const reason = sidecar.failureReason()!;
  assert.match(reason, /不是 Autome 2\.0 的数据库/);
  // Stripped of everything that was written for a terminal rather than for a
  // person: colours, timestamp, the English event name, the trailing fields.
  // What is left is one sentence, which is what a banner can hold.
  assert.ok(!reason.includes('\u001b['), reason);
  assert.ok(!reason.startsWith('20'), reason);
  assert.ok(!reason.includes('failed to open the store'), reason);
  assert.ok(!reason.includes('db_path='), reason);
  assert.ok(reason.startsWith(conn), reason);

});

test('sending to a core that has exited throws with the reason, instead of an uncaught EPIPE', async () => {
  const dbPath = tempDbPath('epipe');
  const home = tempHome('epipe');
  const exits: { code: number | null; signal: NodeJS.Signals | null }[] = [];
  const sidecar = new AutomedSidecar({
    binaryPath,
    dbPath,
    env: { AUTOME_HOME: home },
    onExit: (code, signal) => exits.push({ code, signal }),
  }).start();

  // Kill the core out from under the sidecar, the way a crash would — through
  // the guarded helper, because `kill()` on a child that never started takes
  // down the caller rather than the child, and a suite run before `cargo
  // build` is exactly that case. When there is no core to kill, the state
  // this test wants already holds.
  killIfRunning(sidecar._child);
  await waitFor(() => exits.length === 1);

  assert.throws(() => sidecar.send(aWrite()), /内核已退出/);
  await assert.rejects(sidecar.request(aWrite('req-epipe', 'cmd-epipe')), /内核已退出/);

  // And nothing lands on the event loop afterwards: an unhandled 'error' on
  // stdin would surface here as an uncaught exception.
  await new Promise((resolve) => setTimeout(resolve, 100));

});

test('signalling a core that never started does not take the caller down with it', async () => {
  // `spawn` hands back a ChildProcess before it knows whether the binary is
  // there. When it is not, that object has no pid — and in the same tick,
  // while its internal handle is still around, `kill()` on it does not throw,
  // does not return, and does not kill a child: it signals the caller's own
  // process group. The caller dies.
  //
  // The window is narrow — a tick later the handle is gone and the same call
  // is harmless — which is exactly why it survived: nothing in the product
  // kills a child that fast. The test suite does, to simulate a crash, and a
  // run where the core had not been built took *itself* out. Nothing was
  // reported, because the reporter was the process that died; CI noticed
  // forty-five minutes later when the runner hit its own limit.
  //
  // Reaching the assertion below at all is the test: a regression here does
  // not fail, it vanishes.
  const sidecar = new AutomedSidecar({
    binaryPath: path.join(os.tmpdir(), `no-such-automed-${crypto.randomUUID()}`),
    dbPath: tempDbPath('kill-unstarted'),
    env: { AUTOME_HOME: tempHome('kill-unstarted') },
  }).start();

  assert.equal(sidecar._child!.pid, undefined, 'the premise: a child that never started');
  assert.equal(killIfRunning(sidecar._child), false, 'nothing was signalled');
  await sidecar.stop();
  assert.ok(true, 'still here');
});

test('a stop we asked for is not reported as a crash', async () => {
  // Otherwise every quit would spend one of the restart policy's three
  // strikes, and a quit during a restart would look like a failing core.
  const dbPath = tempDbPath('deliberate-stop');
  const home = tempHome('deliberate-stop');
  const exits: { code: number | null; signal: NodeJS.Signals | null }[] = [];
  const sidecar = new AutomedSidecar({
    binaryPath,
    dbPath,
    env: { AUTOME_HOME: home },
    onExit: (code, signal) => exits.push({ code, signal }),
  }).start();

  await sidecar.stop();
  await new Promise((resolve) => setTimeout(resolve, 100));
  assert.equal(exits.length, 0);

});

async function waitFor(predicate: () => boolean, timeoutMs = 5000) {
  const start = Date.now();
  while (!predicate()) {
    if (Date.now() - start > timeoutMs) throw new Error('timed out waiting for condition');
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
}

test('requestUntilReply 保留在途直到迟到回复或进程退出，普通请求仍超时', async () => {
  const sidecar = new AutomedSidecar({
    binaryPath: path.join(import.meta.dirname, 'fixtures/fake-core.mjs'),
    env: { AUTOME_HOME: tempHome('late-reply') },
  }).start();
  running.push(sidecar);
  const pending = sidecar.requestUntilReply({ ...aWrite('late'), params: { delayMs: 150 } });
  assert.equal(sidecar.isOutstanding('late'), true);
  const timeout = new Promise<string>(resolve => setTimeout(() => resolve('本地超时'), 20));
  assert.equal(await Promise.race([pending, timeout]), '本地超时');
  assert.equal(sidecar.isOutstanding('late'), true);
  assert.equal(sidecar._exited, false);
  assert.equal((await pending).request_id, 'late');
  assert.equal(sidecar.isOutstanding('late'), false);
  await assert.rejects(sidecar.request({ ...aWrite('ordinary'), params: { delayMs: 150 } }, { timeoutMs: 20 }), /超时/);
  assert.equal(sidecar.isOutstanding('ordinary'), false);
  const dying = sidecar.requestUntilReply({ ...aWrite('dying'), method: 'exit' });
  await assert.rejects(dying, /回复前退出/);
  assert.equal(sidecar.isOutstanding('dying'), false);
});

test('测试源码不按条件跳过真实内核用例', () => {
  for (const file of fs.readdirSync(import.meta.dirname).filter(f => f.endsWith('.ts'))) {
    const source = fs.readFileSync(path.join(import.meta.dirname, file), 'utf8');
    assert.doesNotMatch(source, /\b(?:test|it|describe)\s*\.\s*(?:skipIf|runIf|skip|todo)\s*\(/, file);
    assert.doesNotMatch(source, /\b(?:skip|todo)\s*:/, file);
    assert.doesNotMatch(source, /process\.env\s*(?:\.\s*CI\b|\[\s*['"]CI['"]\s*\])/, file);
  }
});

test('浏览器测试与配置不按条件跳过用例', () => {
  const root = path.resolve(import.meta.dirname, '../e2e');
  const files = fs.readdirSync(root, { recursive: true }).map(String).filter(file => file.endsWith('.ts')).map(file => path.join(root, file));
  files.push(path.resolve(root, '../playwright.config.ts'));
  assert.ok(files.length >= 3);
  for (const file of files) {
    const source = fs.readFileSync(file, 'utf8');
    assert.doesNotMatch(source, /\b(?:test|it|describe)\s*\.\s*(?:skipIf|runIf|skip|fixme|todo)\s*\(/, file);
    assert.doesNotMatch(source, /\b(?:skip|todo)\s*:/, file);
    assert.doesNotMatch(source, /process\.env\s*(?:\.\s*CI\b|\[\s*['"]CI['"]\s*\])/, file);
  }
});
