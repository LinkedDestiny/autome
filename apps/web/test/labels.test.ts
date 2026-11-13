import { readFileSync, readdirSync } from 'node:fs';
import { resolve } from 'node:path';
import { test, expect } from 'vitest';
import { FEATURE_STAGES, BUG_STAGES, NODE_LABELS, STATE_LABELS, KIND_LABELS, COLUMN_LABELS, nodeLabel, stageOf, type Stage } from '../app/lib/labels.ts';
const source = readFileSync(new URL('../../../crates/autome-domain/src/task.rs', import.meta.url), 'utf8');
const snake = (s: string) => s.replace(/[A-Z]/g, c => `_${c.toLowerCase()}`).slice(1);
const variants = (s: string, name: string) => [...s.split(`enum ${name} {`)[1].split('\n}')[0].matchAll(/^ {4}([A-Z]\w*)\s*[,{]/gm)].map(m => snake(m[1]));
const nodes = (s: string) => [...s.matchAll(/Node::([A-Z]\w*)/g)].map(m => snake(m[1]));
const list = (s: string, name: string) => nodes(s.split(`pub const ${name}:`)[1].split('];')[0]);
const sorted = (xs: readonly string[]) => [...xs].sort();
function coverage(stages: readonly Stage[], expected: string[]) {
  const actual = stages.flatMap(s => s.nodes);
  return { missing: expected.filter(n => !actual.includes(n)), extra: actual.filter(n => !expected.includes(n)), duplicates: actual.filter((n, i) => actual.indexOf(n) !== i) };
}
const clean = { missing: [], extra: [], duplicates: [] };
test('1 enum Node matches ALL and BUG', () => {
  expect(sorted(variants(source, 'Node'))).toEqual(sorted([...list(source, 'ALL'), ...list(source, 'BUG')]));
  expect(variants(source, 'Node')).toHaveLength(22);
});
test('2 feature stages cover every ALL node exactly once', () => {
  expect(FEATURE_STAGES.map(s => s.label)).toEqual(['整理', '设计', '批准设计', '实现', '审计', '合并', '完成']);
  expect(coverage(FEATURE_STAGES, list(source, 'ALL'))).toEqual(clean);
});
test('3 bug stages cover BUG and shared merge nodes', () => {
  expect(BUG_STAGES.map(s => s.label)).toEqual(['定位', '修复', '复核', '合并', '完成']);
  const merge = BUG_STAGES[3].nodes;
  expect(merge).toEqual(FEATURE_STAGES[5].nodes);
  expect(coverage(BUG_STAGES, [...list(source, 'BUG'), ...merge])).toEqual(clean);
  for (const n of merge) expect(list(source, 'ALL')).toContain(n);
  const bug = readFileSync(new URL('../../../crates/autome-domain/src/bug_loop.rs', import.meta.url), 'utf8').split('#[cfg(test)]')[0];
  for (const n of nodes(bug).filter(n => !list(source, 'BUG').includes(n))) expect(merge).toContain(n);
});
test('4 node labels and unknown fallback', () => {
  expect(sorted(Object.keys(NODE_LABELS))).toEqual(sorted(variants(source, 'Node')));
  for (const n of ['future_node', '', '__proto__', 'toString']) {
    expect(nodeLabel(n)).toBe(n);
    expect(stageOf('feature', { state: 'active', node: n })).toEqual({ label: n, index: -1, total: 7 });
  }
});
test('5 state and kind labels match serialized enums', () => {
  expect(sorted(Object.keys(STATE_LABELS))).toEqual(sorted(variants(source, 'TaskStateRepr')));
  expect(sorted(Object.keys(KIND_LABELS))).toEqual(sorted(variants(source, 'TaskKind')));
});
test('6 contract checker detects new, missing and extra nodes', () => {
  const changed = source.replace('enum Node {', 'enum Node {\n    NewNode,').replace('pub const ALL: [Node; 15] = [', 'pub const ALL: [Node; 16] = [\n        Node::NewNode,');
  expect(variants(changed, 'Node')).toContain('new_node');
  expect(coverage(FEATURE_STAGES, list(changed, 'ALL')).missing).toEqual(['new_node']);
  expect(coverage(FEATURE_STAGES.map(s => ({ ...s, nodes: s.nodes.filter(n => n !== 'gate') })), list(source, 'ALL')).missing).toEqual(['gate']);
  expect(coverage([...FEATURE_STAGES, { label: '', nodes: ['ghost'] }], list(source, 'ALL')).extra).toEqual(['ghost']);
});
test('7 views do not duplicate status, column or stage Chinese literals', () => {
  const labels = [...Object.values(STATE_LABELS), ...Object.values(COLUMN_LABELS), ...FEATURE_STAGES.map(s => s.label), ...BUG_STAGES.map(s => s.label)];
  for (const dir of ['routes', 'components']) {
    const root = resolve('app', dir);
    for (const file of readdirSync(root, { recursive: true }) as string[]) {
      if (!file.endsWith('.tsx')) continue;
      const text = readFileSync(resolve(root, file), 'utf8');
      for (const label of labels) expect(text, `${file}: ${label}`).not.toContain(label);
    }
  }
});
test('8 runtime labels match serialized Runtime variants', async () => {
  const { RUNTIME_LABELS } = await import('../app/lib/labels.ts');
  const role = readFileSync(new URL('../../../crates/autome-domain/src/role.rs', import.meta.url), 'utf8');
  expect(sorted(Object.keys(RUNTIME_LABELS))).toEqual(sorted(variants(role, 'Runtime')));
});
