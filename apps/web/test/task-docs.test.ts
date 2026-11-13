import { mkdtempSync, writeFileSync, rmSync, symlinkSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, expect, it, vi } from 'vitest';
import { createTaskDocs, DOC_LIMIT_BYTES, pickDocument } from '../server/task-docs.ts';
import { loadTaskDetail } from '../app/components/task-detail-data.ts';
const roots: string[] = [];
function fixture() { const root = mkdtempSync(join(tmpdir(), 'w7-docs-')); roots.push(root); return root; }
afterEach(() => { for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true }); });
const docs = [{ name: 'design.md', absolute: '/kernel/first' }, { name: 'design.md', absolute: '/kernel/second' }];
function load(name: string, read = vi.fn()) {
  return { read, result: loadTaskDetail({ gateway: { read: vi.fn().mockResolvedValue({ status: 200, body: { ok: true, payload: { task: { id: 'T-1' }, sessions: [], documents: docs } } }), write: vi.fn() }, taskDocs: { read } }, 'T-1', new URL(`http://localhost/tasks/T-1?doc=${encodeURIComponent(name)}`)) };
}
it('全等匹配并取同名第一项', () => {
  expect(pickDocument(docs, 'design.md')).toEqual({ ok: true, entry: docs[0] });
  expect(pickDocument(docs, 'Design.md').ok).toBe(false);
});
it('五种路径形状拒绝且不读文件', async () => {
  for (const name of ['a/b', 'a\\b', '..', '~file', '\0']) {
    const { read, result } = load(name); expect((await result).document).toMatchObject({ error: '已拒绝：文档名不能是路径' }); expect(read).not.toHaveBeenCalled();
  }
});
it('清单没有或空名称不读盘', async () => {
  for (const name of ['', 'unknown.md']) {
    const { read, result } = load(name); expect((await result).document).toMatchObject({ error: expect.stringContaining('没有名为') }); expect(read).not.toHaveBeenCalled();
  }
});
it('200 KB 上限换行截断；空文件、恰好上限、无换行多字节边界', async () => {
  const path = join(fixture(), 'doc'); const reader = createTaskDocs();
  writeFileSync(path, 'a\n' + 'b'.repeat(DOC_LIMIT_BYTES));
  expect(await reader.read(path, DOC_LIMIT_BYTES)).toEqual({ text: 'a\n', size: DOC_LIMIT_BYTES + 2, truncated: true });
  writeFileSync(path, 'x'.repeat(DOC_LIMIT_BYTES)); expect(await reader.read(path, DOC_LIMIT_BYTES)).toMatchObject({ truncated: false, size: DOC_LIMIT_BYTES });
  writeFileSync(path, ''); expect(await reader.read(path, DOC_LIMIT_BYTES)).toEqual({ text: '', size: 0, truncated: false });
  writeFileSync(path, '中'.repeat(DOC_LIMIT_BYTES)); const value = await reader.read(path, DOC_LIMIT_BYTES); expect(value.text).not.toContain('�'); expect(Buffer.byteLength(value.text!)).toBeLessThanOrEqual(DOC_LIMIT_BYTES);
});
it('文件消失、非普通文件与非法上限有错误；符号链接拒绝', async () => {
  const root = fixture(), reader = createTaskDocs();
  expect(await reader.read(join(root, 'gone'), DOC_LIMIT_BYTES)).toMatchObject({ error: '文档已不在（可能刚被合并清理），刷新后重试' });
  expect(await reader.read(root, DOC_LIMIT_BYTES)).toMatchObject({ error: '文档不是普通文件' });
  const path = join(root, 'doc'); writeFileSync(path, 'ok'); symlinkSync(path, join(root, 'link'));
  expect(await reader.read(join(root, 'link'), DOC_LIMIT_BYTES)).toMatchObject({ error: '文档不是普通文件' });
  expect((await reader.read(path, 0)).error).toContain('正整数');
});
it('reader 收到本次清单 absolute 原样值与上限', async () => {
  const { read, result } = load('design.md', vi.fn().mockResolvedValue({ text: 'ok', size: 2, truncated: false }));
  expect((await result).document).toMatchObject({ text: 'ok' }); expect(read).toHaveBeenCalledExactlyOnceWith('/kernel/first', DOC_LIMIT_BYTES);
});
