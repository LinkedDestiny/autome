import { expect, test } from 'vitest';
import { matchEvent, NOTIFICATION_RULES, levelOf, renderReason, renderTitle } from '../server/notifier-rules.ts';
import { createNotifier } from '../server/notifier.ts';
import { openWebStore } from '../server/store.ts';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
// 原文来自设计文档的真实账本，不改写 payload。
const originals = [
'{"kind":"task.updated","payload":{"state":{"node":"await_design_approval","state":"active"},"trigger":"session_ended"},"seq":21,"subject":"T-1"}',
'{"kind":"task.updated","payload":{"state":{"node":"await_merge","state":"active"},"trigger":"gate_done"},"seq":45,"subject":"T-1"}',
'{"kind":"task.updated","payload":{"state":{"node":"await_info","state":"active"},"trigger":"bug_session_ended"},"seq":124,"subject":"T-5"}',
'{"kind":"task.updated","payload":{"state":{"at":"intake","reason":{"detail":"会话退出码 3","kind":"session_crashed"},"state":"failed"},"trigger":"session_ended"},"seq":116,"subject":"T-4"}',
'{"kind":"project.alert","payload":{"alert_id":"alr-e3bc2b65c341451ea98f1fa54c6b612b","kind":"main_gate_failed","task_id":"T-2"},"seq":102,"subject":"prj-f5a8e540132845849a830aa90fa0667a"}',
'{"kind":"task.updated","payload":{"state":{"state":"done"},"trigger":"core_step_done"},"seq":53,"subject":"T-1"}',
'{"kind":"task.updated","payload":{"state":{"node":"not_a_bug","state":"active"},"trigger":"bug_session_ended"},"seq":9,"subject":"T-1"}',
];
const expected = [
['await_design_approval', 'need_you', 'T-1 设计已定稿，等你批准'], ['await_merge', 'need_you', 'T-1 可以合并了'],
['await_info', 'need_you', 'T-5 无法复现，需要更多信息'], ['failed', 'need_you', 'T-4 停在失败上：会话异常退出（会话退出码 3）'],
['project_alert', 'need_you', 'main 门禁失败'], ['done', 'fyi', 'T-1 已合并'], ['not_a_bug', 'need_you', 'T-1 被判定不是 Bug，等你决定'],
];
test.each(originals.map((raw, i) => [raw, expected[i]] as const))('原始事件 %s', (raw, [kind, level, title]) => {
  const event = JSON.parse(raw), rule = matchEvent(event)!;
  expect(rule.kind).toBe(kind); expect(rule.level).toBe(level); expect(renderTitle(rule, event)).toBe(title);
});
test('表外状态和事件均无通知', () => {
  for (const state of ['draft', 'queued', 'paused', 'stopped', 'cancelled']) expect(matchEvent({ seq: 1, kind: 'task.updated', subject: 'T-1', payload: { state: { state } } })).toBeUndefined();
  for (const node of ['intake', 'design', 'design_review', 'implement', 'audit', 'retro', 'locate', 'fix', 'verify']) expect(matchEvent({ seq: 1, kind: 'task.updated', subject: 'T-1', payload: { state: { state: 'active', node } } })).toBeUndefined();
  for (const kind of ['task.created', 'task.protocol_failure', 'project.alert_dismissed']) expect(matchEvent({ seq: 1, kind, subject: 'T-1', payload: {} })).toBeUndefined();
  expect(matchEvent(JSON.parse('{"kind":"task.updated","payload":{"state":{"node":"locate","state":"active"},"trigger":"slot_available"},"seq":5,"subject":"T-1"}'))).toBeUndefined();
});
test('九种失败原因、未知类型、空白与 60 字边界', () => {
  const cases = { design_budget: '设计轮次用完（上限 0）', impl_budget: '实现轮次用完（上限 0）', fix_budget: '修复轮次用完（上限 0）', infeasible: '设计判定不可实现', protocol: '协议失败（a b）', session_crashed: '会话异常退出（a b）', rebase_conflict: 'rebase 冲突（a b）', config: '配置问题（a b）', core_step: '内核步骤 intake 失败（a b）', future: '未知原因（future）' };
  for (const [kind, title] of Object.entries(cases)) expect(renderReason({ kind, limit: 0, node: 'intake', detail: ' a\n\t b ' })).toBe(title);
  for (const size of [0, 1, 60, 61]) expect(renderReason({ kind: 'protocol', detail: '字'.repeat(size) })).toBe(`协议失败（${'字'.repeat(Math.min(size, 60))}${size > 60 ? '…' : ''}）`);
});
test('告警 kind 与分支', () => {
  const event = JSON.parse(originals[4]); const rule = matchEvent(event)!;
  expect(renderTitle(rule, event, 'develop')).toBe('develop 门禁失败');
  event.payload.kind = 'future'; expect(renderTitle(rule, event)).toBe('项目告警：future');
});
test('恰七行且 kind 唯一，levelOf 源于规则表', () => {
  expect(NOTIFICATION_RULES).toHaveLength(7); expect(new Set(NOTIFICATION_RULES.map(r => r.kind)).size).toBe(7);
  for (const [kind, level] of expected) expect(levelOf(kind)).toBe(level);
  expect(levelOf('future')).toBeUndefined();
});
test('进入：124/145/150 两次进入，深相等的连续状态只算一次', async () => {
  const home = mkdtempSync(join(tmpdir(), 'notifier-rule-')); const store = openWebStore(home);
  const first = JSON.parse(originals[2]);
  const events = [first, { ...first, seq: 130, payload: { state: { state: 'active', node: 'await_info' } } },
    JSON.parse('{"kind":"task.updated","payload":{"state":{"node":"locate","state":"active"},"trigger":"reply"},"seq":145,"subject":"T-5"}'),
    JSON.parse('{"kind":"task.updated","payload":{"state":{"node":"await_info","state":"active"},"trigger":"bug_session_ended"},"seq":150,"subject":"T-5"}')];
  store.commit({ cursor: { name: 'notifier', seq: 0 } });
  const notifier = createNotifier({ store, call: async method => method === 'events.since' ? { events, latest_seq: 150 } : {}, publish() {}, log() {} });
  try { await notifier.pull(); expect(store.list().map(n => n.event_seq)).toEqual([150, 124]); }
  finally { notifier.close(); store.close(); rmSync(home, { recursive: true, force: true }); }
});

test('预算告警标题', () => {
  const event = { seq: 1, kind: 'project.alert', subject: 'p', payload: { kind: 'budget_exceeded', task_id: 'T-9' } };
  expect(renderTitle(matchEvent(event)!, event)).toBe('T-9 费用超出预算');
  expect(matchEvent({ ...event, kind: 'task.budget_exceeded' })).toBeUndefined();
});
