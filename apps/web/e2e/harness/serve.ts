import { setTimeout as delay } from 'node:timers/promises';
import { writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { startServer } from '../../server/main.ts';
import { checkout, type Fixture } from './fixture.ts';
const [root, projectId] = process.argv.slice(2);
if (!root || !projectId) throw Error('Private root and project required');
const service = await startServer({ mode: 'production', port: 0, home: join(root, 'autome-home'), userHome: root,
  binaryPath: join(checkout, 'target/debug/autome'), binaryArgs: ['core'], tickIntervalMs: 1000,
  kernelEnv: { HOME: root, AUTOMED_CLAUDE_BINARY: join(root, 'bin/fake-cli'), AUTOMED_CODEX_BINARY: join(root, 'bin/fake-cli') },
});
const deadline = Date.now() + 10_000;
while (!service.core.isConnected()) {
  if (Date.now() >= deadline) { await service.close(); throw Error('Core readiness timed out'); }
  await delay(50);
}
await service.core.call('project.list', {});
const address = service.server.address();
if (!address || typeof address === 'string') throw Error('Missing server port');
writeFileSync(join(root, 'state.json'), JSON.stringify({ root, home: join(root, 'autome-home'), repo: join(root, 'repo'),
  projectId, url: `http://127.0.0.1:${address.port}`, pid: process.pid } satisfies Fixture));
let closing = false;
for (const signal of ['SIGINT', 'SIGTERM'] as const) process.on(signal, () => {
  if (closing) return;
  closing = true;
  void service.close().catch(error => { console.error(error); process.exitCode = 1; });
});
