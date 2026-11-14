import { existsSync, rmSync } from 'node:fs';
import { afterEach, expect, test, vi } from 'vitest';
import { w9Kernel } from './fixtures/w9-replies.ts';

const mocks = vi.hoisted(() => ({ start: vi.fn(), stop: vi.fn(), rm: vi.fn() }));
vi.mock('../server/core/supervisor.ts', () => ({
  CoreSupervisor: class { start = mocks.start; stop = mocks.stop; },
}));
vi.mock('node:fs/promises', () => ({ rm: mocks.rm }));
const originalHome = process.env.HOME;
const roots: string[] = [];
afterEach(() => {
  if (originalHome === undefined) delete process.env.HOME; else process.env.HOME = originalHome;
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
  vi.resetAllMocks();
});

test('w9 cleanup waits for stop and bounded asynchronous removal', async () => {
  const fixture = await w9Kernel(); roots.push(fixture.root);
  let stopped!: () => void;
  mocks.stop.mockImplementation(() => new Promise<void>(resolve => { stopped = resolve; }));
  let removed!: () => void;
  mocks.rm.mockImplementation(() => new Promise<void>(resolve => { removed = resolve; }));
  let closed = false;
  const closing = fixture.close().then(() => { closed = true; });
  expect(mocks.rm).not.toHaveBeenCalled();
  stopped(); await Promise.resolve();
  expect(mocks.rm).toHaveBeenCalledWith(fixture.root, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
  expect(closed).toBe(false);
  removed(); await closing;
  expect(closed).toBe(true);
});

test('w9 persistent ENOTEMPTY propagates and preserves the directory', async () => {
  const fixture = await w9Kernel(); roots.push(fixture.root);
  const error = Object.assign(new Error('still busy'), { code: 'ENOTEMPTY' });
  mocks.rm.mockRejectedValue(error);
  await expect(fixture.close()).rejects.toBe(error);
  expect(existsSync(fixture.root)).toBe(true);
  expect(process.env.HOME).toBe(originalHome);
});

test('w9 failed startup also awaits bounded cleanup', async () => {
  const error = new Error('startup failed');
  mocks.start.mockRejectedValue(error);
  mocks.rm.mockImplementation(async (root: string) => { roots.push(root); });
  await expect(w9Kernel()).rejects.toBe(error);
  expect(mocks.stop).toHaveBeenCalledOnce();
  expect(mocks.rm).toHaveBeenCalledOnce();
  expect(process.env.HOME).toBe(originalHome);
});
