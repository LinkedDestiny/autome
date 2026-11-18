import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createRoutesStub } from 'react-router';
import { expect, test } from 'vitest';
import { DriverCard } from '../app/components/driver-card.tsx';
import { MarkdownView } from '../app/components/markdown-view.tsx';
import { readFileSync } from 'node:fs';
import { changesView, type TaskChanges } from '../app/components/task-changes-model.ts';
import { TaskChangesPanel } from '../app/components/task-changes.tsx';
import type { TaskPayload } from '../app/components/task-detail-data.ts';
import { parseMarkdown } from '../app/lib/markdown.ts';

const render = (el: React.ReactElement) => renderToStaticMarkup(createElement(createRoutesStub([{ path: '/', Component: () => el }])));

test('A 只读展开入口使用小号文字按钮', () => {
  const changes = JSON.parse(readFileSync(new URL('./fixtures/w7/changes.json', import.meta.url), 'utf8')) as TaskChanges;
  const data = JSON.parse(readFileSync(new URL('./fixtures/w7/feature.json', import.meta.url), 'utf8')) as TaskPayload;
  expect(changesView(changes, data).subjects.length).toBeGreaterThan(0);
  const html = renderToStaticMarkup(createElement(TaskChangesPanel, { data, changes: { payload: changes } }));
  expect(html).toContain('<summary class="btn btn-text btn-sm">提交标题</summary>');
});
test('B 驱动卡去环境页入口使用小号文字按钮', () => {
  const html = render(createElement(DriverCard, { initial: { available: true, list: [] } } as never));
  expect(html).toMatch(/<a class="btn btn-text btn-sm" href="\/env"[^>]*>查看环境/);
});
test('C Markdown 外链使用公共链接类并带下划线类', () => {
  const html = renderToStaticMarkup(createElement(MarkdownView, { nodes: parseMarkdown('[a](https://example.com)') }));
  expect(html).toContain('class="link link-ext"');
});
