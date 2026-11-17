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
  expect(html).toContain('href="/projects/a%2Fb%20%3F%E4%B8%AD"'); expect(html).toContain('title="读盘失败"'); expect(html).toContain('等你处理 —'); expect(html).toContain('<small>没有任务在跑</small>');
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
  const empty = overview({ ...shell, tasks: [] }); expect(empty).toContain('都处理完了'); expect(empty).toContain('没有任务在跑');
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

test('7 总览与侧栏标记配置错误且不混入报警', () => {
  const projects = shell.projects.map(p => ({ ...p, configError: 'TOML 原文' }));
  expect(overview({ ...shell, projects })).toContain('配置有误');
  const text = renderToStaticMarkup(createElement(MemoryRouter, null, createElement(ProjectList, { projects }))); expect(text).toContain('配置有误'); expect(text).not.toContain('报警 1');
});

import { mergeResult, needsNewDay, overviewConclusion, waitingMilliseconds } from '../app/lib/overview-model.ts';
import { overviewCardView } from '../app/lib/overview-labels.ts';
import { ClockProvider } from '../app/lib/clock.tsx';
import type { OverviewDetail } from '../server/overview.ts';
const now = '2026-11-17T02:00:00Z';
const detail: OverviewDetail = { reviews: 2, adjudications: 1, pending: 0, designName: 'example.md', designBytes: 38000, status_block: { milestones_done: 1, milestones_total: 3, current_milestone: 'M-02', design_round: 2, design_round_limit: 15, impl_round: 3, impl_round_limit: 65 } };
function richCard(t: ShellTask, d = detail, waitingSince?: number) {
  return renderToStaticMarkup(createElement(MemoryRouter, null, createElement(ClockProvider, { initial: { now, timeZone: 'Asia/Shanghai' } }, createElement(OverviewCard, { task: t, detail: d, waitingSince, projectName: '项目甲' }))));
}
test('8 结论句四种计数组合，含在跑与排队且省略零项', () => {
  expect(overviewConclusion(0, 0, 0)).toBe('现在没有事情等你 · 没有任务在跑，驱动空闲');
  expect(overviewConclusion(0, 0, 0, true)).toContain('驱动在推进');
  expect(overviewConclusion(1, 0, 0)).toBe('有 1 件事等你决定');
  expect(overviewConclusion(0, 2, 3)).toBe('没有事情等你决定 · 另有 2 个任务在跑，3 个在排队');
  expect(overviewConclusion(1, 2, 3)).toBe('有 1 件事等你决定 · 另有 2 个任务在跑，3 个在排队');
});
test('9 开放区间与今天取交集、随 now 增长、跨日要求重新验证', () => {
  const dayStart = Date.parse('2026-11-16T16:00:00Z'), dayEnd = dayStart + 86400_000;
  const slice = { dayStart, dayEnd, closedMs: 3000, openStarts: [dayStart - 1000, dayStart + 1000] };
  expect(waitingMilliseconds(slice, new Date(dayStart + 2000).toISOString())).toBe(6000);
  expect(waitingMilliseconds(slice, new Date(dayEnd + 1000).toISOString())).toBe(3000 + 2 * 86400_000 - 1000);
  expect(needsNewDay(slice, new Date(dayEnd - 1).toISOString())).toBe(false); expect(needsNewDay(slice, new Date(dayEnd).toISOString())).toBe(true);
});
test('10 等待种类胶囊、摘要、真实失败原因与主按钮目的地', () => {
  for (const [node, badge, summary] of [['await_design_approval', '批准设计', '设计已定稿：3 个里程碑。评审 2 轮、裁决 1 轮。'], ['await_merge', '批准合并', AWAIT_PHRASES.await_merge], ['await_info', '补充信息', AWAIT_PHRASES.await_info], ['not_a_bug', '确认不是 Bug', AWAIT_PHRASES.not_a_bug]]) {
    const html = richCard({ ...task, state: { state: 'active', node } }, detail, Date.parse(now) - 60_000);
    expect(html).toContain(badge); expect(html).toContain(summary); expect(html).toContain('已等 1 分 00 秒');
    if (node === 'await_design_approval') expect(html).toContain('/tasks/T-2?doc=example.md');
    if (node === 'await_merge') expect(html).toContain('/tasks/T-2?tab=changes');
  }
  for (const status of ['paused', 'stopped', 'failed']) expect(richCard({ ...task, state: { state: status, at: 'fix' } })).not.toContain('已等');
  const failed = { ...task, state: { state: 'failed', at: 'fix', reason: { kind: 'session_crashed', detail: '实际原因' } } };
  expect(richCard(failed)).toContain('会话崩溃：实际原因');
});
test('11 没有 Backlog 时不写待表态，有条目才显示数目', () => {
  const waiting = { ...task, state: { state: 'active', node: 'await_design_approval' } };
  expect(richCard(waiting)).not.toContain('等你表态'); expect(richCard(waiting, { ...detail, pending: 2 })).toContain('另有 2 条 Backlog 等你表态');
});
test('12 进行中呈现真实轮次、里程碑进度、本轮与任务时长', () => {
  const html = richCard({ ...task, created_at: '2026-11-17T00:00:00Z', cost_usd: 3.2 }, { ...detail, session: { kind: { kind: 'role', role: 'impl' }, label: '实现', round: 3, started_at: '2026-11-17T01:53:57Z' } });
  for (const text of ['实现 3 / 65', 'aria-valuenow="33.333', '里程碑 1 / 3', '正在做 M-02', '本轮已跑 6 分 03 秒', '2 时 00 分', '$3.20']) expect(html).toContain(text);
  expect(html).not.toContain('门禁未跑过');
});
test('13 排队卡片区分依赖与空闲槽位，包含失败和取消的依赖', () => {
  const queued = { ...task, state: { state: 'queued' }, queue_position: 2 };
  expect(richCard(queued)).toContain('排第 2 位 · 等空闲槽位');
  expect(richCard({ ...queued, blocked_by: [{ id: 'T-1' }] })).toContain('等 T-1 合并后开始');
  expect(richCard({ ...queued, blocked_by: [{ id: 'T-1', reason: 'failed' }, { id: 'T-0', reason: 'cancelled' }] })).toContain('已取消，请修改依赖');
});
test('14 空闲保持两张虚线卡、四个数字块和最近合并区', () => {
  const html = overview({ ...shell, tasks: [] });
  expect(html.match(/card-dash/g)).toHaveLength(2); expect(html.match(/class="tile tile-/g)).toHaveLength(4); expect(html).toContain('最近合并'); expect(html).toContain('新建任务'); expect(html).toContain('现在没有事情等你');
});
test('15 结果胶囊六条规则与优先级', () => {
  const rows = [[{ merge_gate_runs: 2, reopen_total: 6 }, '门禁 2 次', 'orange'], [{ merge_gate_runs: 1, reopen_total: 5 }, '退回 5 次', 'orange'], [{ merge_gate_runs: 1, reopen_total: 4 }, '退回 4 次', 'brown'], [{ merge_gate_runs: 1, reopen_total: 0 }, '门禁一次过', 'green'], [{ reopen_total: 0 }, '无退回', 'green'], [null, '—', 'brown']] as const;
  for (const [metrics, text, color] of rows) expect(mergeResult(metrics)).toMatchObject({ text, color });
});
test('16 计数和进度输入边界：零里程碑、单项、缺失成本与已计费零', () => {
  expect(overviewCardView(task, undefined, now).cost).toBe('无计费'); expect(overviewCardView({ ...task, cost_usd: 0 }, undefined, now).cost).toBe('$0.00');
  const html = richCard(task, { ...detail, status_block: { ...detail.status_block!, milestones_total: 0, milestones_done: 0 } }); expect(html).toContain('aria-valuenow="0"'); expect(html).not.toContain('NaN');
  expect(richCard(task, { ...detail, status_block: { ...detail.status_block!, milestones_total: 1, milestones_done: 1 } })).toContain('aria-valuenow="100"');
});
test('17 卡片详情读取失败不编造设计轮数与摘要，纯文本转义', () => {
  const html = richCard({ ...task, title: '<script>x</script>', state: { state: 'active', node: 'await_design_approval' } }, { reviews: 0, adjudications: 0, pending: 0, error: '详情暂不可用：读盘失败' });
  expect(html).toContain('详情暂不可用：读盘失败'); expect(html).not.toContain('评审 0 轮'); expect(html).toContain('&lt;script&gt;');
});

/* eslint-disable @typescript-eslint/no-explicit-any -- 审计判别检查原文保留 */
test('等合并的 Bug 卡片状态块缺设计轮字段时不得出现 undefined', () => {
  const task: any = { id: 'T-audit', state: { state: 'await_merge' }, kind: 'bug', created_at: '2026-11-18T00:00:00Z', cost_usd: null };
  const v = overviewCardView(task, { status_block: { milestones_done: 1, milestones_total: 1 } } as any, '2026-11-18T00:01:00Z');
  expect(JSON.stringify(v)).not.toMatch(/undefined/);
});
/* eslint-enable @typescript-eslint/no-explicit-any */

test('轮次缺值逐字段回退，零值保留，设计与实现会话使用对应预算', () => {
  for (const role of ['plan', 'impl', 'audit', 'retro']) {
    for (const value of [undefined, null, 0, 1]) {
      const block = { ...detail.status_block!, design_round: value, design_round_limit: value, impl_round: value, impl_round_limit: value } as unknown as NonNullable<OverviewDetail['status_block']>;
      const view = overviewCardView(task, { ...detail, status_block: block, session: { kind: { kind: 'role', role }, label: '会话', round: 1, started_at: now } }, now);
      const expected = `${value ?? '—'} / ${value ?? '—'}`;
      expect(view.round).toBe(expected);
      expect(view.badge).toBe(`会话 ${expected}`);
      expect(JSON.stringify(view)).not.toMatch(/undefined|null|NaN/);
    }
  }
  expect(overviewCardView(task, { ...detail, status_block: { ...detail.status_block!, design_round: 0 } }, now).round).toBe('0 / 15');
});

import { toOverviewDetail } from '../app/lib/overview-model.ts';
import { loadTaskDetail, type TaskPayload } from '../app/components/task-detail-data.ts';
import { vi } from 'vitest';
const approvalTask = { ...task, kind: 'feature', state: { state: 'active', node: 'await_design_approval' } };
function approvalPayload(review: boolean, adjudication: boolean): TaskPayload {
  return { task: { id: task.id, title: task.title, slug: '中文 &' }, sessions: [], documents: [
    ...(review ? [{ name: '中文 &-review.md', absolute: '/private/review', label: '评审' }] : []),
    ...(adjudication ? [{ name: '中文 &-adjudication.md', absolute: '/private/adjudication', label: '裁决' }] : []),
  ] };
}
test.each([[true, true], [true, false], [false, true], [false, false]])('批准设计文档存在组合 review=%s adjudication=%s', (review, adjudication) => {
  const html = richCard(approvalTask, toOverviewDetail(approvalPayload(review, adjudication)));
  for (const [exists, label, suffix] of [[review, '评审', 'review'], [adjudication, '裁决', 'adjudication']] as const) {
    expect(html.includes(`查看${label}`)).toBe(exists);
    expect(html.includes(`${label}结果摘要不可得`)).toBe(!exists);
    if (exists) expect(html).toContain(`href="/tasks/T-2?tab=docs&amp;doc=${encodeURIComponent(`中文 &-${suffix}.md`)}"`);
  }
  expect(html).not.toContain('/private/'); expect(html).not.toMatch(/提了|采纳/);
});
test('批准入口随清单增删与等待状态切换，无详情或失败明确回退', () => {
  for (const exists of [false, true, false]) {
    const d = toOverviewDetail(approvalPayload(exists, exists));
    expect(richCard(approvalTask, d).includes('查看评审')).toBe(exists);
    for (const state of [{ state: 'active', node: 'await_merge' }, { state: 'active', node: 'implement' }, { state: 'failed' }, { state: 'queued' }, { state: 'paused' }, { state: 'stopped' }]) {
      expect(richCard({ ...task, state }, d)).not.toMatch(/查看评审|查看裁决|结果摘要不可得/);
    }
  }
  for (const error of ['读盘失败', '超时', '外部服务不可达']) {
    const html = richCard(approvalTask, { reviews: 0, adjudications: 0, pending: 0, error });
    expect(html).toContain(error); expect(html).toContain('评审结果摘要不可得'); expect(html).toContain('裁决结果摘要不可得');
  }
  expect(card(approvalTask.state, 'feature')).toContain('评审结果摘要不可得');
  expect(toOverviewDetail({ ...approvalPayload(false, false), documents: undefined }).reviewName).toBeUndefined();
});
test('卡片入口经受控详情加载器读取，撤去清单或读取失败不能绕过', async () => {
  const payload = approvalPayload(true, true), read = vi.fn().mockResolvedValue({ body: { ok: true, payload } });
  const docRead = vi.fn().mockResolvedValue({ text: '# 结果', truncated: false });
  const services = { gateway: { read, write: vi.fn() }, taskDocs: { read: docRead } };
  const links = overviewCardView(approvalTask, toOverviewDetail(payload), now).reviewDocuments;
  for (const [index, link] of links.entries()) {
    const loaded = await loadTaskDetail(services, task.id, new URL(link.href!, 'http://localhost'));
    expect(loaded.document?.text).toBe('# 结果');
    expect(docRead).toHaveBeenLastCalledWith(payload.documents![index].absolute, 204800);
  }
  docRead.mockResolvedValueOnce({ error: '读盘失败' });
  expect((await loadTaskDetail(services, task.id, new URL(links[0].href!, 'http://localhost'))).document?.error).toBe('读盘失败');
  payload.documents = []; docRead.mockClear();
  expect((await loadTaskDetail(services, task.id, new URL(links[0].href!, 'http://localhost'))).document?.error).toContain('没有名为');
  for (const name of ['/private/review', '../review.md', '']) {
    expect((await loadTaskDetail(services, task.id, new URL(`http://localhost/tasks/T-2?doc=${encodeURIComponent(name)}`))).document?.error).toBeTruthy();
  }
  expect(docRead).not.toHaveBeenCalled();
});
