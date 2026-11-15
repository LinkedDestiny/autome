import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, expect, test, vi } from 'vitest';
import { checkout, flowSteps, prepare } from '../e2e/harness/fixture.ts';

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

function flowFixture(kind: 'feature' | 'bug' = 'feature') {
  const root = mkdtempSync(join(tmpdir(), 'e2e-flow-barrier-'));
  roots.push(root);
  const source = join(root, 'repo/.autome/fake', kind === 'feature' ? 'feat' : 'bugx');
  mkdirSync(source, { recursive: true });
  mkdirSync(join(root, 'bin'));
  for (const step of [1, 2]) writeFileSync(join(source, `${step}.sh`), `echo completed > '${root}/completed-${step}'\n`);
  return root;
}

// Execute the generated shell, using sleep calls as a deterministic observer.
// The observer releases only after seeing the body held across three polls.
test('feature first step waits for observation and then runs; later steps are free', async () => {
  const root = flowFixture();
  const { execFileSync: run } = await vi.importActual<typeof import('node:child_process')>('node:child_process');
  writeFileSync(join(root, 'bin/w7-sleep'), `#!/bin/sh
[ ! -f '${root}/completed-1' ] || exit 94
echo poll >> '${root}/polls'
[ "$(wc -l < '${root}/polls')" -lt 3 ] || touch '${root}/release-browser-feature-1'
`, { mode: 0o755 });
  flowSteps(root, 'browser-feature', 'feature', { holdFirstStep: true });
  run('sh', [join(root, 'repo/.autome/fake/browser-feature/1.sh')], { timeout: 5000 });
  expect(readFileSync(join(root, 'polls'), 'utf8').trim().split('\n')).toHaveLength(3);
  expect(existsSync(join(root, 'completed-1'))).toBe(true);
  run('sh', [join(root, 'repo/.autome/fake/browser-feature/2.sh')], { timeout: 5000 });
  expect(existsSync(join(root, 'completed-2'))).toBe(true);
});

test('unreleased feature barrier exits 93 without running the step body', async () => {
  const root = flowFixture();
  const { spawnSync } = await vi.importActual<typeof import('node:child_process')>('node:child_process');
  writeFileSync(join(root, 'bin/w7-sleep'), '#!/bin/sh\nexit 0\n', { mode: 0o755 });
  flowSteps(root, 'browser-feature', 'feature', { holdFirstStep: true });
  const result = spawnSync('sh', [join(root, 'repo/.autome/fake/browser-feature/1.sh')], { timeout: 15_000 });
  expect(result.error).toBeUndefined();
  expect(result.status).toBe(93);
  expect(existsSync(join(root, 'completed-1'))).toBe(false);
});

test.each(['feature', 'bug'] as const)('%s keeps its existing default barriers', kind => {
  const root = flowFixture(kind);
  flowSteps(root, 'other-flow', kind);
  for (const step of [1, 2]) {
    const script = readFileSync(join(root, `repo/.autome/fake/other-flow/${step}.sh`), 'utf8');
    expect(script.includes(`release-other-flow-${step}`)).toBe(kind === 'bug');
  }
});
