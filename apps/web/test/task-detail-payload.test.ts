import { readFileSync } from 'node:fs';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createRoutesStub } from 'react-router';
import { expect, it, vi } from 'vitest';
import { loadTaskDetail } from '../app/components/task-detail-data.ts';
import { loadShell } from '../app/components/shell-data.ts';
import TaskPage from '../app/routes/tasks.$id.tsx';
import type { KernelGateway } from '../app/load-context.ts';
import type { TaskEvent } from '../server/task-history.ts';

export function fixture(count = 60) {
  const original = JSON.parse(readFileSync(new URL('./fixtures/w6-task-get.json', import.meta.url), 'utf8'));
  const milestones = Array.from({ length: 30 }, (_, i) => ({ id: `M-${i + 1}`, state: 'open', title: `实现第 ${i + 1} 个里程碑并独立复验` }));
  const at = (i: number) => new Date(Date.UTC(2026, 10, 14, 0, i)).toISOString();
  const sessions = Array.from({ length: count }, (_, i) => ({ ...original.sessions[0], id: `session-${i}`, label: '实现', round: i + 1, model: 'gpt-6-astra', started_at: at(i * 2), ended_at: at(i * 2 + 1), metrics: { cost_usd: 1, input_tokens: 20000, output_tokens: 3000, duration_ms: 60000, design_doc_bytes: 30000, evidence_files: 4 } }));
  const data = { ...original, task: { ...original.task, id: 'T-1', kind: 'feature', state: { state: 'active', node: 'implement' } }, sessions, brief: '# 任务卡\n验收详情页', status_block: { milestones, milestones_done: 0, milestones_total: 30, design_round: 1, design_round_limit: 15, impl_round: 60, impl_round_limit: 100 }, documents: Array.from({ length: 63 }, (_, i) => ({ name: `document-${i}.md`, label: i < 3 ? '设计文档' : '证据', absolute: `/private/tmp/worktree/docs/evidence/document-${i}.md`, source: 'worktree' })) };
  const events: TaskEvent[] = sessions.flatMap((session, i) => ['session.started', 'task.snapshot', 'task.updated', 'gate.queued', 'gate.started', 'gate.result', 'task.snapshot', 'session.ended'].map((kind, j) => ({ seq: i * 8 + j + 1, at: j === 0 ? session.started_at : session.ended_at, kind, subject: 'T-1', payload: kind === 'task.snapshot' ? { milestones: milestones.map(m => [m.id, m.state]), status_block: data.status_block } : { session_id: session.id, node: 'gate', job: 'gate', position: 1, ok: true, exit_code: 0, end: 'exited', trigger: 'session_ended', state: { state: 'active', node: 'implement' } } })));
  const read = vi.fn<KernelGateway['read']>(async request => {
    const { method } = request as { method: string };
    const payload = method === 'task.get' ? data : method === 'project.list' ? { projects: [{ project: { id: 'P-1', display_name: '项目' } }] } : method === 'project.get' ? { tasks: [data.task] } : {};
    return { status: 200, body: { ok: true, payload } };
  });
  return { data, events, services: { gateway: { read, write: vi.fn() }, taskEvents: { forTask: vi.fn().mockResolvedValue({ events, truncated: false }) } } };
}
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
