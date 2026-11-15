import { execFileSync } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, expect, test, vi } from 'vitest';
import { checkout, prepare } from '../e2e/harness/fixture.ts';

vi.mock('node:child_process', () => ({ execFileSync: vi.fn() }));
const roots: string[] = [];
afterEach(() => {
  vi.resetAllMocks();
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

function fixture(duration: number, failure?: Error) {
  const root = mkdtempSync(join(tmpdir(), 'e2e-fixture-budget-'));
  roots.push(root);
  vi.mocked(execFileSync).mockImplementation((file, args, options) => {
    if (file === 'bash') {
      expect(args).toEqual([join(checkout, 'apps/web/test/fixtures/w7-flow.sh'), root, checkout]);
      if (failure) throw failure;
      if (duration > (options?.timeout ?? 0)) throw Object.assign(new Error('spawnSync bash ETIMEDOUT'), { code: 'ETIMEDOUT' });
      return Buffer.from('');
    }
    if (args?.[0] === 'rpc') return JSON.stringify({ projects: [{ project: { id: 'P-1' } }] });
    return JSON.stringify({ task: { id: 'T-1' } });
  });
  return root;
}

// Model elapsed subprocess time without waiting minutes or starting a kernel.
test.each([60_001, 150_000])('shared W7 flow completing at %i ms reaches card seeding', duration => {
  expect(prepare(fixture(duration))).toBe('P-1');
  expect(execFileSync).toHaveBeenCalledTimes(92);
});

test('W7 flow exceeding 150 seconds still times out before card seeding', () => {
  expect(() => prepare(fixture(150_001))).toThrow('spawnSync bash ETIMEDOUT');
  expect(execFileSync).toHaveBeenCalledTimes(1);
});

test('W7 flow failure is preserved and stops card seeding', () => {
  const error = new Error('fixture command failed');
  expect(() => prepare(fixture(0, error))).toThrow(error);
  expect(execFileSync).toHaveBeenCalledTimes(1);
});
