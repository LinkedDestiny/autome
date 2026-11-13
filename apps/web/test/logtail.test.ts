import { afterEach, expect, test, vi } from 'vitest';
import { appendFile, mkdtemp, open as openFile, rename, rm, truncate, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createEventBus } from '../server/eventbus.ts';
import { createLogtail } from '../server/logtail.ts';
import type { LogServerMessage } from '../server/eventbus-protocol.ts';

const cleanups: (() => Promise<void>)[] = [];
afterEach(async () => { for (const cleanup of cleanups.splice(0).reverse()) await cleanup(); });
function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>(done => { resolve = done; });
  cleanups.push(async () => { resolve(); });
  return { promise, resolve };
}
async function fixture(content: string | Buffer = '', running = true, timing: { intervalMs?: number; statusEveryMs?: number } = {}) {
  const dir = await mkdtemp(join(tmpdir(), 'w8-logtail-'));
  const path = join(dir, 'session.log');
  await writeFile(path, content);
  const state = { path, running, onCall: async () => {}, onRead: async () => {}, fail: '' };
  const call = vi.fn(async (method: string, params: Record<string, unknown>) => {
    expect(method).toBe('session.log');
    expect(Object.keys(params)).toEqual(['session_id']);
    await state.onCall();
    return { path: state.path, session: { id: params.session_id, running: state.running,
      lifecycle: { state: state.running ? 'running' : 'exited' }, log_path: state.path }, log: '', truncated: false };
  });
  let handles = 0;
  const reads: number[] = [];
  const open = vi.fn<typeof openFile>(async (...args) => {
    if (state.fail === 'open') throw Object.assign(new Error('拒绝读取'), { code: 'EACCES' });
    const handle = await openFile(...args);
    handles++;
    return new Proxy(handle, { get(target, key) {
      const value = Reflect.get(target, key, target);
      if (typeof value !== 'function') return value;
      return async (...args: unknown[]) => {
        if (key === 'read') await state.onRead();
        if (state.fail === key) throw Object.assign(new Error('读取失败'), { code: 'EIO' });
        const result = await Reflect.apply(value, target, args);
        if (key === 'close') handles--;
        if (key === 'read') reads.push((result as { bytesRead: number }).bytesRead);
        return result;
      };
    } });
  });
  const log = vi.fn();
  const bus = createEventBus({ call: async () => ({ events: [], latest_seq: 0 }), log });
  // Timers never need to fire in these deterministic tests; flush advances a round.
  const tail = createLogtail({ bus, call, open, log, intervalMs: 60_000, statusEveryMs: 0, ...timing });
  function client() {
    const messages: LogServerMessage[] = [];
    const transport = { send: (text: string) => {
      const message = JSON.parse(text);
      if (message.type.startsWith('log.')) messages.push(message);
    }, close: vi.fn() };
    const disconnect = bus.connect(transport);
    const send = (message: unknown) => bus.receive(transport, JSON.stringify(message));
    const subscribe = (id = 'ses-a', offset: number | undefined = 0) => send({ type: 'log.subscribe', session_id: id, ...(offset === undefined ? {} : { offset }) });
    return { messages, disconnect, send, subscribe,
      chunks: () => messages.filter(m => m.type === 'log.chunk'),
      text: () => messages.filter(m => m.type === 'log.chunk').map(m => m.text).join('') };
  }
  const c = client();
  const empty = () => {
    expect(tail.stats()).toEqual({ connections: 0, subscriptions: 0, openHandles: 0, timer: false });
    expect(handles).toBe(0);
  };
  cleanups.push(async () => { await tail.close(); bus.close(); expect(handles).toBe(0); await rm(dir, { recursive: true, force: true }); });
  return { dir, path, state, call, open, log, tail, client, c, empty, reads, handles: () => handles };
}

test('01 缺省从末尾跟随；offset 0 从头读', async () => {
  const f = await fixture('已有');
  f.c.send({ type: 'log.subscribe', session_id: 'ses-a' });
  await f.tail.flush();
  expect(f.c.messages).toEqual([]);
  await appendFile(f.path, '新内容');
  await f.tail.flush();
  expect(f.c.chunks()).toEqual([{ type: 'log.chunk', session_id: 'ses-a', offset: 6, next: 15, text: '新内容' }]);
  f.c.subscribe(); await f.tail.flush();
  expect(f.c.chunks().at(-1)?.text).toBe('已有新内容');
});
test('02 客户端路径与非法 id 在查询和打开之前被拒绝', async () => {
  const f = await fixture();
  for (const extra of [{ path: '/etc/passwd' }, { session_id: '../x' }, { session_id: '' }, { session_id: 's'.repeat(129) }, { offset: '00' }, { offset: -1 }, { offset: Number.MAX_SAFE_INTEGER + 1 }]) {
    f.c.send({ type: 'log.subscribe', session_id: 'ses-a', ...extra });
    expect(f.c.messages.at(-1)).toMatchObject({ type: 'log.rejected', code: 'invalid' });
  }
  expect(f.call).not.toHaveBeenCalled(); expect(f.open).not.toHaveBeenCalled(); f.empty();
  for (const id of ['0', 's'.repeat(128)]) { f.c.subscribe(id); await f.tail.flush(); }
  expect(f.tail.stats().subscriptions).toBe(2);
  expect(f.open.mock.calls.every(args => args[0] === f.path)).toBe(true);
});
test('03 内核相对路径与 jsonl 路径拒绝且不打开文件', async () => {
  const f = await fixture();
  for (const path of ['relative.log', join(f.dir, 'session.jsonl')]) {
    f.state.path = path; f.c.subscribe(); await f.tail.flush();
    expect(f.c.messages.at(-1)).toMatchObject({ code: 'kernel', message: '内核返回的日志路径不合法' }); f.empty();
  }
  expect(f.open).not.toHaveBeenCalled();
});
test('04 第五订阅拒绝；连接隔离、退订释放、重复替换、待查询也占名额', async () => {
  const f = await fixture('x'); const gate = deferred();
  f.state.onCall = () => gate.promise;
  for (let i = 0; i < 5; i++) f.c.subscribe(`s${i}`);
  expect(f.c.messages).toEqual([{ type: 'log.rejected', session_id: 's4', code: 'limit', message: '每个连接最多同时订阅 4 个日志，请先收起其它日志' }]);
  f.c.subscribe('s0');
  const other = f.client(); other.subscribe('s4');
  expect(f.tail.stats().subscriptions).toBe(5);
  f.c.send({ type: 'log.unsubscribe', session_id: 's1' }); f.c.subscribe('s4');
  gate.resolve(); await f.tail.flush();
  expect(f.c.chunks().filter(m => m.session_id === 's0')).toHaveLength(1);
  expect(f.c.chunks().filter(m => m.session_id === 's4')).toHaveLength(1);
  expect(other.text()).toBe('x'); expect(f.tail.stats().subscriptions).toBe(5);
});
test('05 150 KiB 中文增量按整条编码限长且首尾相接', async () => {
  const f = await fixture(); f.c.subscribe(); await f.tail.flush();
  const content = '中文\n'.repeat(22_000);
  await appendFile(f.path, content); await f.tail.flush();
  expect(f.c.chunks().length).toBeGreaterThanOrEqual(3);
  let next = 0;
  for (const chunk of f.c.chunks()) {
    expect(chunk.offset).toBe(next); next = chunk.next;
    expect(Buffer.byteLength(JSON.stringify(chunk))).toBeLessThanOrEqual(65536);
  }
  expect(next).toBe(Buffer.byteLength(content)); expect(f.c.text()).toBe(content);
});
test('06 先查结束再读尾行；end 紧随最后 chunk 并释放资源', async () => {
  const f = await fixture('头'); f.c.subscribe(); await f.tail.flush();
  f.state.onCall = async () => { await appendFile(f.path, '尾行'); f.state.running = false; };
  await f.tail.flush();
  expect(f.c.messages.slice(-2)).toEqual([
    { type: 'log.chunk', session_id: 'ses-a', offset: 3, next: 9, text: '尾行' },
    { type: 'log.end', session_id: 'ses-a', offset: 9, reason: 'ended', message: '会话已结束' },
  ]); f.empty();
});
test('07 历史全部内容结束后名额释放；空历史也结束', async () => {
  const f = await fixture('历史全文', false); f.c.subscribe(); await f.tail.flush();
  expect(f.c.text()).toBe('历史全文'); expect(f.c.messages.at(-1)).toMatchObject({ reason: 'ended', offset: 12 }); f.empty();
  await writeFile(f.path, ''); f.c.subscribe(); await f.tail.flush();
  expect(f.c.messages.at(-1)).toMatchObject({ reason: 'ended', offset: 0 }); f.empty();
  f.state.running = true;
  for (let i = 0; i < 4; i++) f.c.subscribe(`s${i}`);
  await f.tail.flush(); expect(f.tail.stats().subscriptions).toBe(4);
});
test('08 inode 替换、截短、偏移超长和内核换路径都 rotated 并关句柄', async () => {
  for (const kind of ['inode', 'truncate', 'offset', 'path']) {
    const f = await fixture('abcd'); f.c.subscribe('ses-a', kind === 'offset' ? Number.MAX_SAFE_INTEGER : 0); await f.tail.flush();
    if (kind === 'inode') { await rename(f.path, join(f.dir, 'old.log')); await writeFile(f.path, 'newer'); }
    if (kind === 'truncate') await truncate(f.path, 1);
    if (kind === 'path') f.state.path = join(f.dir, 'new.log');
    await f.tail.flush(); expect(f.c.messages.at(-1)).toMatchObject({ reason: 'rotated' }); f.empty();
  }
});
test('09 删除和历史缺文件结束；运行中文件未创建等待且从零读', async () => {
  const f = await fixture('x'); f.c.subscribe(); await f.tail.flush(); await rm(f.path); await f.tail.flush();
  expect(f.c.messages.at(-1)).toMatchObject({ reason: 'deleted' }); f.empty();
  f.state.running = false; f.c.subscribe(); await f.tail.flush();
  expect(f.c.messages.at(-1)).toMatchObject({ reason: 'deleted' }); f.empty();
  f.state.running = true; f.c.send({ type: 'log.subscribe', session_id: 'new' }); await f.tail.flush();
  expect(f.tail.stats().subscriptions).toBe(1);
  await writeFile(f.path, '刚创建'); await f.tail.flush();
  expect(f.c.chunks().at(-1)).toMatchObject({ session_id: 'new', offset: 0, text: '刚创建' });
});
test('10 查询失败映射 not_found、offline、kernel 且释放名额', async () => {
  const f = await fixture();
  for (const [error, code, message] of [
    [Object.assign(new Error('找不到会话 ses-a'), { code: 'not_found' }), 'not_found', '找不到会话'],
    [new Error('内核未连接：重启中'), 'offline', '内核离线'], [new Error('失败'), 'kernel', '查询日志失败'], [new Error('调用超时'), 'kernel', '查询日志失败'],
  ] as const) {
    f.state.onCall = async () => { throw error; }; f.c.subscribe(); await f.tail.flush();
    expect(f.c.messages.at(-1)).toMatchObject({ code, message: expect.stringContaining(message) }); f.empty();
  }
  expect(f.open).not.toHaveBeenCalled();
});
test('11 断连释放全部，内核晚到不打开、不发送', async () => {
  const f = await fixture('x'); f.c.subscribe(); await f.tail.flush();
  const gate = deferred(); f.state.onCall = () => gate.promise;
  f.c.subscribe('pending'); const pending = f.tail.flush(); f.c.disconnect();
  const opened = f.open.mock.calls.length; const count = f.c.messages.length;
  gate.resolve(); await pending;
  expect(f.open).toHaveBeenCalledTimes(opened); expect(f.c.messages).toHaveLength(count); f.empty();
});
test('12 退订不再推送；不存在的退订无副作用；非法退订拒绝', async () => {
  const f = await fixture('x'); f.c.subscribe(); await f.tail.flush();
  f.c.send({ type: 'log.unsubscribe', session_id: 'missing' }); expect(f.tail.stats().subscriptions).toBe(1);
  f.c.send({ type: 'log.unsubscribe', session_id: 'ses-a', path: f.path });
  expect(f.c.messages.at(-1)).toMatchObject({ code: 'invalid' }); expect(f.tail.stats().subscriptions).toBe(1);
  f.c.send({ type: 'log.unsubscribe', session_id: 'ses-a' }); f.empty();
  const count = f.c.messages.length; await appendFile(f.path, 'later'); await f.tail.flush(); expect(f.c.messages).toHaveLength(count);
});
test('13 close 等待读取关闭，注销监听，之后订阅忽略', async () => {
  const f = await fixture('x'); const entered = deferred(); const release = deferred();
  f.state.onRead = async () => { entered.resolve(); await release.promise; };
  f.c.subscribe(); await entered.promise;
  expect(f.handles()).toBe(1);
  let closed = false; const closing = f.tail.close().then(() => { closed = true; });
  await Promise.resolve(); expect(closed).toBe(false);
  release.resolve(); await closing; f.empty();
  expect(f.c.messages).toEqual([]);
  f.c.subscribe(); await f.tail.flush(); expect(f.call).toHaveBeenCalledTimes(1); expect(f.c.messages).toEqual([]);
  await f.tail.close(); f.empty();
});
test('14 状态失败继续读取；恢复结束后不再查状态', async () => {
  const f = await fixture(); f.c.subscribe(); await f.tail.flush();
  f.state.onCall = async () => { throw new Error('内核未连接'); };
  await appendFile(f.path, '离线期间'); await f.tail.flush();
  expect(f.c.text()).toBe('离线期间'); expect(f.log).toHaveBeenCalledWith(expect.stringContaining('[日志] 查询会话状态失败'));
  expect(f.tail.stats().subscriptions).toBe(1);
  f.state.onCall = async () => {}; f.state.running = false; await f.tail.flush(); f.empty();
  const calls = f.call.mock.calls.length; await f.tail.flush(); expect(f.call).toHaveBeenCalledTimes(calls);
});
test('15 新连接带 next 续传不重复不遗漏', async () => {
  const f = await fixture('第一段'); f.c.subscribe(); await f.tail.flush();
  const next = f.c.chunks().at(-1)!.next; f.c.disconnect(); await appendFile(f.path, '第二段');
  const other = f.client(); other.subscribe('ses-a', next); await f.tail.flush();
  expect(other.chunks()[0].offset).toBe(next); expect(f.c.text() + other.text()).toBe('第一段第二段');
});
test('16 1 MiB 多轮读取每轮不超过 256 KiB 且编码消息限长', async () => {
  const f = await fixture(); f.c.subscribe(); await f.tail.flush();
  const content = 'a'.repeat(1024 * 1024); await appendFile(f.path, content);
  for (let i = 0; i < 4; i++) {
    const before = f.c.text().length; await f.tail.flush(); expect(f.c.text().length - before).toBeLessThanOrEqual(256 * 1024);
  }
  expect(f.c.text()).toBe(content);
  expect(f.reads.every(bytes => bytes <= 256 * 1024)).toBe(true);
  for (const m of f.c.messages) expect(Buffer.byteLength(JSON.stringify(m))).toBeLessThanOrEqual(65536);
});
test('17 残缺 UTF-8 结束时替换并结束；运行时等补齐', async () => {
  for (const prefix of ['', 'ab']) {
    const f = await fixture(Buffer.concat([Buffer.from(prefix), Buffer.from([0xe4, 0xb8])]), false);
    f.c.subscribe(); await f.tail.flush();
    expect(f.c.messages).toEqual([
      { type: 'log.chunk', session_id: 'ses-a', offset: 0, next: prefix.length + 2, text: prefix + '\uFFFD' },
      { type: 'log.end', session_id: 'ses-a', offset: prefix.length + 2, reason: 'ended', message: '会话已结束' },
    ]); f.empty();
  }
  const f = await fixture(Buffer.from([97, 98, 0xe4, 0xb8])); f.c.subscribe(); await f.tail.flush();
  expect(f.c.chunks()).toEqual([{ type: 'log.chunk', session_id: 'ses-a', offset: 0, next: 2, text: 'ab' }]);
  await appendFile(f.path, Buffer.from([0xad])); await f.tail.flush();
  expect(f.c.chunks().at(-1)).toMatchObject({ offset: 2, next: 5, text: '中' }); expect(f.tail.stats().subscriptions).toBe(1);
});
test('18 open/stat/read 失败各发 error 并关闭实际句柄', async () => {
  for (const stage of ['open', 'stat', 'read']) {
    const f = await fixture('x'); f.state.fail = stage; f.c.subscribe(); await f.tail.flush();
    expect(f.c.messages).toEqual([{ type: 'log.end', session_id: 'ses-a', offset: 0, reason: 'error', message: `读取日志失败：${stage === 'open' ? 'EACCES' : 'EIO'}` }]); f.empty();
  }
});
test('19 读取中退订、断连、替换丢弃旧结果且同一订阅不重入', async () => {
  for (const action of ['unsubscribe', 'disconnect', 'replace']) {
    const f = await fixture('x'); const entered = deferred(); const release = deferred();
    f.state.onRead = async () => { entered.resolve(); await release.promise; };
    f.c.subscribe(); await entered.promise;
    const pending = f.tail.flush(); const concurrent = f.tail.flush();
    expect(f.open).toHaveBeenCalledTimes(1);
    if (action === 'unsubscribe') f.c.send({ type: 'log.unsubscribe', session_id: 'ses-a' });
    if (action === 'disconnect') f.c.disconnect();
    if (action === 'replace') f.c.subscribe('ses-a', 1);
    release.resolve(); await Promise.all([pending, concurrent]); await f.tail.flush();
    expect(f.c.messages).toEqual([]); expect(f.handles()).toBe(0);
    if (action !== 'replace') f.empty();
  }
});
test('20 满预算后台立即续读，无须等待定时器，历史多轮只查一次状态', async () => {
  const f = await fixture('a'.repeat(1024 * 1024), false);
  const ended = deferred(); const original = f.c.messages.push.bind(f.c.messages);
  f.c.messages.push = (...messages) => { const result = original(...messages); if (messages.some(m => m.type === 'log.end')) ended.resolve(); return result; };
  f.c.subscribe();
  await ended.promise; await f.tail.flush();
  // end is sent inside the read; close waits for its finally block as well.
  await f.tail.close(); f.empty();
  expect(f.c.text()).toBe('a'.repeat(1024 * 1024)); expect(f.call).toHaveBeenCalledTimes(1);
});
test('21 共享轮询按 500ms 读增量，生命周期按 1000ms 查，结束清定时器', async () => {
  vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval', 'Date'] });
  const f = await fixture('', true, { intervalMs: 500, statusEveryMs: 1000 });
  try {
    f.c.subscribe(); await f.tail.flush();
    expect(vi.getTimerCount()).toBe(1); expect(f.call).toHaveBeenCalledTimes(1);
    await appendFile(f.path, 'tick');
    vi.advanceTimersByTime(500); await f.tail.flush();
    expect(f.c.text()).toBe('tick'); expect(f.call).toHaveBeenCalledTimes(1);
    f.state.running = false;
    vi.advanceTimersByTime(500); await f.tail.flush();
    expect(f.call).toHaveBeenCalledTimes(2); expect(f.c.messages.at(-1)).toMatchObject({ reason: 'ended' });
    f.empty(); expect(vi.getTimerCount()).toBe(0);
  } finally { await f.tail.close(); vi.useRealTimers(); }
});
