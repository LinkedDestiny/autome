import { AutomedSidecar, type Outbound } from './sidecar.ts';
import { nextRestart } from './restart-policy.ts';

export const CALL_TIMEOUT_MS = 15000;
export type CoreStatus = { state: 'starting' | 'connected' | 'restarting' | 'offline' | 'stopped'; reason?: string; attempts: number };
type Options = { binaryPath?: string; home: string; dbPath?: string; log?: (line: string) => void; now?: () => number; onStatus?: (status: CoreStatus) => void };
export type TickReport = { skipped?: boolean; errors?: string[]; [key: string]: unknown };

function payload(reply: Outbound): unknown {
  if (reply.outcome.status === 'error') {
    throw Object.assign(new Error(String(reply.outcome.message)), { code: reply.outcome.code });
  }
  return reply.outcome.payload;
}

export class CoreSupervisor {
  status: CoreStatus = { state: 'stopped', attempts: 0 };
  sidecar?: AutomedSidecar;
  private options: Options;
  private stopping = false;
  private timer?: ReturnType<typeof setTimeout>;
  private sequence = 0;
  private tickId?: string;

  constructor(options: Options) { this.options = options; }
  private setStatus(state: CoreStatus['state'], reason?: string) {
    this.status = { state, reason, attempts: this.status.attempts };
    this.options.onStatus?.(this.status);
  }
  isConnected() { return this.status.state === 'connected'; }
  isTickBusy() { return !!this.tickId && !!this.sidecar?.isOutstanding(this.tickId); }

  start({ timeoutMs = 5000 } = {}): Promise<TickReport> {
    clearTimeout(this.timer);
    this.stopping = false;
    const startedAt = (this.options.now ?? Date.now)();
    const sidecar = new AutomedSidecar({
      binaryPath: this.options.binaryPath,
      dbPath: this.options.dbPath,
      env: { ...process.env, AUTOME_HOME: this.options.home },
      onStderrLine: this.options.log,
      onExit: () => {
        if (this.stopping || this.sidecar !== sidecar) return;
        const decision = nextRestart({ ranForMs: (this.options.now ?? Date.now)() - startedAt, quickFailures: this.status.attempts });
        this.status.attempts = decision.quickFailures;
        const reason = sidecar.failureReason() ?? '内核已退出';
        this.setStatus(decision.restart ? 'restarting' : 'offline', reason);
        if (decision.restart) this.timer = setTimeout(() => {
          void this.start().catch(error => this.options.log?.(String(error)));
        }, decision.delayMs);
      },
    });
    this.sidecar = sidecar;
    this.setStatus('starting');
    sidecar.start();
    return this.tick({ timeoutMs });
  }

  async restart({ timeoutMs = 5000 } = {}) {
    if (this.status.state === 'starting' || this.isConnected()) return { already_running: true };
    this.status.attempts = 0;
    try {
      await this.start({ timeoutMs });
    } catch (error) {
      throw this.sidecar?.failureReason() ? new Error(this.sidecar.failureReason()!) : error;
    }
    return { restarted: true };
  }

  private command(method: string, params: Record<string, unknown>) {
    const n = ++this.sequence;
    return { request_id: `web-req-${n}`, command_id: `web-cmd-${n}`, expected_revision: null, protocol_version: 1, method, params };
  }
  private activeSidecar() {
    if (!this.sidecar || !['starting', 'connected'].includes(this.status.state)) {
      throw new Error(`内核未连接：${this.status.reason ?? '已停止'}`);
    }
    return this.sidecar;
  }
  private async wait<T>(promise: Promise<T>, method: string, timeoutMs: number): Promise<T> {
    let timer: ReturnType<typeof setTimeout> | undefined;
    try {
      return await Promise.race([promise, new Promise<never>((_, reject) => {
        timer = setTimeout(() => reject(Object.assign(new Error(`内核响应超时：${method}，操作可能仍在执行`), { timeout: true })), timeoutMs);
      })]);
    } finally { clearTimeout(timer); }
  }
  async call(method: string, params: Record<string, unknown> = {}, { timeoutMs = CALL_TIMEOUT_MS } = {}) {
    const sidecar = this.activeSidecar();
    return this.wait(sidecar.requestUntilReply(this.command(method, params)).then(payload), method, timeoutMs);
  }
  tick({ timeoutMs = 5000 } = {}): Promise<TickReport> {
    const sidecar = this.activeSidecar();
    if (this.isTickBusy()) return Promise.reject(new Error('调度仍在执行'));
    const command = this.command('scheduler.tick', {});
    this.tickId = command.request_id;
    const reply = sidecar.requestUntilReply(command).then(reply => {
      const report = payload(reply) as TickReport;
      if (this.sidecar === sidecar && this.status.state === 'starting') this.setStatus('connected');
      return report;
    });
    return this.wait(reply, 'scheduler.tick', timeoutMs);
  }
  async stop() {
    this.stopping = true;
    clearTimeout(this.timer);
    this.setStatus('stopped');
    await this.sidecar?.stop();
  }
}
