'use strict';

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

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const crypto = require('node:crypto');
const { AutomedSidecar, defaultBinaryPath } = require('../src/sidecar');

// Everything these helpers hand out, removed when the process ends.
//
// Cleaning up at the end of each test was the rule, and the rule was kept
// about half the time: four tests deleted their database and not their home,
// so every run since left four directories behind — 636 of them by the time
// anyone counted. A test that throws halfway leaves its own, too. Registering
// the path where it is created is the only version of this that cannot be
// forgotten, so the per-test deletions are gone: there is one mechanism.
const scratch = [];
process.on('exit', () => {
  for (const target of scratch) fs.rmSync(target, { recursive: true, force: true });
});

function tempDbPath(label) {
  const file = path.join(
    os.tmpdir(),
    `automed-desktop-e2e-${label}-${crypto.randomUUID()}.sqlite3`
  );
  // SQLite's WAL and shared-memory files sit beside it and outlive it.
  scratch.push(file, `${file}-wal`, `${file}-shm`);
  return file;
}

function tempHome(label) {
  const dir = path.join(os.tmpdir(), `automed-desktop-home-${label}-${crypto.randomUUID()}`);
  fs.mkdirSync(dir, { recursive: true });
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
function startSidecar({ dbPath, home, onEvent }) {
  return new AutomedSidecar({
    dbPath,
    env: { AUTOME_HOME: home },
    onEvent,
  }).start();
}

test('the packaged location wins over the development build, but only if it is there', () => {
  // The order is the point. A developer running the packaged app must not
  // silently get their working-tree build; a packaged app has no Cargo target
  // directory to fall back to. So packaged is checked first and used only when
  // the binary actually exists there.
  const { packagedBinaryPath } = require('../src/sidecar');
  const originalResources = process.resourcesPath;
  const originalBin = process.env.AUTOMED_BIN;
  delete process.env.AUTOMED_BIN;

  const staging = path.join(os.tmpdir(), `automed-resources-${crypto.randomUUID()}`);
  fs.mkdirSync(path.join(staging, 'core'), { recursive: true });
  Object.defineProperty(process, 'resourcesPath', { value: staging, configurable: true });

  try {
    // Nothing there yet: fall back to the development build.
    assert.match(defaultBinaryPath(), /target[/\\](debug|release)[/\\]automed/);

    // Once the binary exists in the bundle, that is the one.
    const exe = process.platform === 'win32' ? 'automed.exe' : 'automed';
    const packaged = path.join(staging, 'core', exe);
    fs.writeFileSync(packaged, '');
    assert.equal(defaultBinaryPath(), packaged);
    assert.equal(packagedBinaryPath(exe), packaged);

    // And an explicit override beats both.
    process.env.AUTOMED_BIN = '/somewhere/else/automed';
    assert.equal(defaultBinaryPath(), '/somewhere/else/automed');
  } finally {
    delete process.env.AUTOMED_BIN;
    if (originalBin !== undefined) process.env.AUTOMED_BIN = originalBin;
    if (originalResources === undefined) {
      delete process.resourcesPath;
    } else {
      Object.defineProperty(process, 'resourcesPath', {
        value: originalResources,
        configurable: true,
      });
    }
    fs.rmSync(staging, { recursive: true, force: true });
  }
});

test('no test starts a real core against the developer own ~/.autome', () => {
  // Since the ledger moved under `AUTOME_HOME`, forgetting to set it here
  // does not merely read the developer's config — it writes their projects,
  // tasks and session history. The cost of the mistake went up, so it stops
  // being a thing to remember and becomes a thing that is checked.
  //
  // A sidecar in this file therefore either names an `AUTOME_HOME`, or names
  // a `binaryPath` that cannot start a core at all (the missing-binary case).
  const source = fs.readFileSync(__filename, 'utf8');
  const offenders = [];
  let from = 0;
  for (;;) {
    const at = source.indexOf('new AutomedSidecar({', from);
    if (at < 0) break;
    from = at + 1;
    const block = source.slice(at, at + 400);
    if (!block.includes('AUTOME_HOME') && !block.includes('binaryPath')) {
      offenders.push(source.slice(0, at).split('\n').length);
    }
  }
  assert.deepEqual(offenders, [], `sidecar started without AUTOME_HOME at line(s) ${offenders}`);
});

test('automed binary is built before running sidecar e2e tests', () => {
  assert.ok(
    fs.existsSync(defaultBinaryPath()),
    `expected ${defaultBinaryPath()} to exist; run \`cargo build -p automed\` first`
  );
});

test('a command round-trips through the real binary and emits its event', async () => {
  const dbPath = tempDbPath('roundtrip');
  const home = tempHome('roundtrip');
  const events = [];
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

  const firstEvents = [];
  const first = startSidecar({ dbPath, home, onEvent: (e) => firstEvents.push(e) });
  first.send(aWrite());
  await waitFor(() => firstEvents.length === 1);
  assert.equal(firstEvents[0].event_seq, 1);
  await first.stop();

  const secondEvents = [];
  const second = startSidecar({ dbPath, home, onEvent: (e) => secondEvents.push(e) });
  second.send(aWrite());
  await waitFor(() => secondEvents.length === 1);
  assert.equal(secondEvents[0].event_seq, 2, 'the stream position survived the restart');

  await second.stop();
});

test('request() resolves with the Reply correlated by request_id', async () => {
  const dbPath = tempDbPath('request-ok');
  const home = tempHome('request-ok');
  const events = [];
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

  await sidecar.stop();
});

test('request() rejects a second call reusing an in-flight request_id', async () => {
  const dbPath = tempDbPath('request-dup');
  const home = tempHome('request-dup');
  const sidecar = startSidecar({ dbPath, home });
  sidecar.send = () => {}; // swallow the frame so the first request never resolves on its own
  const cmd = aWrite();

  const firstPending = sidecar.request(cmd, { timeoutMs: 5000 });
  await assert.rejects(sidecar.request(cmd, { timeoutMs: 5000 }), /already pending/);

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
    /timed out/
  );

  await sidecar.stop();
});

test('a pending request() is rejected if the process exits before replying', async () => {
  const dbPath = tempDbPath('request-exit');
  const home = tempHome('request-exit');
  const sidecar = startSidecar({ dbPath, home });
  sidecar.send = () => {}; // swallow the frame so the process has nothing to reply to

  const pending = sidecar.request(aWrite(), { timeoutMs: 5000 });
  await sidecar.stop(); // closes stdin with nothing written; Core sees EOF and exits cleanly

  await assert.rejects(pending, /exited before replying/);
});

test('with no database named, the core puts one under its own home', async () => {
  // The shell stopped naming a path: the core's state belongs with the rest
  // of the core's state, and Electron's `userData` was a second installation
  // that nothing but the app could see. Verified against the real binary,
  // with `AUTOME_HOME` pointed at a temporary directory — which is also what
  // keeps this test off the developer's real `~/.autome`.
  const home = tempHome('own-home');
  const sidecar = new AutomedSidecar({
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
  const exits = [];
  const sidecar = new AutomedSidecar({
    binaryPath: path.join(os.tmpdir(), `no-such-automed-${crypto.randomUUID()}`),
    dbPath: tempDbPath('missing-binary'),
    onExit: (code, signal) => exits.push({ code, signal }),
  }).start();

  await waitFor(() => exits.length === 1);
  assert.match(sidecar.failureReason(), /无法启动内核/);
  assert.equal(exits.length, 1, 'exactly one notification, whichever way the child ended');
});

test('a core that refuses to start is quoted, not merely counted', async () => {
  // The real failure: a database at the app's path that another program
  // wrote. The core says so on stderr and exits; that sentence is the only
  // thing the window can show the user, so the sidecar has to keep it.
  const dbPath = tempDbPath('foreign-db');
  const conn = path.join(os.tmpdir(), `foreign-${crypto.randomUUID()}.sqlite3`);
  const { execFileSync } = require('node:child_process');
  execFileSync('sqlite3', [conn, 'CREATE TABLE projects (id TEXT PRIMARY KEY);']);

  const exits = [];
  const sidecar = new AutomedSidecar({
    dbPath: conn,
    env: { AUTOME_HOME: tempHome('foreign-db') },
    onExit: (code, signal) => exits.push({ code, signal }),
  }).start();

  await waitFor(() => exits.length === 1);
  const reason = sidecar.failureReason();
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
  const exits = [];
  const sidecar = new AutomedSidecar({
    dbPath,
    env: { AUTOME_HOME: home },
    onExit: (code, signal) => exits.push({ code, signal }),
  }).start();

  // Kill the core out from under the sidecar, the way a crash would.
  sidecar._child.kill('SIGKILL');
  await waitFor(() => exits.length === 1);

  assert.throws(() => sidecar.send(aWrite()), /内核已退出/);
  await assert.rejects(sidecar.request(aWrite('req-epipe', 'cmd-epipe')), /内核已退出/);

  // And nothing lands on the event loop afterwards: an unhandled 'error' on
  // stdin would surface here as an uncaught exception.
  await new Promise((resolve) => setTimeout(resolve, 100));

});

test('a stop we asked for is not reported as a crash', async () => {
  // Otherwise every quit would spend one of the restart policy's three
  // strikes, and a quit during a restart would look like a failing core.
  const dbPath = tempDbPath('deliberate-stop');
  const home = tempHome('deliberate-stop');
  const exits = [];
  const sidecar = new AutomedSidecar({
    dbPath,
    env: { AUTOME_HOME: home },
    onExit: (code, signal) => exits.push({ code, signal }),
  }).start();

  await sidecar.stop();
  await new Promise((resolve) => setTimeout(resolve, 100));
  assert.equal(exits.length, 0);

});

async function waitFor(predicate, timeoutMs = 5000) {
  const start = Date.now();
  while (!predicate()) {
    if (Date.now() - start > timeoutMs) throw new Error('timed out waiting for condition');
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
}
