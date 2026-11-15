import { taskPageData } from './fixtures/task-page-data.ts';
import { timelineEntries } from '../app/components/task-timeline-model.ts';
import { readdirSync, readFileSync } from 'node:fs';
import { createElement as h, type ComponentType, type ReactNode } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createRoutesStub } from 'react-router';
import ts from 'typescript';
import { expect, test } from 'vitest';
import { TEST_IDS as ids } from '../app/lib/testids.ts';
import { Shell } from '../app/components/shell.tsx';
import Overview from '../app/routes/_index.tsx';
import Board from '../app/routes/projects.$id.tsx';
import Task from '../app/routes/tasks.$id.tsx';
import NewTask from '../app/routes/projects.$id.tasks.new.tsx';
import NewProject, { PathSuggestions, DirectoryPicker } from '../app/routes/projects.new.tsx';
import Settings from '../app/routes/projects.$id.settings.tsx';
import Env, { EnvView } from '../app/routes/env.tsx';
import { BellView } from '../app/components/bell.tsx';
import { LiveBanner } from '../app/lib/live.tsx';
import { TaskOps } from '../app/components/task-ops-view.tsx';
import { OP_LABELS } from '../app/components/task-ops.ts';
import { TaskTimeline } from '../app/components/task-timeline.tsx';
import { SessionLogView } from '../app/components/session-log.tsx';
import { initialLogState } from '../app/components/session-log-model.ts';
import { CommandPalette } from '../app/components/command-palette.tsx';
const now = '2026-11-14T00:00:00Z';
const project = { id: 'P-1', name: '项目', display_name: '项目', path: '/tmp/project', onboarding: { onboarding: 'skipped' } };
const task = { id: 'T-01', title: '任务', kind: 'bug', project_id: project.id, state: { state: 'draft' } };
const shell = { kernel: { ok: true }, projects: [project], tasks: [task] };
const board = { project, tasks: [task], now, showAll: false, alerts: [] };
function render(Component: ComponentType, data: unknown, url = '/') {
  const Stub = createRoutesStub([{ id: 'root', path: '*', Component: () => h(Shell, { projects: [], banner: null, children: h(Component) }) }]);
  return renderToStaticMarkup(h(Stub, { initialEntries: [url], hydrationData: { loaderData: { root: data } } }));
}
function view(child: ReactNode, url = '/') { return render(() => child, shell, url); }
function has(html: string, ...selectors: string[]) { for (const id of selectors) expect(html).toContain(`data-testid="${id}"`); }
const pages: [string, ComponentType, unknown, string][] = [
  [ids.pageOverview, Overview, shell, '/'],
  [ids.pageBoard, Board, board, '/projects/P-1'],
  [ids.pageTask, Task, await taskPageData({ task, project, sessions: [] }), '/tasks/T-01'],
  [ids.pageTaskNew, NewTask, shell, '/projects/P-1/tasks/new'],
  [ids.pageProjectNew, NewProject, { display: '' }, '/projects/new'],
  [ids.pageSettings, Settings, { project, files: [], session: { state: 'none' } }, '/projects/P-1/settings'],
  [ids.pageEnv, Env, { recipes: {}, errors: {} }, '/env'],
];
// Explicit route parameters are required by the new-task page.
function pageRender(Component: ComponentType, data: unknown, url: string) {
  if (Component !== NewTask) return render(Component, data, url);
  const Stub = createRoutesStub([{ id: 'root', path: '/projects/:id/tasks/new', Component: () => h(Shell, { projects: [], banner: null, children: h(NewTask) }) }]);
  return renderToStaticMarkup(h(Stub, { initialEntries: [url], hydrationData: { loaderData: { root: data } } }));
}
for (const [id, Component, data, url] of pages) test(`${id} and shell root`, () => {
  const html = pageRender(Component, data, url); has(html, ids.shellMain, id);
  if (id === ids.pageTaskNew) { has(html, ids.taskNewForm, ids.taskNewDraft); has(pageRender(NewTask, { ...shell, projects: [] }, url), id); }
  if (id === ids.pageBoard) has(render(Board, { error: '读盘失败' }), id);
  if (id === ids.pageSettings) has(render(Settings, { error: '项目不存在' }), id);
  if (id === ids.pageProjectNew) has(html, ids.pathInput, ids.pathSuggestions);
});
test('component selectors retain object identity and conditional states', () => {
  const html = render(Board, board); has(html, ids.boardColumn, ids.taskCard, ids.taskOp);
  for (const column of ['todo', 'running', 'needs_you', 'done']) expect(html).toContain(`data-column="${column}"`);
  expect(html).toContain('data-task-id="T-01"'); expect(html).toContain('data-op="start"');
  const ops = view(h(TaskOps, { ops: Object.keys(OP_LABELS).filter(op => op !== 'decide').map(op => ({ op: op as keyof typeof OP_LABELS, enabled: true })) }));
  for (const op of Object.keys(OP_LABELS).filter(op => op !== 'decide')) expect(ops).toContain(`data-op="${op}"`);
  const bellProps = { snapshot: { unread: 1, items: [{ id: 'N-1', task_id: task.id, project_id: project.id, kind: 'task', level: 'info', title: '批准', body: null, created_at: now, read_at: null }] }, onToggle() {}, async onRead() {}, async onAll() {} };
  has(view(h(BellView, { ...bellProps, open: true })), ids.bellButton, ids.bellPanel, ids.bellItem);
  expect(view(h(BellView, { ...bellProps, open: true }))).toContain('data-task-id="T-01"');
  expect(view(h(BellView, { ...bellProps, open: false }))).not.toContain(`data-testid="${ids.bellPanel}"`);
  for (const state of ['offline', 'outdated', 'reconnecting'] as const) {
    const banner = view(h(LiveBanner, state === 'reconnecting' ? { connection: state } : { connection: 'open', core: { type: 'core', status: state, reason: '错误' } }));
    has(banner, ids.liveBanner); expect(banner).toContain(`data-state="${state}"`);
  }
  expect(view(h(LiveBanner, { connection: 'open', core: { type: 'core', status: 'connected' } }))).not.toContain(`data-testid="${ids.liveBanner}"`);
  const session = { id: 'S-01', label: '定位', round: 1, running: true, started_at: now };
  const timeline = view(h(TaskTimeline, { entries: timelineEntries([session], [{ seq: 1, subject: task.id, at: now, kind: 'session.started', payload: { session_id: session.id, node: 'locate' } }], now), window: { from: null, earlier: 0 } }));
  has(timeline, ids.timeline, ids.timelineEntry, ids.sessionLog, ids.sessionLogBody);
  expect(timeline).toContain('data-node="locate"'); expect(timeline).toContain('data-entry-key="S-01"');
  for (const open of [false, true]) {
    const log = view(h(SessionLogView, { session, state: initialLogState(session.id), open, onToggle() {}, onAction() {} }));
    has(log, ids.sessionLog); expect(log.includes(`data-testid="${ids.sessionLogBody}"`)).toBe(open); expect(log).toContain('data-session-id="S-01"');
  }
  for (const entries of [[], [{ path: '/tmp/中文 空格', name: '中文 空格' }]]) {
    const suggestions = view(h(PathSuggestions, { entries, disabled: false, onSelect() {} })); has(suggestions, ids.pathSuggestions);
    expect(suggestions.includes(`data-testid="${ids.pathSuggestion}"`)).toBe(entries.length > 0);
  }
  for (const picker of [undefined, async () => '/tmp/project']) expect(view(h(DirectoryPicker, { picker, disabled: false, onPick() {}, onError() {} })).includes(`data-testid="${ids.pickDirectory}"`)).toBe(Boolean(picker));
  const command = 'npm install -g example';
  const env = view(h(EnvView, { data: { errors: {}, recipes: { node: { recipe: { component: 'node', command, prerequisite: null, note: null }, prerequisite_ok: true, login_command: null } }, snapshot: { probed: true, probing: false, generation: 1, environment: { components: [{ component: 'node', present: false, version: null, path: null, login: { login: 'not_applicable' } }] }, problems: [] } } }));
  has(env, ids.envCopy); expect(env).toContain(`data-command="${command}"`);
  for (const open of [false, true]) expect(view(h(CommandPalette, { candidates: [], state: { open, query: '', active: -1 }, onChange() {} })).includes(`data-testid="${ids.palette}"`)).toBe(open);
});
function sources(dir = new URL('../app/', import.meta.url)): [string, string][] {
  return readdirSync(dir, { withFileTypes: true }).flatMap(entry => {
    const url = new URL(entry.name + (entry.isDirectory() ? '/' : ''), dir);
    return entry.isDirectory() ? sources(url) : /\.tsx?$/.test(entry.name) ? [[url.pathname, readFileSync(url, 'utf8')]] : [];
  });
}
function attributes(source: string, visit: (attrs: ts.JsxAttributes) => void) {
  const ast = ts.createSourceFile('source.tsx', source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  function walk(node: ts.Node) { if (ts.isJsxAttributes(node)) visit(node); ts.forEachChild(node, walk); } walk(ast);
}
test('selector registry is unique, used, and the only source for JSX testids', () => {
  expect(new Set(Object.values(ids)).size).toBe(Object.keys(ids).length);
  const used = new Set<string>();
  for (const [, source] of sources()) attributes(source, attrs => {
    for (const attr of attrs.properties) if (ts.isJsxAttribute(attr) && attr.name.getText() === 'data-testid') {
      const value = attr.initializer;
      expect(value && ts.isJsxExpression(value) && value.expression && ts.isPropertyAccessExpression(value.expression)).toBeTruthy();
      if (!value || !ts.isJsxExpression(value) || !value.expression || !ts.isPropertyAccessExpression(value.expression)) return;
      expect(value.expression.expression.getText()).toBe('TEST_IDS');
      const key = value.expression.name.text; expect(ids).toHaveProperty(key); used.add(key);
    }
  });
  expect([...used].sort()).toEqual(Object.keys(ids).sort());
});
test('scroll exemptions are restricted to designated internal regions', () => {
  const allowed = ['task-notes-panel', 'dependency-choice-list', 'board-config-error', 'board-column-body', 'task-timeline', 'session-log-text', 'task-docs-body', 'settings-card-body', 'palette-list', 'bell-list'];
  const found = new Set<string>();
  for (const [, source] of sources()) attributes(source, attrs => {
    if (!attrs.properties.some(attr => ts.isJsxAttribute(attr) && attr.name.getText() === 'data-scroll-region')) return;
    const cls = attrs.properties.find(attr => ts.isJsxAttribute(attr) && attr.name.getText() === 'className');
    const value = cls && ts.isJsxAttribute(cls) && cls.initializer && ts.isStringLiteral(cls.initializer) ? cls.initializer.text : '';
    expect(allowed).toContain(value); found.add(value);
  });
  expect([...found].sort()).toEqual(allowed.sort());
});

test('expanded cancelled section exposes stable selectors', () => {
  has(render(Board, { ...board, showCancelled: true, cancelledCounts: { all: 1, feature: 0, bug: 1 }, cancelled: [{ ...task, state: { state: 'cancelled' } }] }, '/projects/P-1?cancelled=all'), ids.boardCancelled, ids.boardCancelledToggle);
});
