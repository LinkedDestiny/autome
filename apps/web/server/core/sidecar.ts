import { spawn, type ChildProcessWithoutNullStreams, type ChildProcess } from 'node:child_process';
import { encodeFrame, FrameDecoder } from './framing.ts';
import { resolveAutomedBinary } from './binary.ts';

export type Command = { request_id: string; [key: string]: unknown };
export type Outbound = { frame: string; request_id: string; command_id?: string; outcome: { status: string; code?: string; [key: string]: unknown }; [key: string]: unknown };
export type SidecarOptions = { binaryPath?: string; args?: string[]; dbPath?: string; env?: NodeJS.ProcessEnv; onEvent?: (event: Outbound) => void; onStderrLine?: (line: string) => void; onExit?: (code: number | null, signal: NodeJS.Signals | null) => void };
const MAX_FRAME_LEN = 8 * 1024 * 1024;
const DEFAULT_REQUEST_TIMEOUT_MS = 5000;
// Enough of the core's stderr to explain why it died, and not a byte more:
// this is kept so a failure can be *named* in the window, not so the app can
// hold a log.
const DIAGNOSTIC_LINES = 40;
const MAX_REASON_LENGTH = 400;

// Wraps one child `automed` process. Every frame on stdout is a tagged
// `Outbound` (`{"frame": "event", ...}` or `{"frame": "reply", ...}` — see
// crates/automed/src/ipc/envelope.rs): `onEvent(event)` fires for every
// decoded Event frame in arrival order; `onStderrLine(line)` fires for
// diagnostic output (never protocol data). Replies are correlated to the
// `request()` call that sent the matching Command by `request_id` — never
// by arrival order — since Core now guarantees exactly one Reply per
// Command, so a caller waiting on one never hangs.
export class AutomedSidecar {
  _binaryPath: string;
  _args: string[];
  _dbPath?: string;
  _env?: NodeJS.ProcessEnv;
  _onEvent: NonNullable<SidecarOptions['onEvent']>;
  _onStderrLine: NonNullable<SidecarOptions['onStderrLine']>;
  _onExit: NonNullable<SidecarOptions['onExit']>;
  _child: ChildProcessWithoutNullStreams | null;
  _pending: Map<string, { resolve: (reply: Outbound) => void; reject: (error: Error) => void; timer?: ReturnType<typeof setTimeout> }>;
  _decoder: FrameDecoder;
  _diagnostics: string[];
  _exited: boolean;
  _stopping: boolean;
  constructor({ binaryPath, args, dbPath, env, onEvent, onStderrLine, onExit }: SidecarOptions = {}) {
    const binary = binaryPath ? { path: binaryPath, args: [] } : resolveAutomedBinary();
    this._binaryPath = binary.path;
    this._args = args ?? binary.args;
    this._dbPath = dbPath;
    this._onEvent = onEvent || (() => {});
    this._onStderrLine = onStderrLine || (() => {});
    this._onExit = onExit || (() => {});
    this._child = null;
    this._pending = new Map();
    this._decoder = new FrameDecoder((frame) => {
      this._handleOutbound(JSON.parse(frame.toString('utf8')));
    }, MAX_FRAME_LEN);
    this._env = env;
    // The last few stderr lines, kept so `failureReason()` can say why the
    // core died rather than only that it did.
    this._diagnostics = [];
    this._exited = false;
    this._stopping = false;
  }

  _remember(line: string) {
    this._diagnostics.push(line);
    if (this._diagnostics.length > DIAGNOSTIC_LINES) this._diagnostics.shift();
  }

  /**
   * One sentence for why the core is not running, taken from its own stderr.
   *
   * The core reports a fatal startup failure the only way it can — a
   * `tracing` line on stderr, then exit — so that line is the reason, and
   * the window has nothing else to show the user.
   *
   * What comes back is the sentence the core wrote for a person, without the
   * apparatus around it: the ANSI colours, the timestamp and target prefix,
   * the English event name in front of it, and the structured fields trailing
   * behind. `tracing` puts the human-readable part in `error=`, so that is
   * what a banner gets when it is there.
   */
  failureReason() {
    const plain = this._diagnostics
      // ANSI escape removal is required to turn terminal diagnostics into user text.
      // eslint-disable-next-line no-control-regex
      .map((line) => line.replace(/\u001b\[[0-9;]*m/g, '').trim())
      .filter(Boolean);
    if (!plain.length) return null;
    const line = [...plain].reverse().find((l) => /\bERROR\b/.test(l)) || plain[plain.length - 1];

    const body = line.replace(/^.*?\bautomed\b:\s*/, '').replace(/^ERROR\s+/, '');
    const field = /(?:^|\s)error=([\s\S]*)$/.exec(body);
    const message =
      (field ? field[1] : body).replace(/(\s+\w+=(?:"[^"]*"|\S+))+$/, '').trim() || line;

    return message.length > MAX_REASON_LENGTH
      ? `${message.slice(0, MAX_REASON_LENGTH)}…`
      : message;
  }

  _handleOutbound(outbound: Outbound) {
    if (outbound.frame === 'event') {
      this._onEvent(outbound);
      return;
    }
    if (outbound.frame === 'reply') {
      const pending = this._pending.get(outbound.request_id);
      // No pending entry is not an error: `send()` fires commands without
      // waiting on a reply, and a malformed-frame Reply carries an empty
      // request_id nothing is ever waiting on.
      if (!pending) return;
      this._pending.delete(outbound.request_id);
      clearTimeout(pending.timer);
      pending.resolve(outbound);
    }
  }

  // Rejects every in-flight `request()` call — used when the process exits
  // or is stopped, so a caller waiting on a reply that can now never arrive
  // is unblocked instead of hanging forever.
  _rejectAllPending(error: Error) {
    for (const pending of this._pending.values()) {
      clearTimeout(pending.timer);
      pending.reject(error);
    }
    this._pending.clear();
  }

  start() {
    if (this._child) throw new Error('内核进程已启动');
    // No `dbPath` means the core picks its own — `AUTOME_HOME/state`, the one
    // place this machine's Autome state lives. The shell used to name
    // Electron's `userData` here, which put the ledger in a directory
    // Chromium owns and split the installation in two.
    const env = { ...process.env, ...this._env };
    if (this._dbPath) env.AUTOMED_DB_PATH = this._dbPath;
    this._child = spawn(this._binaryPath, this._args, {
      env,
      stdio: ['pipe', 'pipe', 'pipe'],
    });
    this._child.stdout.on('data', (chunk) => this._decoder.push(chunk));
    let stderrTail = '';
    this._child.stderr.setEncoding('utf8');
    this._child.stderr.on('data', (chunk) => {
      stderrTail += chunk;
      const lines = stderrTail.split('\n');
      stderrTail = lines.pop()!;
      for (const line of lines) {
        this._remember(line);
        this._onStderrLine(line);
      }
    });
    // A binary that is missing or not executable fails here, not at `exit`:
    // Node reports it as an `error` event and never emits `exit` at all.
    // Without this listener that event is an unhandled `error`, which throws
    // out of the event loop and takes Main down — the one failure mode the
    // window can least afford, since Main is what would have reported it.
    this._child.on('error', (err) => {
      this._remember(`ERROR 无法启动内核 ${this._binaryPath}：${err instanceof Error ? err.message : String(err)}`);
      this._finish(null, null);
    });
    // stdin reports a write to a dead process asynchronously, on the stream,
    // after `send()` has already returned. Unhandled, that is an uncaught
    // EPIPE in Main. `send()` throws synchronously for the same condition;
    // this is only here so the late echo of it is not fatal.
    this._child.stdin.on('error', (err) => {
      this._remember(`ERROR 写入内核失败：${err instanceof Error ? err.message : String(err)}`);
    });
    this._child.on('exit', (code, signal) => this._finish(code, signal));
    return this;
  }

  // Both ways a child can end — `exit` and a failed spawn — converge here, so
  // callers get exactly one notification either way.
  _finish(code: number | null, signal: NodeJS.Signals | null) {
    if (this._exited) return;
    this._exited = true;
    this._rejectAllPending(
      new Error(`内核在回复前退出 (code=${code}, signal=${signal})`)
    );
    this._releasePipes();
    if (this._stopping) return;
    this._onExit(code, signal);
  }

  /**
   * Closes the three stdio streams of a process that is over.
   *
   * `spawn` creates the pipes before it knows whether the binary is even
   * there, so a failed start leaves three open handles behind — nothing is
   * reading them and nothing ever will, but they are enough to keep a Node
   * event loop from draining. A process that holds them never exits: the
   * desktop test suite, run where the core had not been built, finished every
   * test and then sat there until CI killed the runner forty-five minutes
   * later.
   *
   * Idempotent and defensive: this runs on the failure path, where a stream
   * may be half-constructed, and throwing here would replace a clear error
   * with an obscure one.
   */
  _releasePipes(child = this._child) {
    if (!child) return;
    for (const stream of [child.stdin, child.stdout, child.stderr]) {
      try {
        if (stream && !stream.destroyed) stream.destroy();
      } catch {
        // Nothing to do about a stream that will not close; the process is
        // already gone and the handle is the operating system's problem now.
      }
    }
  }

  // Fire-and-forget: writes the Command frame and returns immediately,
  // without waiting for (or even decoding) its Reply. Prefer `request()`
  // for any caller that needs to know the outcome.
  //
  // Throws rather than writing into a closed pipe: a dead core is a fact the
  // caller has to handle, and `request()` turns this into a rejection.
  send(command: Command) {
    if (!this._child) throw new Error('内核进程尚未启动');
    if (this._exited || !this._child.stdin.writable) {
      throw new Error(`内核已退出，命令没有送达${this._because()}`);
    }
    try {
      this._child.stdin.write(encodeFrame(Buffer.from(JSON.stringify(command), 'utf8')));
    } catch (err) {
      throw new Error(`写入内核失败：${err instanceof Error ? err.message : String(err)}`, { cause: err });
    }
  }

  _because() {
    const reason = this.failureReason();
    return reason ? `：${reason}` : '';
  }

  // Sends `command` and resolves with its decoded Reply, correlated by
  // `request_id`. Rejects if no Reply arrives within `timeoutMs`, or if the
  // process exits first — never hangs silently either way.
  request(command: Command, { timeoutMs = DEFAULT_REQUEST_TIMEOUT_MS }: { timeoutMs?: number } = {}): Promise<Outbound> {
    return this._request(command, timeoutMs);
  }

  requestUntilReply(command: Command): Promise<Outbound> {
    return this._request(command);
  }

  isOutstanding(requestId: string): boolean {
    return this._pending.has(requestId);
  }

  _request(command: Command, timeoutMs?: number): Promise<Outbound> {
    if (!this._child) return Promise.reject(new Error('内核进程尚未启动'));
    if (this._pending.has(command.request_id)) {
      return Promise.reject(
        new Error(`请求 ${command.request_id} 已在等待回复`)
      );
    }
    return new Promise((resolve, reject) => {
      const timer = timeoutMs === undefined ? undefined : setTimeout(() => {
        this._pending.delete(command.request_id);
        reject(new Error(`请求 ${command.request_id} 等待 ${timeoutMs}ms 超时`));
      }, timeoutMs);
      this._pending.set(command.request_id, { resolve, reject, timer });
      try {
        this.send(command);
      } catch (e) {
        this._pending.delete(command.request_id);
        clearTimeout(timer);
        reject(e);
      }
    });
  }

  // Best-effort graceful stop: close stdin so Core sees EOF and exits its
  // own loop (main.rs: "stdin closed, shutting down"), falling back to
  // SIGKILL after `graceMs` if it does not. This is a placeholder for the
  // real §9.5 PrepareShutdown/SafePark-gated stop, not a substitute for it.
  async stop(graceMs = 2000) {
    if (!this._child) return;
    const child = this._child;
    // A stop we asked for is not a crash, and must not be counted as one by
    // whatever restart policy is watching `onExit`.
    this._stopping = true;
    this._child = null;
    // A child that never started has no process to wait for, and — worse —
    // no pid to signal. See `killIfRunning`.
    if (child.pid === undefined || this._exited) {
      this._releasePipes(child);
      return;
    }
    child.stdin.end();
    await new Promise<void>((resolve) => {
      const timer = setTimeout(() => {
        killIfRunning(child);
      }, graceMs);
      child.once('exit', () => {
        clearTimeout(timer);
        resolve();
      });
    });
  }
}

/**
 * Signals a child, but only one that actually exists.
 *
 * `spawn` returns a ChildProcess before it knows whether the binary is there.
 * When it is not, the object has an internal handle but **no pid** — and
 * `kill()` on that object does not throw, does not return, and does not kill
 * a child: it takes down the calling process, and with it everything in its
 * process group. Quitting Autome after a core that failed to start would have
 * killed the app itself, with no log line and no window, which is the one
 * failure mode a shell whose job is to report failures cannot have.
 *
 * Returns whether a signal was actually sent, so a caller can tell "killed" from
 * "there was nothing to kill".
 */
export function killIfRunning(child: ChildProcess | null, signal: NodeJS.Signals = 'SIGKILL') {
  if (!child || child.pid === undefined || child.exitCode !== null) return false;
  try {
    child.kill(signal);
    return true;
  } catch {
    // Already gone between the check and the call. Nothing to do.
    return false;
  }
}
