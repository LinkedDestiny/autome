import { expect, test } from 'vitest';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { harness, processes } from './fixtures/web-launcher.ts';
import type { EnvSnapshot } from '../app/components/env-model.ts';
function setup() {
  const h = harness(); const bin = join(h.root, 'tools'); mkdirSync(bin);
  for (const name of ['claude', 'codex']) writeFileSync(join(bin, name), '#!/bin/sh\necho "1.0.0"\n', { mode: 0o755 });
  const shell = join(h.root, 'shell'); writeFileSync(shell, `#!/bin/sh\nprintf '%s' '__autome_path__${bin}:/usr/bin:/bin__autome_path__'\n`, { mode: 0o755 });
  h.env.SHELL = shell; h.env.PATH = '/usr/bin:/bin';
  delete h.env.AUTOMED_CLAUDE_BINARY; delete h.env.AUTOMED_CODEX_BINARY;
  return { h, bin };
}
test('最小 PATH 经启动器与登录 shell 后 env.get 找到 claude/codex', async () => {
  const { h, bin } = setup();
  try {
    const result = await h.run(['start']); expect(result.code, result.stderr).toBe(0); const v = await h.registration();
    await expect.poll(async () => {
      const response = await fetch(`${v.url}/api/read`, { method: 'POST', headers: { 'Content-Type': 'application/json', Origin: v.url }, body: JSON.stringify({ method: 'env.get', params: {} }) });
      const body = await response.json(); const snapshot = body.payload as EnvSnapshot;
      return snapshot?.environment?.components.filter(c => ['claude', 'codex'].includes(c.component)).map(c => ({ component: c.component, present: c.present, path: c.path }));
    }, { timeout: 20000 }).toEqual(['claude', 'codex'].map(component => ({ component, present: true, path: join(bin, component) })));
  } finally { await h.cleanup(); }
});
test('登录 shell、服务与内核无残留', async () => { const { h } = setup(); try { expect((await h.run(['start'])).code).toBe(0); } finally { await h.cleanup(); } expect(processes(h.root)).toEqual([]); });
