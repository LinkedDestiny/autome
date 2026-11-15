import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createRoutesStub } from 'react-router';
import { expect, it } from 'vitest';
import { loadTaskDetail } from '../app/components/task-detail-data.ts';
import { loadShell } from '../app/components/shell-data.ts';
import TaskPage from '../app/routes/tasks.$id.tsx';
import { fixture } from './fixtures/task-detail-60.ts';

async function measure(query = '', count = 60) {
  const { services } = fixture(count);
  const data = await loadTaskDetail(services, 'T-1', new URL(`http://localhost/tasks/T-1${query}`));
  const shell = await loadShell(services.gateway);
  const Stub = createRoutesStub([{ id: 'task', path: '/tasks/:id', Component: TaskPage }]);
  const html = renderToStaticMarkup(createElement(Stub, { initialEntries: [`/tasks/T-1${query}`], hydrationData: { loaderData: { task: data } } }));
  const bytes = Buffer.byteLength(JSON.stringify(data)), document = bytes + Buffer.byteLength(JSON.stringify(shell)) + Buffer.byteLength(html);
  console.info(`detail ${query || 'default'} count=${count}: data=${bytes}, document=${document}`);
  return { data, bytes, document, html };
}
it('60 会话首次响应不超过 150 KB', async () => { expect((await measure()).document).toBeLessThanOrEqual(150 * 1024); });
it('同一 loader 重验和钉住后新增五会话不超过 150 KB', async () => { for (const count of [60, 65]) expect((await measure('?from=session-40', count)).bytes).toBeLessThanOrEqual(150 * 1024); });
it('timeline=all 显示全部 60 会话', async () => { expect((await measure('?timeline=all')).html.match(/data-testid="session-log"/g)).toHaveLength(60); });
it('输出没有绝对路径字段或原始事件', async () => { expect(JSON.stringify((await measure()).data)).not.toMatch(/"absolute"|"log_path"|"events"|"task.snapshot"/); });
