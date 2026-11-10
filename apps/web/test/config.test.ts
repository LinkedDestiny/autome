import { expect, test } from 'vitest';
import { resolvePort } from '../server/config.ts';

test('未设端口默认 7420', () => { expect(resolvePort(undefined)).toBe(7420); });
test('端口 0 分配空闲端口', () => { expect(resolvePort('0')).toBe(0); });
test('显式端口 8123', () => { expect(resolvePort('8123')).toBe(8123); });
test('非整数报中文错误', () => { expect(() => resolvePort('abc')).toThrow('端口必须是'); });
test('超出范围报中文错误', () => { expect(() => resolvePort('70000')).toThrow('端口必须是'); });
