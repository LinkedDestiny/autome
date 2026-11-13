import { readFileSync } from 'node:fs';
import { expect, test } from 'vitest';
test('添加页限高与补全/成员内部滚动', () => {
  const css = readFileSync(new URL('../app/styles/app.css', import.meta.url), 'utf8');
  const rule = (selector: string) => css.split(selector + ' {')[1]?.split('}')[0];
  expect(rule('.add-project')).toContain('height: 100%'); expect(rule('.add-project')).toContain('overflow: hidden');
  for (const selector of ['.path-suggestions', '.probe-members']) { expect(rule(selector)).toContain('overflow-y: auto'); expect(rule(selector)).toContain('min-height: 0'); }
});
