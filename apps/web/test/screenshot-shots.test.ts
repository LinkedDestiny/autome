import { readFileSync, readdirSync } from 'node:fs';
import { resolve } from 'node:path';
import { expect, test } from 'vitest';
import { parseScreenshotArgs, screenshotShots } from '../scripts/screenshot-shots.ts';
import { TEST_IDS } from '../app/lib/testids.ts';

const root = resolve(import.meta.dirname, '../../..');
test('默认集合恰为 README 五张且默认参数不变', () => {
  expect(parseScreenshotArgs([])).toEqual({ set: 'readme', out: 'docs/images' });
  expect(screenshotShots('readme', 'p', 't').map(s => `${s.name}.png`).sort()).toEqual(['dashboard.png', 'loop.png', 'project.png', 'skills.png', 'task.png']);
});
test('all 十张与改前附件一一对应且没有重复', () => {
  const names = screenshotShots('all', 'p', 't').map(s => `${s.name}.png`).sort();
  expect(names).toEqual(readdirSync(resolve(root, 'docs/面板观感收尾/attachments')).filter(n => n.endsWith('.png')).sort());
  expect(new Set(names).size).toBe(10);
});
test('两个抽屉通过稳定 testid 与对应页面打开', () => {
  const shots = screenshotShots('all', 'p', 't').filter(s => s.click);
  expect(shots).toEqual([
    { name: 'loop-drawer', path: '/loop', click: { testId: TEST_IDS.loopRole, button: true } },
    { name: 'skills-click', path: '/skills', click: { testId: TEST_IDS.skillCard } },
  ]);
  for (const shot of shots) expect(Object.values(TEST_IDS)).toContain(shot.click!.testId);
});
test('参数支持 all 和输出目录以及交换顺序，未知集合拒绝', () => {
  expect(parseScreenshotArgs(['--set', 'all', '--out', '/tmp/对照图'])).toEqual({ set: 'all', out: '/tmp/对照图' });
  expect(parseScreenshotArgs(['--out', '/tmp/对照图', '--set', 'all'])).toEqual({ set: 'all', out: '/tmp/对照图' });
  expect(() => parseScreenshotArgs(['--set', 'nope'])).toThrow('未知截图集合');
});
test('空值、缺值、重复和未知参数拒绝，all 必须指定目录', () => {
  for (const args of [['--out'], ['--out', ' '], ['--set', ''], ['--out', '--set'], ['--wat', 'x'], ['--set', 'all'], ['--out', 'a', '--out', 'b']]) {
    expect(() => parseScreenshotArgs(args), JSON.stringify(args)).toThrow();
  }
  expect(parseScreenshotArgs(['--out', '单目录'])).toEqual({ set: 'readme', out: '单目录' });
});
test('动态 ID 编码且项目信息、默认值均有独立拍摄步骤', () => {
  const shots = screenshotShots('all', 'a/b 空', 't?#');
  expect(shots.find(s => s.name === 'project')!.path).toBe('/projects/a%2Fb%20%E7%A9%BA');
  expect(shots.find(s => s.name === 'task')!.path).toBe('/tasks/t%3F%23');
  expect(shots.find(s => s.name === 'loop-projinfo')!.path).toBe('/loop?project=a%2Fb%20%E7%A9%BA&tab=project');
  expect(shots.find(s => s.name === 'loop-bottom')!.scroll).toBe('.loop-defaults');
});
test('执行脚本逐张在点击、滚动之后检查整页脏文本再截图', () => {
  const source = readFileSync(resolve(root, 'apps/web/scripts/screenshots.ts'), 'utf8');
  const loop = source.slice(source.indexOf('for (const { name, path, click, scroll } of shots)'));
  expect(loop).toContain("page.getByTestId(click.testId).first()");
  expect(loop).toContain("target.getByRole('button')");
  const check = loop.indexOf("expect(dirtyText(await page.locator('body').innerText()), name).toEqual([])");
  expect(check).toBeGreaterThan(loop.indexOf('scrollIntoViewIfNeeded'));
  expect(loop.indexOf('await page.screenshot')).toBeGreaterThan(check);
});
