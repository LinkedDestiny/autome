import { readFileSync } from 'node:fs';
import { test, expect } from 'vitest';
import { columnOf, countByColumn, AWAITING_NODES } from '../app/components/task-columns.ts';
import { stageOf, FEATURE_STAGES, BUG_STAGES, NODE_LABELS, type TaskState } from '../app/lib/labels.ts';
const cases: [TaskState, string | null][] = [
  [{ state: 'draft' }, 'todo'], [{ state: 'queued' }, 'todo'], [{ state: 'queued', resume: 'gate' }, 'todo'],
  ...['implement', 'gate'].map(node => [{ state: 'active', node }, 'running'] as [TaskState, string]),
  ...['await_design_approval', 'await_merge', 'await_info', 'not_a_bug'].map(node => [{ state: 'active', node }, 'needs_you'] as [TaskState, string]),
  ...['paused', 'stopped', 'failed'].map(state => [{ state }, 'needs_you'] as [TaskState, string]),
  [{ state: 'done' }, 'done'], [{ state: 'cancelled' }, null],
];
test('1 fourteen state shapes map to their designed columns', () => {
  expect(cases).toHaveLength(14);
  for (const node of Object.keys(NODE_LABELS)) expect(columnOf({ state: 'active', node })).toBe(['await_design_approval', 'await_merge', 'await_info', 'not_a_bug'].includes(node) ? 'needs_you' : 'running');
  for (const [state, expected] of cases) expect(columnOf(state), JSON.stringify(state)).toBe(expected);
});
test('2 awaiting nodes match Node::awaits_user', () => {
  const source = readFileSync(new URL('../../../crates/autome-domain/src/task.rs', import.meta.url), 'utf8');
  const method = source.split('pub const fn awaits_user(self)')[1].split('\n    }')[0];
  const actual = [...method.matchAll(/Node::(\w+)/g)].map(m => m[1].replace(/[A-Z]/g, c => `_${c.toLowerCase()}`).slice(1));
  expect([...AWAITING_NODES].sort()).toEqual(actual.sort());
});
test('3 paused waits; cancelled and unknown states are excluded', () => {
  expect(columnOf({ state: 'paused', resume: 'gate' })).toBe('needs_you');
  for (const state of ['cancelled', 'future', '']) expect(columnOf({ state })).toBeNull();
});
test('4 counts agree with classification including empty and singleton inputs', () => {
  const tasks = cases.map(([state]) => ({ state }));
  const counts = countByColumn(tasks);
  for (const column of ['todo', 'running', 'needs_you', 'done'] as const) expect(counts[column]).toBe(tasks.filter(t => columnOf(t.state) === column).length);
  expect(countByColumn([])).toEqual({ ['todo']: 0, running: 0, needs_you: 0, done: 0 });
  expect(countByColumn([{ state: { state: 'stopped' } }])).toEqual({ ['todo']: 0, running: 0, needs_you: 1, done: 0 });
});
test('5 real project.get counts agree with the shared classification', () => {
  const raw = JSON.parse(readFileSync(new URL('./fixtures/w5-project-get.json', import.meta.url), 'utf8'));
  const { tasks, counts } = raw.payload ?? raw;
  const actual = countByColumn(tasks);
  expect(actual.needs_you).toBe(['awaiting_user', 'failed', 'stopped', 'paused'].reduce((n, k) => n + (counts[k] ?? 0), 0));
  expect(actual.running).toBe(counts.running ?? 0);
  for (const state of ['draft', 'queued', 'active', 'stopped', 'failed']) expect(tasks.some((t: { state: TaskState }) => t.state.state === state)).toBe(true);
});
test('6 stage lookup covers every node and state carrier', () => {
  const expected = {
    feature: [
      ['整理', ['intake']], ['设计', ['design', 'review', 'adjudicate']], ['批准设计', ['await_design_approval']],
      ['实现', ['implement', 'gate']], ['审计', ['audit', 'retro']],
      ['合并', ['rebase', 'merge_gate', 'await_merge', 'merging', 'main_check', 'cleanup']],
    ],
    bug: [
      ['定位', ['locate', 'repro_check', 'await_info', 'not_a_bug']], ['修复', ['fix', 'fix_gate']], ['复核', ['fix_review']],
      ['合并', ['rebase', 'merge_gate', 'await_merge', 'merging', 'main_check', 'cleanup']],
    ],
  } as const;
  for (const kind of ['feature', 'bug'] as const) for (const [label, nodes] of expected[kind]) {
    for (const node of nodes) expect(stageOf(kind, { state: 'active', node })?.label).toBe(label);
  }
  for (const [kind, stages] of [['feature', FEATURE_STAGES], ['bug', BUG_STAGES]] as const) {
    stages.forEach((stage, index) => {
      for (const node of stage.nodes) for (const state of [{ state: 'active', node }, { state: 'paused', resume: node }, { state: 'queued', resume: node }, { state: 'stopped', at: node }, { state: 'failed', at: node }]) {
        expect(stageOf(kind, state)).toEqual({ label: stage.label, index, total: stages.length });
      }
    });
    expect(stageOf(kind, { state: 'done' })?.label).toBe('完成');
    for (const state of ['draft', 'queued', 'cancelled', 'unknown']) expect(stageOf(kind, { state })).toBeNull();
    expect(stageOf(kind, { state: 'active', node: 'future' })?.label).toBe('future');
  }
});
