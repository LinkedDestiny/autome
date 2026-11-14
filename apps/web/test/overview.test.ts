import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createRoutesStub, MemoryRouter } from 'react-router';
import { expect, test } from 'vitest';
import { ProjectList } from '../app/components/project-list.tsx';
import { OverviewCard } from '../app/components/overview-card.tsx';
import Overview from '../app/routes/_index.tsx';
import { taskHref, projectHref, newTaskHref } from '../app/components/nav-links.ts';
import { AWAIT_PHRASES, FEATURE_STAGES, BUG_STAGES, STATE_LABELS, stageOf, type TaskState } from '../app/lib/labels.ts';
import type { ShellData, ShellTask } from '../app/components/shell-data.ts';
const task: ShellTask = { id: 'T-2', title: '检查卡片', kind: 'bug', project_id: 'P-1', state: { state: 'active', node: 'fix' } };
const shell: ShellData = { kernel: { ok: true }, projects: [{ id: 'P-1', name: '项目甲', counts: { running: 1, needs_you: 1 } }], tasks: [task, { ...task, id: 'T-10', state: { state: 'stopped', at: 'fix' } }] };
function overview(data = shell) {
  const Stub = createRoutesStub([{ id: 'root', path: '/', loader: () => data, Component: Overview }]);
  return renderToStaticMarkup(createElement(Stub, { hydrationData: { loaderData: { root: data } } }));
}
function card(state: TaskState, kind = 'bug') { return renderToStaticMarkup(createElement(MemoryRouter, null, createElement(OverviewCard, { task: { ...task, kind, state }, projectName: '项目甲' }))); }
test('1 project cards expose counts, encoded destinations, failed reads and empty state', () => {
  const html = renderToStaticMarkup(createElement(MemoryRouter, null, createElement(ProjectList, { projects: [...shell.projects, { id: 'a/b ?中', name: '未知项目', counts: null, error: '读盘失败' }, { id: 'P-0', name: '空项目', counts: { running: 0, needs_you: 0 } }] })));
  expect(html).toContain('aria-label="项目甲：进行中 1，等你处理 1"'); expect(html).toContain('href="/projects/P-1"');
  expect(html).toContain('href="/projects/a%2Fb%20%3F%E4%B8%AD"'); expect(html).toContain('title="读盘失败"'); expect(html).toContain('等你处理 —'); expect(html).toContain('count-zero');
  expect(renderToStaticMarkup(createElement(ProjectList, { projects: [] }))).toContain('还没有项目。可以用命令行');
});
test('2 overview columns, encoded card links, stages and numeric ordering', () => {
  const data = { ...shell, tasks: [...shell.tasks, { ...task, id: 'T-20' }, { ...task, id: 'a/b ?中' }] };
  const html = overview(data); expect(html).toContain('等你处理（1）'); expect(html).toContain('进行中（3）');
  expect(html).toContain('href="/tasks/a%2Fb%20%3F%E4%B8%AD"'); expect(html).toContain(stageOf('bug', task.state)!.label);
  expect(html.indexOf('href="/tasks/T-20"')).toBeLessThan(html.indexOf('href="/tasks/T-2"'));
});
test('3 cards show type, identity, project and every waiting or stopped status; empty columns are explicit', () => {
  const html = card(task.state); for (const text of ['Bug', 'T-2', '检查卡片', '项目甲', '修复']) expect(html).toContain(text);
  expect(card({ state: 'active', node: 'gate' }, 'feature')).toContain('需求');
  for (const [node, phrase] of Object.entries(AWAIT_PHRASES)) expect(card({ state: 'active', node })).toContain(phrase);
  for (const [state, label] of [['stopped', '已停止'], ['failed', '失败'], ['paused', '已暂停']]) expect(card({ state, at: 'fix', resume: 'fix' })).toContain(`修复 · ${label}`);
  const empty = overview({ ...shell, tasks: [] }); expect(empty).toContain('没有需要你处理的任务'); expect(empty).toContain('没有正在进行的任务');
});
test('4 offline renders the unavailable message in both columns', () => { expect(overview({ ...shell, kernel: { ok: false }, projects: [], tasks: [] }).match(/内核离线，暂时拿不到任务/g)).toHaveLength(2); });
test('5 navigation encodes all three targets including empty and reserved IDs', () => {
  for (const id of ['a/b ?中', '', 'T-01']) {
    const encoded = id === 'a/b ?中' ? 'a%2Fb%20%3F%E4%B8%AD' : id;
    expect(taskHref(id)).toBe(`/tasks/${encoded}`); expect(projectHref(id)).toBe(`/projects/${encoded}`); expect(newTaskHref(id)).toBe(`/projects/${encoded}/tasks/new`);
  }
});

test('6 every kind, node and status renders one deduplicated stage line', () => {
  for (const [kind, stages] of [['feature', FEATURE_STAGES], ['bug', BUG_STAGES]] as const) {
    for (const node of [...stages.flatMap(stage => stage.nodes), 'future', '']) for (const state of Object.keys(STATE_LABELS)) {
      const html = card({ state, node, resume: node, at: node }, kind);
      expect(html.match(/class="task-stage"/g)).toHaveLength(1); expect(html).not.toContain('task-status');
      const parts = html.match(/class="task-stage">([^<]*)/)![1].split(' · ');
      expect(new Set(parts).size).toBe(parts.length);
    }
  }
  expect(card({ state: 'active', node: 'design' }, 'feature')).toContain('class="task-stage">设计</span>');
});
