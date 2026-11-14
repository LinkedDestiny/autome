import { createElement } from 'react';
import { renderToString } from 'react-dom/server';
import { createMemoryRouter } from 'react-router';
import { readFileSync } from 'node:fs';
import { expect, it, vi } from 'vitest';
import type { CoreMessage } from '../server/eventbus-protocol.ts';
import { LiveBanner, liveBanner, writeGate, WriteGateProvider, useWriteGate, needsKernelReload, useKernelRecovery } from '../app/lib/live.tsx';
import { liveShouldRevalidate, byKind } from '../app/lib/live-routes.ts';
// Node 环境无 DOM：只替代 React effect/ref 调度与 useRevalidator，执行真实 hook 和路由重验。
const harness = vi.hoisted(() => ({ enabled: false, ref: { current: false }, effects: [] as (() => void)[], revalidate: async () => {} }));
vi.mock('react', async importOriginal => {
  const actual = await importOriginal<typeof import('react')>();
  return { ...actual, useRef: (...args: Parameters<typeof actual.useRef>) => harness.enabled ? harness.ref : actual.useRef(...args),
    useEffect: (...args: Parameters<typeof actual.useEffect>) => harness.enabled ? harness.effects.push(args[0]) : actual.useEffect(...args) };
});
vi.mock('react-router', async importOriginal => { const actual = await importOriginal<typeof import('react-router')>(); return { ...actual, useRevalidator: () => harness.enabled ? { revalidate: harness.revalidate } : actual.useRevalidator() }; });
const connected: CoreMessage = { type: 'core', status: 'connected' };
const offline: CoreMessage = { type: 'core', status: 'offline', reason: '服务不可达' };
it('横幅优先级和未知状态的 loader 错误回退', () => {
  expect(liveBanner({ connection: 'reconnecting', core: offline, kernelError: '超时' })?.text).toBe('连接中断，正在重连');
  expect(liveBanner({ connection: 'open', core: offline, kernelError: '超时' })?.text).toBe('内核离线：服务不可达，写操作暂不可用');
  for (const kernelError of ['拒绝授权', '读盘失败', '内核响应超时', '']) expect(liveBanner({ connection: 'connecting', kernelError })?.text).toBe(`内核离线：${kernelError}，写操作暂不可用`);
  expect(liveBanner({ connection: 'open', core: connected, kernelError: '旧错误' })).toBeNull();
  expect(liveBanner({ connection: 'connecting' })).toBeNull();
});
it('重连与离线横幅角色、类名与正文', () => {
  for (const connection of ['reconnecting', 'open'] as const) {
    const html = renderToString(createElement(LiveBanner, { connection, core: offline }));
    expect(html).toContain(`role="${connection === 'open' ? 'alert' : 'status'}"`); expect(html).toContain('class="live-banner"');
    expect(html).toContain(connection === 'open' ? '内核离线：服务不可达，写操作暂不可用' : '连接中断，正在重连');
  }
});
it('写判定覆盖 connection × core 全域及中文理由', () => {
  for (const connection of ['connecting', 'reconnecting', 'open'] as const) for (const core of [undefined, offline, connected]) {
    const gate = writeGate({ connection, core }); expect(gate.allowed).toBe(connection === 'open' && core === connected);
    expect(gate.reason).toBe(connection === 'connecting' ? '正在连接服务' : connection === 'reconnecting' ? '连接中断，正在重连' : core === connected ? '' : core === offline ? '内核离线：服务不可达' : '正在连接服务');
  }
});
it('open 尚未收到 core 时拒绝写', () => { expect(writeGate({ connection: 'open' }).allowed).toBe(false); });
it('共享 context 按钮 SSR 禁用与理由', () => {
  function Button() { const gate = useWriteGate(); return createElement('button', { disabled: !gate.allowed, title: gate.reason }, '写入'); }
  for (const core of [offline, connected]) {
    const gate = writeGate({ connection: 'open', core });
    const html = renderToString(createElement(WriteGateProvider, { value: gate }, createElement(Button)));
    expect(html.includes('disabled=""')).toBe(!gate.allowed); expect(html).toContain(`title="${gate.reason}"`);
  }
});
it('真实恢复 hook 在无账本事件下重验一次，首个 hello 与持续失败不循环', async () => {
  for (const core of [undefined, offline, connected]) for (const kernelOk of [true, false]) expect(needsKernelReload({ core, kernelOk })).toBe(core === connected && !kernelOk);
  const root = readFileSync(new URL('../app/root.tsx', import.meta.url), 'utf8'); expect(root).toMatch(/useKernelRecovery\(\{ core: live.core, kernelOk: kernel.ok \}\)/);
  for (const initialCore of [offline, connected]) for (const succeeds of [true, false]) {
    let calls = 0;
    const router = createMemoryRouter([{ id: 'root', path: '/', loader: () => ({ ok: ++calls > 1 && succeeds }), shouldRevalidate: liveShouldRevalidate(byKind('task.')) }]);
    await vi.waitFor(() => expect(router.state.initialized).toBe(true));
    harness.enabled = true; harness.ref = { current: false }; harness.revalidate = () => router.revalidate();
    vi.useFakeTimers();
    const render = async (core: CoreMessage) => {
      useKernelRecovery({ core, kernelOk: (router.state.loaderData.root as { ok: boolean }).ok });
      harness.effects.splice(0).forEach(effect => effect());
      const before = calls;
      await vi.advanceTimersByTimeAsync(999); expect(calls).toBe(before);
      await vi.advanceTimersByTimeAsync(1);
      await vi.waitFor(() => expect(router.state.revalidation).toBe('idle'));
    };
    try {
      await render(initialCore); await render(connected); await render(connected);
      expect(calls).toBe(2); expect(router.state.loaderData.root).toEqual({ ok: succeeds });
      await render(offline); await render(connected); expect(calls).toBe(succeeds ? 2 : 3);
    } finally { await vi.runOnlyPendingTimersAsync(); vi.useRealTimers(); harness.enabled = false; router.dispose(); }
  }
});
