import { execFileSync } from 'node:child_process';
import { existsSync, mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { afterEach, expect, test } from 'vitest';
import { startServer } from '../server/main.ts';
import type { CoreSupervisor } from '../server/core/supervisor.ts';
import { configureFailureRepo } from './fixtures/notifier-failure.ts';

const cleanups: (() => Promise<void>)[] = [];
afterEach(async () => { for (const cleanup of cleanups.splice(0).reverse()) await cleanup(); });

async function setup() {
  const root = mkdtempSync(join(tmpdir(), 'gateway-lifecycle-'));
  const home = join(root, 'home'), repo = join(root, 'repo'), fake = join(root, 'fake-cli');
  const keys = ['HOME', 'AUTOMED_CLAUDE_BINARY', 'AUTOMED_CODEX_BINARY'] as const;
  const previous = keys.map(key => process.env[key]);
  cleanups.push(async () => {
    keys.forEach((key, index) => {
      if (previous[index] === undefined) delete process.env[key];
      else process.env[key] = previous[index];
    });
    rmSync(root, { recursive: true, force: true });
  });
  mkdirSync(home); mkdirSync(repo);
  const protocol = readFileSync(resolve(import.meta.dirname, '../../../protocol/bug-protocol.md'), 'utf8');
  const block = protocol.match(/<!-- kernel-contract: bug-status-block -->\s*```text\n([\s\S]*?)```/);
  expect(block).not.toBeNull();
  writeFileSync(join(root, 'bug-doc.md'), block![1].replace(/^repos:.*\n/gm, '')
    .replace(/^status:.*$/m, 'status: 无法复现').replace(/^复核:.*$/m, '复核: 待复核')
    .replace(/^复现命令:.*$/m, '复现命令: sh regression.sh')
    + '\n## 现象\n需要补充复现步骤。\n## 复现\n尚缺环境。\n## 根因分析\n待补充。\n## 修复说明\n无。\n## 复核意见\n待复核。\n');
  // Resolve fixture files relative to this executable, without interpolating shell paths.
  writeFileSync(fake, `#!/bin/sh
set -eu
cat >/dev/null
root=$(dirname "$0")
printf '%s\\n' "$0" >> "$root/history"
slug=$(basename "$PWD")
mkdir -p "docs/$slug"
cp "$root/bug-doc.md" "docs/$slug/$slug-bug.md"
git add "docs/$slug/$slug-bug.md"
git -c user.name=fixture -c user.email=f@f commit --allow-empty -qm 'need info'
`, { mode: 0o755 });
  configureFailureRepo(repo);
  execFileSync('git', ['init', '-q', '-b', 'main'], { cwd: repo });
  execFileSync('git', ['-c', 'user.name=fixture', '-c', 'user.email=f@f', 'commit', '--allow-empty', '-qm', 'init'], { cwd: repo });
  process.env.HOME = home;
  process.env.AUTOMED_CLAUDE_BINARY = fake;
  process.env.AUTOMED_CODEX_BINARY = fake;
  expect(process.env.AUTOMED_CLAUDE_BINARY).toBe(fake);
  expect(process.env.AUTOMED_CODEX_BINARY).toBe(fake);
  const running = await startServer({ port: 0, mode: 'production', home,
    binaryPath: process.env.AUTOMED_BIN || resolve(import.meta.dirname, '../../../target/debug/automed') });
  cleanups.push(() => running.close());
  const address = running.server.address();
  if (!address || typeof address === 'string') throw new Error('无端口');
  const url = `http://127.0.0.1:${address.port}`;
  const post = (channel: 'read' | 'write', body: unknown) => fetch(`${url}/api/${channel}`, {
    method: 'POST', headers: { 'Content-Type': 'application/json', Origin: url }, body: JSON.stringify(body),
  });
  const core = running.gateway.core as CoreSupervisor;
  const { project } = await core.call('project.add', { path: repo }) as { project: { id: string } };
  await core.call('project.onboarding.skip', { project_id: project.id });
  const created = await post('write', { op: 'task.create', params: {
    project_id: project.id, kind: 'bug', brief: '# 生命周期验证\n请补充复现环境。', draft: true,
  } });
  expect(created.status).toBe(200);
  const task = (await created.json()).payload.task;
  expect(task.state.state).toBe('draft');
  expect(existsSync(join(root, 'history'))).toBe(false);
  const write = (op: string, params: Record<string, unknown> = {}) => post('write', { op, params: { task_id: task.id, ...params } });
  const read = async () => {
    const response = await post('read', { method: 'task.get', params: { task_id: task.id } });
    expect(response.status).toBe(200);
    return (await response.json()).payload.task;
  };
  const history = () => readFileSync(join(root, 'history'), 'utf8').trim().split('\n');
  const awaitInfo = async (sessions: number) => {
    await expect.poll(async () => {
      await core.call('scheduler.tick', {});
      return (await read()).state.node;
    }, { timeout: 20000, interval: 50 }).toBe('await_info');
    expect(history()).toEqual(Array.from({ length: sessions }, () => fake));
    return read();
  };
  const start = async () => {
    const response = await write('task.start');
    expect(response.status).toBe(200);
    expect(['queued', 'active']).toContain((await response.json()).payload.task.state.state);
    return awaitInfo(1);
  };
  return { write, read, start, awaitInfo, repo };
}

test('task.start 经网关开始 Bug 草稿，会话只落到替身', async () => {
  const s = await setup();
  await s.start();
});

test('task.reply 经网关把补充信息原样送达等待补充的 Bug', async () => {
  const s = await setup();
  const waiting = await s.start();
  const replies = join(s.repo, '.worktree', waiting.slug, 'docs', waiting.slug, `${waiting.slug}-replies.md`);
  for (const info of ['', ' \n\t']) {
    const refused = await s.write('task.reply', { info });
    expect(refused.status).toBe(409);
    expect((await refused.json()).error).toContain('补充信息不能为空');
    expect((await s.read()).state).toEqual(waiting.state);
    expect(existsSync(replies)).toBe(false);
  }
  const info = '  请保留  双空格\n中文 补充\n';
  const response = await s.write('task.reply', { info });
  expect(response.status).toBe(200);
  expect(readFileSync(replies, 'utf8')).toContain(info);
  await s.awaitInfo(2);
  expect(readFileSync(replies, 'utf8')).toContain(info);
});
