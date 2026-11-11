import { expect, test } from 'vitest';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { configureFailureRepo } from './fixtures/notifier-failure.ts';

test('live fixture 首次退出 3 即产生终态 failed，不进入默认重试等待', async () => {
  const root = mkdtempSync(join(tmpdir(), 'ntf-fixture-'));
  const home = join(root, 'home'), repo = join(root, 'repo'), fake = join(root, 'fake-cli');
  const binary = resolve(import.meta.dirname, '../../../target/debug/autome');
  const env = { ...process.env, HOME: home, AUTOME_HOME: home, AUTOMED_CLAUDE_BINARY: fake, AUTOMED_CODEX_BINARY: fake };
  const command = (file: string, args: string[]) => promisify(execFile)(file, args, { cwd: repo, env, timeout: 15000 });
  try {
    mkdirSync(home); mkdirSync(repo);
    writeFileSync(fake, '#!/bin/sh\ncat >/dev/null\nexit 3\n', { mode: 0o755 });
    configureFailureRepo(repo);
    await command('git', ['init', '-q', '-b', 'main']);
    await command('git', ['-c', 'user.name=test', '-c', 'user.email=t@t', 'commit', '--allow-empty', '-qm', 'init']);
    await command(binary, ['project', 'add', repo, '--skip-onboarding']);
    await command(binary, ['task', 'new', 'first fail', '-p', repo]);
    await command(binary, ['run', '--until-idle', '--interval', '0.2']);
    const { stdout } = await command(binary, ['rpc', 'events.since', '{"after_seq":0,"limit":1000}']);
    const events = JSON.parse(stdout).events as { kind: string; payload: { state?: { state: string } } }[];
    expect(events.filter(event => event.kind === 'session.retry')).toHaveLength(0);
    expect(events.filter(event => event.kind === 'task.updated' && event.payload.state?.state === 'failed')).toHaveLength(1);
  } finally { rmSync(root, { recursive: true, force: true }); }
}, 30000);
