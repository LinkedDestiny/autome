import ts from 'typescript';
import { readFileSync, readdirSync } from 'node:fs';
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

const textControls = ':where(input:not([type=checkbox],[type=radio],[type=hidden],[type=file],[type=range]), select, textarea)';
function baseRule(selector: string) {
  const body = [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)].find(([, s]) => s.replace(/\/\*[\s\S]*?\*\//g, '').trim() === selector)?.[2];
  expect(body, selector).toBeDefined();
  return body!;
}

test('七类控件有元素级外观、选中与禁用状态及黄色焦点环', () => {
  const text = baseRule(textControls);
  for (const value of ['appearance: none', 'padding: 0 14px', 'border-radius: var(--animal-radius-sm)', 'background: var(--animal-bg-input)', 'border: 2px solid var(--animal-border-light)', 'box-shadow: var(--ui-shadow-input)', 'font: 600 13px var(--animal-font)', 'color: var(--animal-text)']) expect(text).toContain(value);
  expect(baseRule(textControls.replace(', textarea', ''))).toContain('height: 38px');
  expect(baseRule(':where(textarea)')).not.toMatch(/(?:^|;)\s*height:/);
  for (const value of ['padding-right: 36px', 'linear-gradient(45deg', 'linear-gradient(135deg', 'background-repeat: no-repeat']) expect(baseRule(':where(select)')).toContain(value);
  const check = baseRule(':where(input[type=checkbox], input[type=radio])');
  for (const value of ['appearance: none', 'width: 18px', 'height: 18px', 'border: 2px solid var(--animal-border)', 'background: var(--ui-white)']) expect(check).toContain(value);
  expect(baseRule(':where(input[type=checkbox])')).toContain('border-radius: var(--animal-radius-sm)');
  expect(baseRule(':where(input[type=radio])')).toContain('border-radius: 50%');
  for (const value of ['background: var(--animal-primary)', 'border-color: var(--animal-primary)']) expect(baseRule(':where(input[type=checkbox]:checked, input[type=radio]:checked)')).toContain(value);
  for (const value of ['border: solid var(--ui-white)', 'transform: rotate(45deg)']) expect(baseRule(':where(input[type=checkbox]:checked)::after')).toContain(value);
  expect(baseRule(':where(input[type=radio]:checked)')).toContain('box-shadow: inset 0 0 0 4px var(--ui-white)');
  for (const value of ['background-color: var(--animal-bg-disabled)', 'color: var(--animal-text-muted)']) expect(baseRule(':where(input:disabled, select:disabled, textarea:disabled)')).toContain(value);
  expect(baseRule(':where(input[type=checkbox]:disabled, input[type=radio]:disabled)')).toContain('opacity: .5');
  for (const value of ['border: 2px solid var(--animal-border-light)', 'border-radius: var(--animal-radius)', 'padding: 12px 14px', 'margin: 0', 'min-width: 0']) expect(baseRule(':where(fieldset)')).toContain(value);
  for (const value of ['padding: 0 6px', 'font-size: 12px', 'font-weight: 800', 'color: var(--animal-text-muted)']) expect(baseRule(':where(legend)')).toContain(value);
  expect(baseRule(':where(input, select, textarea):focus-visible')).toContain('outline: 3px solid var(--animal-focus-yellow)');
  for (const selector of ['.palette input', '.task-new textarea', '.onboarding-editor textarea', '.add-project input:not([type="checkbox"]):not([type="hidden"])', '.add-project select']) expect(rule(selector)).not.toMatch(/(?:^|;)\s*(?:border(?:-radius)?|background|color|font|padding):/);
  expect(rule('.seg input')).toContain('opacity: 0');
  expect(rule('.switch input')).toContain('opacity: 0');
  expect(rule('.stepper input')).toContain('background: transparent');
});

test('全站 TSX 控件类型落在基础覆盖集合，文件输入保持隐藏', () => {
  const app = new URL('../app/', import.meta.url);
  const allowed = new Set(['text', 'search', 'number', 'checkbox', 'radio', 'hidden', 'file']);
  let count = 0;
  for (const file of readdirSync(app, { recursive: true }).map(String).filter(f => f.endsWith('.tsx'))) {
    const source = ts.createSourceFile(file, readFileSync(new URL(file, app), 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
    function visit(node: ts.Node) {
      if ((ts.isJsxSelfClosingElement(node) || ts.isJsxOpeningElement(node)) && /^(input|Input|select|textarea|fieldset)$/.test(node.tagName.getText(source))) {
        count++;
        if (/^(input|Input)$/.test(node.tagName.getText(source))) {
          const attrs = node.attributes.properties.filter(ts.isJsxAttribute);
          const type = attrs.find(a => a.name.getText(source) === 'type')?.initializer;
          const value = !type ? 'text' : ts.isStringLiteral(type) ? type.text : ts.isJsxExpression(type) && type.expression && ts.isStringLiteral(type.expression) ? type.expression.text : '<dynamic>';
          expect(allowed.has(value), `${file}: ${node.getText(source)}`).toBe(true);
          if (value === 'file') expect(attrs.find(a => a.name.getText(source) === 'className')?.initializer?.getText(source)).toMatch(/\bvisually-hidden\b/);
        }
      }
      ts.forEachChild(node, visit);
    }
    visit(source);
  }
  expect(count).toBeGreaterThan(0);
});

test('全站样式不恢复浏览器原生外观', () => {
  const source = css.replace(/\/\*[\s\S]*?\*\//g, '');
  for (const [, value] of source.matchAll(/(?:[\w-]*appearance)\s*:\s*([^;}]+)/g)) expect(value.trim()).toBe('none');
  expect(source).not.toMatch(/\ball\s*:\s*(?:revert(?:-layer)?|unset|initial)\b/i);
});

import { Drawer } from '../app/components/ui/drawer.tsx';
test('丝带标题继承字号，两档尺寸且容器不拉伸', () => {
  for (const declaration of ['font-size: inherit', 'line-height: inherit', 'margin: 0', 'color: inherit']) expect(baseRule('.ribbon :is(h1, h2, h3)')).toContain(declaration);
  for (const declaration of ['width: fit-content', 'max-width: 100%', 'font-size: 17px']) expect(rule('.ribbon')).toContain(declaration);
  expect(rule('.ribbon-sm')).toContain('font-size: 13px');
});
test('没有其它针对丝带标题的字号覆盖', () => {
  for (const [, selector, body] of css.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    if (/font-size\s*:/.test(body) && /h[123]/.test(selector) && /ribbon|overview-heading|add-project|task-new|env-page/.test(selector)) expect(selector.trim()).toBe('.ribbon :is(h1, h2, h3)');
  }
});
test('Drawer 标题行关闭、正文独立滚动、固定 footer', () => {
  const html = renderToStaticMarkup(h(Drawer, { label: '编辑', title: '标题', onClose() {}, footer: h('button', {}, '保存'), children: '正文' }));
  expect(html).toMatch(/<aside[^>]*role="dialog"[^>]*aria-modal="false"/);
  expect(html).toMatch(/<header class="drawer-head"><h2>标题<\/h2><button[^>]*drawer-close[^>]*>关闭<\/button><\/header>/);
  expect(html).toContain('<div class="drawer-body" data-scroll-region="true">正文</div>');
  expect(html).toContain('<footer class="drawer-foot"><button>保存</button></footer>');
  for (const value of ['position: fixed', 'inset: 64px 16px 16px auto', 'overflow: hidden', 'flex-direction: column']) expect(rule('.drawer')).toContain(value);
  for (const value of ['flex: 1', 'min-height: 0', 'overflow-y: auto']) expect(rule('.drawer-body')).toContain(value);
  expect(rule('.drawer-foot')).toContain('flex: none');
});
