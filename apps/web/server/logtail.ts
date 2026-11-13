import { open as openFile } from 'node:fs/promises';
import type { createEventBus } from './eventbus.ts';
import { isKernelLogPath, parseLogSubscribe, parseLogUnsubscribe, type ExtensionMessage, type LogServerMessage } from './eventbus-protocol.ts';
import { splitChunks } from './logtail-chunk.ts';

type Bus = ReturnType<typeof createEventBus>;
type Connection = Parameters<Parameters<Bus['onDisconnect']>[0]>[0];
type Subscription = {
  connection: Connection; id: string; pos?: number; path?: string; ended: boolean;
  checked: number; identity?: { ino: number; dev: number }; pending?: Promise<void>;
  immediate?: ReturnType<typeof setImmediate>;
};
type EndReason = Extract<LogServerMessage, { type: 'log.end' }>['reason'];
const endMessages = {
  ended: '会话已结束', rotated: '日志文件被替换或截短，已停止跟随',
  deleted: '日志文件不存在或已被删除', error: '读取日志失败',
};
const messageOf = (error: unknown) => error instanceof Error ? error.message : String(error);
const codeOf = (error: unknown) => error && typeof error === 'object' && 'code' in error ? String(error.code) : 'UNKNOWN';

export function createLogtail({ bus, call, log, intervalMs = 500, statusEveryMs = 1000,
  readBudget = 256 * 1024, maxSubscriptions = 4, now = Date.now, open = openFile,
}: {
  bus: Bus; call: (method: string, params: Record<string, unknown>) => Promise<unknown>;
  log: (line: string) => void; intervalMs?: number; statusEveryMs?: number;
  readBudget?: number; maxSubscriptions?: number; now?: () => number; open?: typeof openFile;
}) {
  const connections = new Map<Connection, Map<string, Subscription>>();
  const jobs = new Set<Promise<void>>();
  let timer: ReturnType<typeof setInterval> | undefined;
  let closed = false;
  let openHandles = 0;
  function active(sub: Subscription) { return !closed && connections.get(sub.connection)?.get(sub.id) === sub; }
  function remove(sub: Subscription) {
    if (sub.immediate) clearImmediate(sub.immediate);
    sub.immediate = undefined;
    const entries = connections.get(sub.connection);
    if (entries?.get(sub.id) !== sub) return;
    entries.delete(sub.id);
    if (!entries.size) connections.delete(sub.connection);
    if (!connections.size && timer) { clearInterval(timer); timer = undefined; }
  }
  function end(sub: Subscription, reason: EndReason, code?: string) {
    if (!active(sub)) return;
    sub.connection.send({ type: 'log.end', session_id: sub.id, offset: sub.pos ?? 0, reason,
      message: reason === 'error' ? `${endMessages.error}：${code}` : endMessages[reason] });
    remove(sub);
  }
  function reject(sub: Subscription, code: Extract<LogServerMessage, { type: 'log.rejected' }>['code'], message: string) {
    if (!active(sub)) return;
    sub.connection.send({ type: 'log.rejected', session_id: sub.id, code, message });
    remove(sub);
  }
  async function status(sub: Subscription, initial: boolean) {
    sub.checked = now();
    try {
      const result = await call('session.log', { session_id: sub.id }) as { path: unknown; session: { running: boolean } };
      if (!active(sub)) return;
      if (initial) {
        if (!isKernelLogPath(result.path)) { reject(sub, 'kernel', '内核返回的日志路径不合法'); return; }
        sub.path = result.path;
      } else if (result.path !== sub.path) { end(sub, 'rotated'); return; }
      sub.ended = result.session.running === false;
    } catch (error) {
      if (!active(sub)) return;
      const message = messageOf(error);
      if (!initial) { log(`[日志] 查询会话状态失败：${message}`); return; }
      if (codeOf(error) === 'not_found') reject(sub, 'not_found', message);
      else if (message.startsWith('内核未连接')) reject(sub, 'offline', `内核离线，暂时拿不到日志：${message}`);
      else reject(sub, 'kernel', `查询日志失败：${message}`);
    }
  }
  async function read(sub: Subscription): Promise<boolean> {
    const handle = await open(sub.path!, 'r');
    openHandles++;
    try {
      if (!active(sub)) return false;
      const stat = await handle.stat();
      if (!active(sub)) return false;
      if (sub.identity && (sub.identity.ino !== stat.ino || sub.identity.dev !== stat.dev)) {
        end(sub, 'rotated'); return false;
      }
      sub.identity = { ino: stat.ino, dev: stat.dev };
      sub.pos ??= stat.size;
      if (stat.size < sub.pos) { end(sub, 'rotated'); return false; }
      const bytes = Buffer.alloc(Math.min(readBudget, stat.size - sub.pos));
      const { bytesRead } = await handle.read(bytes, 0, bytes.length, sub.pos);
      if (!active(sub)) return false;
      const atEof = sub.pos + bytesRead >= stat.size;
      const final = sub.ended && atEof;
      const chunks = splitChunks({ sessionId: sub.id, start: sub.pos, bytes: bytes.subarray(0, bytesRead), final });
      for (const chunk of chunks.messages) {
        if (!active(sub)) return false;
        sub.connection.send(chunk);
        sub.pos = chunk.next;
      }
      if (final) end(sub, 'ended');
      return bytesRead === readBudget && !atEof && chunks.consumed > 0;
    } finally {
      await handle.close();
      openHandles--;
    }
  }
  async function step(sub: Subscription) {
    const initial = sub.path === undefined;
    if (initial || (!sub.ended && now() - sub.checked >= statusEveryMs)) await status(sub, initial);
    if (!active(sub)) return;
    try {
      if (await read(sub) && active(sub)) {
        sub.immediate = setImmediate(() => { sub.immediate = undefined; void run(sub); });
      }
    } catch (error) {
      if (!active(sub)) return;
      if (codeOf(error) === 'ENOENT') {
        if (sub.identity || sub.ended) end(sub, 'deleted');
        else sub.pos ??= 0;
      } else end(sub, 'error', codeOf(error));
    }
  }
  function run(sub: Subscription): Promise<void> {
    if (sub.pending) return sub.pending;
    if (!active(sub)) return Promise.resolve();
    if (sub.immediate) clearImmediate(sub.immediate);
    sub.immediate = undefined;
    const job = step(sub).catch(error => {
      log(`[日志] 跟随失败：${messageOf(error)}`);
      remove(sub);
    }).finally(() => { sub.pending = undefined; jobs.delete(job); });
    sub.pending = job;
    jobs.add(job);
    return job;
  }
  async function flush() {
    await Promise.all([...connections.values()].flatMap(entries => [...entries.values()].map(run)));
  }
  function subscribe(message: ExtensionMessage, connection: Connection) {
    if (closed) return;
    const parsed = parseLogSubscribe(message);
    if (!parsed.ok) { connection.send({ type: 'log.rejected', code: parsed.code, message: parsed.message, session_id: parsed.session_id }); return; }
    const { session_id: id, offset: pos } = parsed.value;
    const entries = connections.get(connection);
    const previous = entries?.get(id);
    if (!previous && (entries?.size ?? 0) >= maxSubscriptions) {
      connection.send({ type: 'log.rejected', session_id: id, code: 'limit', message: '每个连接最多同时订阅 4 个日志，请先收起其它日志' });
      return;
    }
    if (previous) remove(previous);
    const sub: Subscription = { connection, id, pos, ended: false, checked: 0 };
    const current = connections.get(connection) ?? new Map<string, Subscription>();
    current.set(id, sub);
    connections.set(connection, current);
    timer ??= setInterval(() => { void flush(); }, intervalMs);
    void run(sub);
  }
  const off = [bus.onClientMessage('log.subscribe', subscribe), bus.onClientMessage('log.unsubscribe', (message, connection) => {
    if (closed) return;
    const parsed = parseLogUnsubscribe(message);
    if (!parsed.ok) { connection.send({ type: 'log.rejected', code: parsed.code, message: parsed.message, session_id: parsed.session_id }); return; }
    const sub = connections.get(connection)?.get(parsed.value.session_id);
    if (sub) remove(sub);
  }), bus.onDisconnect(connection => {
    for (const sub of connections.get(connection)?.values() ?? []) remove(sub);
  })];
  return {
    flush,
    stats: () => ({ connections: connections.size,
      subscriptions: [...connections.values()].reduce((sum, entries) => sum + entries.size, 0), openHandles, timer: timer !== undefined }),
    async close() {
      closed = true;
      for (const detach of off) detach();
      for (const entries of connections.values()) for (const sub of entries.values()) remove(sub);
      await Promise.all(jobs);
    },
  };
}
