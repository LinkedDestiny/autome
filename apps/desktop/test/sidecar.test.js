'use strict';

// Same contract crates/automed/tests/stdio_loop.rs verifies from the Rust
// side, verified here from the actual Node module Electron Main will use:
// spawn the real `automed` binary, round-trip a command over real OS
// pipes, and check the decoded Event. Requires `cargo build -p automed`
// (or -p automed --release with AUTOMED_CARGO_PROFILE=release) to have
// produced the binary first; this test intentionally does not build it
// itself, matching Cargo's own tests/stdio_loop.rs which relies on the
// CARGO_BIN_EXE_automed env var Cargo provides rather than building
// out-of-band.

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

function advanceNominal(aggregateId) {
  return {
    request_id: 'req-1',
    command_id: 'cmd-1',
    expected_revision: null,
    protocol_version: 1,
    method: 'run.advance_nominal',
    params: { aggregate_id: aggregateId },
  };
}

test('automed binary is built before running sidecar e2e tests', () => {
  assert.ok(
    fs.existsSync(defaultBinaryPath()),
    `expected ${defaultBinaryPath()} to exist; run \`cargo build -p automed\` first`
  );
});

test('advance_nominal round-trips through the real binary', async () => {
  const dbPath = tempDbPath('advance');
  const events = [];
  const sidecar = new AutomedSidecar({ dbPath, onEvent: (e) => events.push(e) }).start();

  sidecar.send(advanceNominal('run-e2e'));
  await waitFor(() => events.length === 1);

  assert.equal(events[0].aggregate_id, 'run-e2e');
  assert.equal(events[0].aggregate_revision, 1);
  assert.equal(events[0].event_type, 'AdvanceNominal');
  assert.equal(events[0].event_seq, 1);

  await sidecar.stop();
  fs.rmSync(dbPath, { force: true });
});

test('state persists across sidecar restarts against the same db path', async () => {
  const dbPath = tempDbPath('restart');

  const firstEvents = [];
  const first = new AutomedSidecar({ dbPath, onEvent: (e) => firstEvents.push(e) }).start();
  first.send(advanceNominal('run-restart'));
  await waitFor(() => firstEvents.length === 1);
  assert.equal(firstEvents[0].aggregate_revision, 1);
  await first.stop();

  const secondEvents = [];
  const second = new AutomedSidecar({ dbPath, onEvent: (e) => secondEvents.push(e) }).start();
  second.send(advanceNominal('run-restart'));
  await waitFor(() => secondEvents.length === 1);
  assert.equal(secondEvents[0].aggregate_revision, 2);

  await second.stop();
  fs.rmSync(dbPath, { force: true });
});

async function waitFor(predicate, timeoutMs = 5000) {
  const start = Date.now();
  while (!predicate()) {
    if (Date.now() - start > timeoutMs) throw new Error('timed out waiting for condition');
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
}
