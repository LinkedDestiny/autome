import { expect, test } from 'vitest';
import { dirtyText } from '../scripts/dirty-text.ts';
test('四种泄漏字面量都被检出，包括中文紧邻与重复', () => {
  for (const value of ['undefined', 'NaN', 'null', '[object Object]']) expect(dirtyText(`值：${value}。 ${value}`)).toEqual([value, value]);
});
test('普通词、空串与合法数字不误判', () => {
  for (const value of ['', 'nullable NaNa undefinedValue', '0 123.45', 'null_value']) expect(dirtyText(value)).toEqual([]);
});
