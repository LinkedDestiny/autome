import { readFileSync } from 'node:fs';
import { runInNewContext } from 'node:vm';
import { expect, test } from 'vitest';

// Execute the actual B-3 poll body, so reverting its read order breaks this test.
const source = readFileSync(new URL('../e2e/controls.spec.ts', import.meta.url), 'utf8');
const body = source.match(/cli\(f.root, \['merge', parent\]\);\s*await expect.poll\(\(\) => \{([\s\S]*?)\}, \{ timeout: 60_000 \}\).toBe\('done'\)/)?.[1];
if (!body) throw new Error('B-3 merge poll not found');

function poll(read: (id: string) => { state: string }) {
  return runInNewContext(`(() => {${body}})()`, {
    f: { root: 'fixture' }, parent: 'parent', child: 'child', expect,
    taskState: (_root: string, id: string) => read(id),
  });
}

test('B-3 两次读取之间前置合并、子任务启动不会误红', () => {
  let reads = 0;
  const result = poll(id => {
    // Audit #7 interleaving: commit both transitions after the first snapshot.
    const merged = reads++ > 0;
    return { state: id === 'parent' ? (merged ? 'done' : 'active') : (merged ? 'active' : 'queued') };
  });
  expect(reads).toBe(2);
  expect(result).toBe('done');
});

test.each([
  ['queued', 'active'],
  ['queued', 'done'],
  ['active', 'done'],
])('B-3 子任务 %s、前置 %s 保留轮询结果', (child, parent) => {
  expect(poll(id => ({ state: id === 'parent' ? parent : child }))).toBe(parent);
});

test('B-3 子任务提前 active 而前置未 done 仍拒绝', () => {
  expect(() => poll(() => ({ state: 'active' }))).toThrow();
});
