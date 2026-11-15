import { execFileSync, spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { expect, it } from 'vitest';
import { writeWindowScripts } from '../e2e/harness/window-task.ts';

it('C-2 的 23 个真实夹具步骤各自产生提交，不因相邻文档相同提前退出', () => {
  const root = mkdtempSync(join(tmpdir(), 'autome-window-'));
  try {
    const cwd = join(root, 'repo');
    mkdirSync(cwd);
    execFileSync('git', ['init', '-q', cwd]);
    execFileSync('git', ['config', 'core.hooksPath', '/dev/null'], { cwd });
    const scripts = writeWindowScripts(root, 'window-history');
    for (let step = 1; step <= 23; step++) {
      writeFileSync(join(root, `release-window-history-${step}`), 'go');
      const result = spawnSync('sh', [join(scripts, `${step}.sh`)], { cwd, encoding: 'utf8' });
      expect(result.status, `step ${step}: ${result.stderr}\n${result.stdout}`).toBe(0);
      expect(Number(execFileSync('git', ['rev-list', '--count', 'HEAD'], { cwd, encoding: 'utf8' }))).toBe(step);
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
});
