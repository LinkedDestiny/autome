import { readFileSync, readdirSync } from 'node:fs';
import { resolve } from 'node:path';
import { expect, test } from 'vitest';

const root = resolve(import.meta.dirname, '../../..');
const readme = readFileSync(resolve(root, 'README.md'), 'utf8');
const names = ['dashboard.png', 'project.png', 'task.png'];
test('README 只引用三张存在的面板图片', () => {
  const references = [...readme.matchAll(/!\[[^\]]*\]\(docs\/images\/([^)]+)\)/g)].map(match => match[1]);
  expect(references.sort()).toEqual(names);
  expect(readdirSync(resolve(root, 'docs/images')).sort()).toEqual(names);
  for (const name of names) expect(readFileSync(resolve(root, 'docs/images', name)).length).toBeGreaterThan(24);
});
test('三张图片都是 1512×944 PNG', () => {
  for (const name of names) {
    const data = readFileSync(resolve(root, 'docs/images', name));
    expect(data.subarray(0, 8).toString('hex'), name).toBe('89504e470d0a1a0a');
    expect(data.subarray(12, 16).toString(), name).toBe('IHDR');
    expect([data.readUInt32BE(16), data.readUInt32BE(20)], name).toEqual([1512, 944]);
  }
});
test('README 使用 Web 截图脚本，不再引用桌面截图工具', () => {
  expect(readme).toContain('apps/web/scripts/screenshots.ts');
  expect(readme).not.toMatch(/apps\/desktop\/tools\/\s*screenshots\.js/);
});
