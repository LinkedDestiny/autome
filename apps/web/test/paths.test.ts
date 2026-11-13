import { mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync, realpathSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, expect, test } from 'vitest';
import { createPathGuard, resolveUserHome } from '../server/paths.ts';
let root: string, home: string, guard: ReturnType<typeof createPathGuard>;
beforeEach(() => { root = mkdtempSync(join(tmpdir(), 'w9-paths-')); home = join(root, 'home'); mkdirSync(home); guard = createPathGuard({ home }); });
afterEach(() => rmSync(root, { recursive: true, force: true }));
const dir = (name: string) => mkdirSync(join(home, name), { recursive: true });
test('目录、码点序与显示路径；不存在及读盘失败', () => {
  for (const name of ['z', 'B', 'a']) dir(name);
  writeFileSync(join(home, 'file'), 'x');
  expect(guard.complete('')).toEqual({ base: '~/', entries: ['B', 'a', 'z'].map(name => ({ name, path: '~/' + name })), truncated: false });
  expect(guard.complete('~/missing/')).toEqual({ base: '~/missing/', entries: [], truncated: false });
  expect(guard.complete('~/file/')).toHaveProperty('error');
});
test('拒绝所有 .. 子串与不支持的路径形式', () => {
  for (const input of ['~/a/..', '~/a..b', 'relative', '~/bad\\name']) {
    expect(guard.check(input).ok).toBe(false); expect(guard.complete(input)).toHaveProperty('error');
  }
});
test('NUL、长度、类型及缺少 HOME 边界', () => {
  for (const input of ['~/bad\0', '', '~/' + 'a'.repeat(511), null, 1]) expect(guard.check(input).ok).toBe(false);
  expect(guard.complete('~/bad\0')).toHaveProperty('error');
  const long = '~/' + Array(5).fill('a'.repeat(101)).join('/');
  expect(long.length).toBe(511); expect(guard.check(long).ok).toBe(true);
  expect(guard.check(long + 'a').ok).toBe(true); expect(guard.check(long + 'aa').ok).toBe(false);
  expect(createPathGuard({ home: '' }).check('~')).toMatchObject({ ok: false, message: expect.stringContaining('HOME') });
  expect(createPathGuard({ home: join(root, 'missing') }).check('~').ok).toBe(false);
  expect(resolveUserHome({ HOME: home + '/' })).toBe(home);
});
test('绝对路径只允许 HOME 内', () => {
  dir('a'); for (const path of ['/etc', root, home + '-sibling']) expect(guard.check(path).ok).toBe(false);
  expect(guard.check(join(home, 'a'))).toMatchObject({ ok: true, display: '~/a' });
});
test('符号链接逃逸完整输入拒绝，部分前缀正常', () => {
  symlinkSync(root, join(home, 'escape')); symlinkSync(join(root, 'missing'), join(home, 'broken'));
  expect(guard.complete('~/')).toMatchObject({ entries: [] });
  for (const input of ['~/escape', '~/escape/', '~/escape/sub']) {
    expect(guard.check(input).ok).toBe(false); expect(guard.complete(input)).toHaveProperty('error');
  }
  expect(guard.check('~/esc').ok).toBe(true); expect(guard.complete('~/esc')).toMatchObject({ entries: [] });
  expect(guard.check('~/broken/sub').ok).toBe(false);
});
test('49、50、51、60 项边界', () => {
  for (let n = 0; n < 60; n++) { dir(String(n).padStart(2, '0')); if ([48, 49, 50, 59].includes(n)) {
    const result = guard.complete('~/'); expect(result).toMatchObject({ truncated: n >= 50 });
    if ('entries' in result) expect(result.entries).toHaveLength(Math.min(n + 1, 50));
  } }
});
test('隐藏目录由前缀控制；单元素及区分大小写', () => {
  dir('.hidden'); dir('Alpha'); expect(guard.complete('~')).toMatchObject({ entries: [{ name: 'Alpha', path: '~/Alpha' }] });
  expect(guard.complete('~/.')).toMatchObject({ entries: [{ name: '.hidden', path: '~/.hidden' }] });
  expect(guard.complete('~/a')).toMatchObject({ entries: [] });
});
test('HOME 本身是符号链接', () => {
  dir('a'); const alias = join(root, 'alias'); symlinkSync(home, alias);
  const linked = createPathGuard({ home: alias });
  expect(linked.check('~/a')).toEqual({ ok: true, absolute: realpathSync(join(home, 'a')), display: '~/a', isHome: false });
  expect(linked.complete('~/')).toMatchObject({ entries: [{ name: 'a', path: '~/a' }] });
});
test('最深祖先确定未创建目录落点', () => {
  dir('a'); expect(guard.check('~/a/new/nested')).toMatchObject({ ok: true, absolute: join(realpathSync(home), 'a/new/nested') });
  expect(guard.check(join(root, 'new/nested')).ok).toBe(false);
  symlinkSync(join(home, 'a'), join(home, 'alias'));
  expect(guard.check('~/alias/new')).toMatchObject({ ok: true, absolute: join(realpathSync(home), 'a/new') });
});
