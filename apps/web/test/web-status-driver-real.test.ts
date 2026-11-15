import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { test, expect } from 'vitest';
import { harness, cli } from './fixtures/web-launcher.ts';
import { startServer } from '../server/main.ts';
test('真实 run 与面板共存，status 明确报告驱动，退出后消失', async () => {
  const h = harness();
  let server: Awaited<ReturnType<typeof startServer>> | undefined;
  let child: ReturnType<typeof spawn> | undefined;
  try {
    child = spawn(cli, ['run', '--interval', '1'], { env: h.env, stdio: 'ignore' });
    const exited = once(child, 'exit');
    server = await startServer({ port: 0, mode: 'production', home: h.home, userHome: h.root, binaryPath: cli, binaryArgs: ['core'], instanceFile: true });
    await expect.poll(async () => (await h.run(['status'])).stdout, { timeout: 10000 }).toContain('另一个驱动在推进：pid ' + child!.pid);
    const text = (await h.run(['status'])).stdout;
    expect(text).toContain('autome run');
    child.kill('SIGTERM'); await exited;
    await expect.poll(async () => (await h.run(['status'])).stdout).toContain('没有其他驱动');
  } finally {
    if (child && child.exitCode === null && child.signalCode === null) { const exited = once(child, 'exit'); child.kill('SIGKILL'); await exited; }
    await server?.close();
    await h.cleanup();
  }
});
