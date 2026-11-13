import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { MemoryRouter } from 'react-router';
import { expect, it, vi } from 'vitest';
import { TaskDocs } from '../app/components/task-docs.tsx';
import { docsView } from '../app/components/task-docs-model.ts';
import { loadTaskDetail, type TaskPayload } from '../app/components/task-detail-data.ts';
import { parseMarkdown } from '../app/lib/markdown.ts';
const data: TaskPayload = { task: { id: 'T-1', title: 'x', slug: 'feat', request: '需求原文' }, sessions: [], documents: [
  { name: 'feat.md', label: '设计文档', absolute: '/kernel/design' }, { name: 'a.md', label: '证据', absolute: '/kernel/evidence' }, { name: 'retro.md', label: '运行记录', absolute: '/kernel/retro' },
], status_block: { current_milestone: 'M-01', milestones_done: 0, milestones_total: 1, design_round: 1, design_round_limit: 2, impl_round: 1, impl_round_limit: 2 } };
type Doc = Awaited<ReturnType<typeof loadTaskDetail>>['document'];
const render = (payload: TaskPayload = data, document: Doc = { name: null, text: '', truncated: false, nodes: [] }) => renderToStaticMarkup(createElement(MemoryRouter, {}, createElement(TaskDocs, { data: payload, document })));
it('标签顺序、证据分组以及 Bug 与已完成说明', () => {
  expect(docsView(data).tabs.map(tab => tab.label)).toEqual(['任务卡', '设计文档', '运行记录']);
  expect(render()).toContain('证据（1）'); expect(docsView(data).evidence[0].label).toBe('a.md');
  expect(render({ ...data, documents: [], task: { ...data.task, kind: 'bug' } })).toContain('内核尚未提供 Bug 文档');
  expect(render({ ...data, documents: [], task: { ...data.task, state: { state: 'done' } } })).toContain('工作目录已清理');
});
it('仅设计文档里程碑章节表格当前行高亮', () => {
  const table = '| ID | 状态 |\n| --- | --- |\n| M-01 | 开放 |\n| M-02 | 已完成 |';
  const nodes = parseMarkdown(`## 其它\n${table}\n## 里程碑\n${table}\n## Backlog\n${table}`);
  for (const name of [null, 'feat.md', 'retro.md']) {
    const html = render(data, { name, nodes, text: '', truncated: false });
    expect((html.match(/class="is-current"/g) ?? []).length).toBe(name === 'feat.md' ? 1 : 0);
    if (name === 'feat.md') expect(html).toContain('aria-current="true"><td>M-01');
  }
});
it('缺任务卡正文回退原文，空字符串保留；截断提示', async () => {
  for (const brief of [null, '']) {
    const read = vi.fn().mockResolvedValue({ status: 200, body: { ok: true, payload: { ...data, brief } } });
    const result = await loadTaskDetail({ gateway: { read, write: vi.fn() } }, 'T-1', new URL('http://localhost/tasks/T-1'));
    expect(render(data, result.document).includes('这个任务没有任务卡正文')).toBe(brief === null);
    expect(render(data, result.document).includes('需求原文')).toBe(brief === null);
  }
  expect(render(data, { name: 'feat.md', text: 'x', nodes: parseMarkdown('x'), size: 204801, truncated: true })).toContain('文档共 201 KB，只显示前 200 KB');
});
it('标签使用编码后的名字，无路径', () => {
  const html = render({ ...data, documents: [{ name: '中文 &.md', absolute: '/private/secret', label: '评审' }] });
  expect(html).toContain('?doc=%E4%B8%AD%E6%96%87%20%26.md'); expect(html).not.toContain('/private/secret');
});
it('拒绝路径、清单缺失、读盘失败三类错误原样呈现', () => {
  for (const error of ['已拒绝：文档名不能是路径', '这个任务没有名为 x 的文档', '文档已不在（可能刚被合并清理），刷新后重试', 'EACCES: permission denied']) {
    expect(render(data, { name: 'x', error, nodes: [] })).toContain(`<p role="alert">${error}</p>`);
  }
});
