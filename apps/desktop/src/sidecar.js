'use strict';

// Spawns and speaks to the `automed` Rust Core binary per plan §3.1/§9.5:
// "stdout 仅承载协议帧，stderr 仅承载诊断信息". We therefore pipe stdin/
// stdout for the framed Command/Event protocol and leave stderr to inherit
// (or be captured separately by the caller for diagnostics), never mixing
// the two streams.
//
// This is the M0 dev-mode sidecar only: it resolves the binary from a path
// (defaulting to the cargo debug build) rather than the packaged
// `process.resourcesPath/core/` layout, and it performs no manifest/
// signature verification, no single-instance-lock/nonce binding, and no
// PrepareShutdown/SafePark-aware graceful stop — all of which §9.5 requires
// before this can run against a real user machine. Deferred, not decided
// against; tracked as the next slice of the desktop shell.

const { spawn } = require('node:child_process');
const path = require('node:path');
const { encodeFrame, FrameDecoder } = require('./framing');

const MAX_FRAME_LEN = 8 * 1024 * 1024;

function defaultBinaryPath() {
  if (process.env.AUTOMED_BIN) return process.env.AUTOMED_BIN;
  const profile = process.env.AUTOMED_CARGO_PROFILE || 'debug';
  const exe = process.platform === 'win32' ? 'automed.exe' : 'automed';
  return path.join(__dirname, '..', '..', '..', 'target', profile, exe);
}

// Wraps one child `automed` process. `onEvent(event)` fires for every
// decoded Event frame in arrival order; `onStderrLine(line)` fires for
// diagnostic output (never protocol data). Pending requests are matched to
// replies strictly by arrival order (M0 has no correlation id in Event, so
// out-of-order replies are not distinguishable — acceptable only because
// the current single-command-in-flight IPC pattern never pipelines).
class AutomedSidecar {
  constructor({ binaryPath, dbPath, env, onEvent, onStderrLine, onExit } = {}) {
    this._binaryPath = binaryPath || defaultBinaryPath();
    this._dbPath = dbPath;
    this._onEvent = onEvent || (() => {});
    this._onStderrLine = onStderrLine || (() => {});
    this._onExit = onExit || (() => {});
    this._child = null;
    this._decoder = new FrameDecoder((frame) => {
      this._onEvent(JSON.parse(frame.toString('utf8')));
    }, MAX_FRAME_LEN);
    this._env = env;
  }

  start() {
    if (this._child) throw new Error('sidecar already started');
    this._child = spawn(this._binaryPath, [], {
      env: { ...process.env, ...this._env, AUTOMED_DB_PATH: this._dbPath },
      stdio: ['pipe', 'pipe', 'pipe'],
    });
    this._child.stdout.on('data', (chunk) => this._decoder.push(chunk));
    let stderrTail = '';
    this._child.stderr.setEncoding('utf8');
    this._child.stderr.on('data', (chunk) => {
      stderrTail += chunk;
      const lines = stderrTail.split('\n');
      stderrTail = lines.pop();
      for (const line of lines) this._onStderrLine(line);
    });
    this._child.on('exit', (code, signal) => this._onExit(code, signal));
    return this;
  }

  send(command) {
    if (!this._child) throw new Error('sidecar not started');
    this._child.stdin.write(encodeFrame(Buffer.from(JSON.stringify(command), 'utf8')));
  }

  // Best-effort graceful stop: close stdin so Core sees EOF and exits its
  // own loop (main.rs: "stdin closed, shutting down"), falling back to
  // SIGKILL after `graceMs` if it does not. This is a placeholder for the
  // real §9.5 PrepareShutdown/SafePark-gated stop, not a substitute for it.
  async stop(graceMs = 2000) {
    if (!this._child) return;
    const child = this._child;
    this._child = null;
    child.stdin.end();
    await new Promise((resolve) => {
      const timer = setTimeout(() => {
        child.kill('SIGKILL');
        resolve();
      }, graceMs);
      child.once('exit', () => {
        clearTimeout(timer);
        resolve();
      });
    });
  }
}

module.exports = { AutomedSidecar, defaultBinaryPath, MAX_FRAME_LEN };
