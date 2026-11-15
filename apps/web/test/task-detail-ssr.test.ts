import { pathToFileURL } from 'node:url';
import { resolve } from 'node:path';
import { createRequestHandler, type ServerBuild } from 'react-router';
import { beforeAll, expect, it } from 'vitest';
import { createLoadContext } from '../app/load-context.ts';
import { fixture } from './fixtures/task-detail-60.ts';
import { ensureBuilt } from './fixtures/ensure-built.ts';

let build: ServerBuild;
beforeAll(async () => {
  ensureBuilt();
  build = await import(pathToFileURL(resolve('build/server/index.js')).href);
});

async function measure(path: string, count = 60) {
  const { services } = fixture(count);
  const response = await createRequestHandler(build, 'production')(
    new Request(`http://localhost${path}`), createLoadContext(services)(),
  );
  const body = await response.arrayBuffer();
  const text = new TextDecoder().decode(body);
  console.info(`SSR ${path} count=${count}: status=${response.status}, bytes=${body.byteLength}`);
  expect(response.status).toBe(200);
  // The last round distinguishes a rendered task from a small shell/error response.
  expect(text).toContain(`实现会话-${count}`);
  expect(body.byteLength).toBeLessThanOrEqual(150 * 1024);
}
it('生产构建 60 会话首次 SSR 文档不超过 150 KB', async () => { await measure('/tasks/T-1'); });
it('同 URL 的单次数据响应不超过 150 KB', async () => { await measure('/tasks/T-1.data'); });
it('钉住窗口后新增五会话的 SSR 文档不超过 150 KB', async () => { await measure('/tasks/T-1?from=session-40', 65); });
