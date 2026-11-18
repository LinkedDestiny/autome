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
it('标签顺序、证据分组以及 Bug 与已完成不再显示过时说明', () => {
  expect(docsView(data).tabs.map(tab => tab.label)).toEqual(['任务卡', '设计文档', '运行记录']);
  expect(render()).toContain('证据（1）'); expect(docsView(data).evidence[0].label).toBe('a.md');
  expect(render({ ...data, documents: [], task: { ...data.task, kind: 'bug' } })).not.toContain('内核尚未提供 Bug 文档');
  expect(render({ ...data, documents: [], task: { ...data.task, state: { state: 'done' } } })).not.toContain('工作目录已清理');
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
  expect(html).toContain('doc=%E4%B8%AD%E6%96%87+%26.md'); expect(html).not.toContain('/private/secret');
});
it('拒绝路径、清单缺失、读盘失败三类错误原样呈现', () => {
  for (const error of ['已拒绝：文档名不能是路径', '这个任务没有名为 x 的文档', '文档已不在（可能刚被合并清理），刷新后重试', 'EACCES: permission denied']) {
    expect(render(data, { name: 'x', error, nodes: [] })).toContain(`<p role="alert">${error}</p>`);
  }
});
it('任务卡并入首标签，Bug 文档与复盘教训照常可选', () => {
  const payload = { ...data, documents: [...data.documents!, { name: 'card.md', label: '任务卡', absolute: '/card' }, { name: 'bug.md', label: 'Bug 文档', absolute: '/bug' }, { name: 'lessons.md', label: '复盘教训', absolute: '/lessons' }] };
  expect(docsView(payload).tabs.map(t => t.label)).toEqual(['任务卡', '设计文档', '运行记录', 'Bug 文档', '复盘教训']);
  expect(render(payload)).not.toContain('?doc=card.md');
});
it('仅取消任务空清单提示未合并', () => {
  for (const state of ['cancelled', 'done', 'draft', 'active']) for (const documents of [[], data.documents!]) {
    expect(render({ ...data, documents, task: { ...data.task, state: { state } } }).includes('任务已取消，文档没有合并到默认分支。')).toBe(state === 'cancelled' && documents.length === 0);
  }
});
it('默认分支来源标注，主检出读盘失败仍显示原错误', async () => {
  for (const source of ['worktree', 'default_branch'] as const) {
    const payload = { ...data, documents: data.documents!.map(d => ({ ...d, source })) };
    const read = vi.fn().mockResolvedValue({ status: 200, body: { ok: true, payload } });
    const docRead = vi.fn().mockResolvedValue({ error: '文档已不在（可能刚被合并清理），刷新后重试' });
    const result = await loadTaskDetail({ gateway: { read, write: vi.fn() }, taskDocs: { read: docRead } }, 'T-1', new URL('http://localhost/tasks/T-1?doc=feat.md'));
    const html = render(payload, result.document);
    expect(html.includes('来自默认分支（主检出）')).toBe(source === 'default_branch');
    expect(html).toContain('role="alert"'); expect(docRead).toHaveBeenCalledWith('/kernel/design', 204800);
  }
});

import { readFileSync } from 'node:fs';
it('38 份证据用胶囊入口展开，链接没有额外上下内边距', () => {
 const payload={...data,documents:Array.from({length:38},(_,i)=>({name:`evidence-${i}.md`,label:'证据',absolute:`/evidence/${i}`}))};
 const html=render(payload); expect(html).toContain('<summary class="pill pill-blue">证据（38）</summary>');
 expect(html.match(/>evidence-\d+\.md<\/a>/g)).toHaveLength(38);
 expect(render(payload,{name:'evidence-37.md',text:'',nodes:[],truncated:false})).toContain('<details open="">');
 const css=readFileSync(new URL('../app/styles/app.css',import.meta.url),'utf8');
 expect(css).not.toMatch(/\.task-docs nav a\s*\{[^}]*padding/);
 expect(css).not.toMatch(/\.task-docs summary\s*\{[^}]*padding/);
 expect(css).toMatch(/\.task-docs nav\s*\{[^}]*align-items: center/);
 expect(css).toMatch(/\.task-docs nav details\[open\]\s*\{[^}]*flex-basis: 100%/);
});
