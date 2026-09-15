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

function tempDbPath(label) {
  return path.join(os.tmpdir(), `automed-desktop-e2e-${label}-${crypto.randomUUID()}.sqlite3`);
}

function tempHome(label) {
  const dir = path.join(os.tmpdir(), `automed-desktop-home-${label}-${crypto.randomUUID()}`);
  fs.mkdirSync(dir, { recursive: true });
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
  fs.rmSync(dbPath, { force: true });
  fs.rmSync(home, { recursive: true, force: true });
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
  fs.rmSync(dbPath, { force: true });
  fs.rmSync(home, { recursive: true, force: true });
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
  fs.rmSync(dbPath, { force: true });
  fs.rmSync(home, { recursive: true, force: true });
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
  fs.rmSync(dbPath, { force: true });
  fs.rmSync(home, { recursive: true, force: true });
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
  fs.rmSync(dbPath, { force: true });
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
  fs.rmSync(dbPath, { force: true });
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
  fs.rmSync(dbPath, { force: true });
});

test('a pending request() is rejected if the process exits before replying', async () => {
  const dbPath = tempDbPath('request-exit');
  const home = tempHome('request-exit');
  const sidecar = startSidecar({ dbPath, home });
  sidecar.send = () => {}; // swallow the frame so the process has nothing to reply to

  const pending = sidecar.request(aWrite(), { timeoutMs: 5000 });
  await sidecar.stop(); // closes stdin with nothing written; Core sees EOF and exits cleanly

  await assert.rejects(pending, /exited before replying/);
  fs.rmSync(dbPath, { force: true });
});

async function waitFor(predicate, timeoutMs = 5000) {
  const start = Date.now();
  while (!predicate()) {
    if (Date.now() - start > timeoutMs) throw new Error('timed out waiting for condition');
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
}
