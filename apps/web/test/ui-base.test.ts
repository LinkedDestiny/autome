import { readFileSync } from 'node:fs';
import { createElement as h, type ComponentType } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createRoutesStub } from 'react-router';
import { expect, test } from 'vitest';
import Loop from '../app/routes/loop.tsx';
import Board from '../app/routes/projects.$id.tsx';
import Task from '../app/routes/tasks.$id.tsx';
import { TaskTimeline } from '../app/components/task-timeline.tsx';
import { SegmentedTabs } from '../app/components/ui/segmented.tsx';
import { Pill } from '../app/components/ui/pill.tsx';
import loopConfig from './fixtures/m07-global.json' with { type: 'json' };
import { taskPageData } from './fixtures/task-page-data.ts';

const css = readFileSync(new URL('../app/styles/app.css', import.meta.url), 'utf8');
const rules = [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map(([, selector, body]) => ({ selectors: selector.trim().split(',').map(s => s.trim()), body }));
function rule(selector: string) {
  const found = rules.filter(r => r.selectors.includes(selector));
  expect(found, selector).not.toHaveLength(0);
  return found.map(r => r.body).join('\n');
}
function render(Component: ComponentType, data: unknown, url: string) {
  const Stub = createRoutesStub([{ id: 'page', path: '*', Component }]);
  return renderToStaticMarkup(h(Stub, { initialEntries: [url], hydrationData: { loaderData: { page: data } } }));
}
function nav(html: string, label: string) {
  const found = [...html.matchAll(/<(nav|div)\b([^>]*aria-label="[^"]+"[^>]*)>([\s\S]*?)<\/\1>/g)].find(m => m[2].includes(`aria-label="${label}"`));
  expect(found, label).toBeDefined();
  expect(found![2]).toMatch(/class="seg(?: |")/);
  expect(found![3]).toMatch(/aria-(?:current="page"|pressed="true")/);
  return found![3];
}

test('分段链接、按钮和单选共享尺寸、白底阴影与黄色焦点，容器可横向滚动', () => {
  for (const selector of ['.seg > a', '.seg > button', '.seg label > span']) {
    const body = rule(selector);
    for (const value of ['min-height: 26px', 'padding: 0 12px', 'font-size: 12px', 'font-weight: 700', 'var(--animal-radius-pill)', 'var(--animal-text-muted)']) expect(body).toContain(value);
  }
  for (const selector of ['.seg > [aria-current="page"]', '.seg > [aria-pressed="true"]', '.seg input:checked + span']) {
    const body = rule(selector);
    for (const value of ['background: var(--ui-surface)', 'color: var(--animal-text)', 'box-shadow: var(--animal-shadow-sm)']) expect(body).toContain(value);
  }
  for (const selector of ['.seg > a:focus-visible', '.seg > button:focus-visible', '.seg input:focus-visible + span']) expect(rule(selector)).toContain('outline: 3px solid var(--animal-focus-yellow)');
  for (const value of ['gap: 4px', 'width: fit-content', 'max-width: 100%', 'overflow-x: auto', 'overflow-y: hidden']) expect(rule('.seg')).toContain(value);
});

test('分段内胶囊保留自身标记，文字样式只作用于 label 的直接 span', () => {
  const html = renderToStaticMarkup(h(SegmentedTabs, { label: '计数', children: h('a', { href: '?tab=decisions', 'aria-current': 'page' }, '待决策', h(Pill, { color: 'yellow' }, 3)) }));
  expect(html).toContain('<span class="pill pill-yellow">3</span>');
  expect(rule('.seg label > span')).toContain('padding: 0 12px');
  expect(rules.flatMap(r => r.selectors)).not.toContain('.seg span');
});

test('Loop 范围和页签、时间线、看板、任务详情 SSR 共用 seg 并保留选中与链接', async () => {
  const project = { id: 'P-1', display_name: '项目', path: '/tmp/project', onboarding: { onboarding: 'skipped' } };
  for (const scope of [undefined, 'P-1']) {
    const html = render(Loop, { config: loopConfig, projects: [project], skills: [], project: scope }, scope ? '/loop?project=P-1' : '/loop');
    nav(html, '配置范围');
    if (scope) nav(html, '项目配置页签');
  }
  const settings = render(Loop, { projects: [project], skills: [], project: 'P-1', settings: { project, files: [], session: { state: 'none' } } }, '/loop?project=P-1&tab=project');
  expect(nav(settings, '项目配置页签')).toMatch(/aria-current="page"[^>]*>项目信息/);
  for (const show of ['', 'gates', 'reopens']) {
    const html = render(() => h(TaskTimeline, { entries: [], window: { from: null, earlier: 0 } }), {}, `/tasks/T-1?tab=timeline&show=${show}`);
    const links = nav(html, '执行过程筛选');
    expect(links.match(/aria-current="page"/g)).toHaveLength(1);
    expect(links).toContain('tab=timeline');
    expect(links).toMatch(new RegExp(`aria-current="page"[^>]*>${show === 'gates' ? '只看门禁' : show === 'reopens' ? '只看退回' : '全部'}`));
  }
  nav(render(Board, { project, tasks: [], now: '2026-11-18T00:00:00Z', showAll: false, alerts: [] }, '/projects/P-1'), '任务类型');
  const task = { id: 'T-1', title: '任务', kind: 'feature', project_id: project.id, state: { state: 'draft' } };
  const data = await taskPageData({ task, project, sessions: [] });
  for (const [tab, label] of [['docs', '文档'], ['decisions', '待你决定'], ['changes', '改动与合并'], ['notes', '备注与依赖']]) {
    const links = nav(render(Task, data, `/tasks/T-1?tab=${tab}`), '任务详情标签');
    expect(links.match(/aria-current="page"/g)).toHaveLength(1);
    expect(links).toMatch(new RegExp(`aria-current="page"[^>]*>${label}`));
  }
});

test('页面不再定义分段选中外观，时间线使用 SegmentedTabs', () => {
  for (const { selectors, body } of rules) {
    for (const selector of selectors.filter(s => /\.(timeline-filters|board-filters|task-detail-tabs)/.test(s))) {
      expect(selector).not.toMatch(/aria-current|aria-pressed/);
      expect(body).not.toMatch(/background:|box-shadow:|text-decoration:|font-weight:|padding:/);
    }
  }
  const source = readFileSync(new URL('../app/components/task-timeline.tsx', import.meta.url), 'utf8');
  expect(source).toContain('<SegmentedTabs className="timeline-filters"');
});
