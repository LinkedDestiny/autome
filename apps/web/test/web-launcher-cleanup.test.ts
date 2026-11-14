import { afterAll, expect, test, vi } from 'vitest';
import { existsSync, rmSync } from 'node:fs';
import { harness } from './fixtures/web-launcher.ts';

// This regression exercises hook cleanup without starting or signalling processes.
vi.mock('node:child_process', async importOriginal => ({
  ...await importOriginal<typeof import('node:child_process')>(),
  execFileSync: vi.fn(() => '1 0 /sbin/launchd\n'),
}));

let fixture: ReturnType<typeof harness>;
test('夹具在 afterAll 中清理，不依赖测试体内的 expect.poll', () => {
  fixture = harness();
  expect(existsSync(fixture.root)).toBe(true);
});
afterAll(async () => {
  try {
    await fixture.cleanup();
    expect(existsSync(fixture.root)).toBe(false);
  } finally {
    rmSync(fixture.root, { recursive: true, force: true });
  }
});
