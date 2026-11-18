import { readFileSync } from 'node:fs';
import { expect, test } from 'vitest';
test('添加页限高与补全/成员内部滚动', () => {
  const css = readFileSync(new URL('../app/styles/app.css', import.meta.url), 'utf8');
  const rule = (selector: string) => css.split(selector + ' {')[1]?.split('}')[0];
  expect(rule('.add-project')).toContain('height: 100%'); expect(rule('.add-project')).toContain('overflow: hidden');
  for (const selector of ['.path-suggestions', '.probe-members']) { expect(rule(selector)).toContain('overflow-y: auto'); expect(rule(selector)).toContain('min-height: 0'); }
});
test('设置页整体滚动且卡片自然高度，编辑器内部滚动', () => {
  const css = readFileSync(new URL('../app/styles/app.css', import.meta.url), 'utf8'); const rule = (selector: string) => css.split(selector + ' {')[1]?.split('}')[0];
  for (const value of ['flex: 1', 'min-height: 0', 'overflow-y: auto']) expect(rule('.project-settings')).toContain(value);
  expect(rule('.settings-card-body')).toContain('flex: none');
  expect(css).not.toMatch(/\.settings-card-body\s*\{[^}]*?(overflow-y|flex: 1 1 0)/);
  for (const selector of ['.onboarding-editor textarea']) { expect(rule(selector)).toContain('overflow-y: auto'); expect(rule(selector)).toContain('min-height: 0'); }
});
test('报警区限高且内部滚动，详情保留换行', () => {
  const css = readFileSync(new URL('../app/styles/app.css', import.meta.url), 'utf8');
  const rule = css.split('.board-alerts {')[1]?.split('}')[0];
  for (const value of ['min-height: 0', 'max-height:', 'overflow-y: auto']) expect(rule).toContain(value);
  expect(css.split('.project-alert-detail {')[1]?.split('}')[0]).toContain('white-space: pre-wrap');
});
test('环境页限高与组件内部滚动', () => {
  const css=readFileSync(new URL('../app/styles/app.css',import.meta.url),'utf8'); const rule=(s:string)=>css.split(s+' {')[1]?.split('}')[0];
  for(const value of ['height: 100%','overflow: hidden'])expect(rule('.env-page')).toContain(value);
  for(const value of ['overflow-y: auto','min-height: 0'])expect(rule('.env-components')).toContain(value);
});
