import { readFileSync, readdirSync } from 'node:fs';
import { resolve } from 'node:path';
import { expect, test } from 'vitest';
import config from '../vitest.config.ts';
import { testGroups } from '../test-groups.ts';

const web = resolve(import.meta.dirname, '..');
const projects = config.test!.projects as { test: { name: string; include: string[]; fileParallelism?: boolean; maxWorkers?: number; sequence: { groupOrder: number } } }[];
test('每个测试文件恰属一个 project', () => {
  const files = readdirSync(resolve(web, 'test')).filter(f => f.endsWith('.test.ts')).map(f => `test/${f}`).sort();
  const assigned = projects.flatMap(p => p.test.include).sort();
  expect(assigned).toEqual(files);
  expect(new Set(assigned).size).toBe(assigned.length);
});
test('内核、端口、进程及构建测试不进入 unit', () => {
  const marker = /target\/debug|CoreSupervisor|AutomedSidecar|w9Kernel|fixtures\/.*sh|startServer\(|createHttpServer\(|createServer\(|listen\(|spawn\(['"]npm|npm run build|ps -axo|process-leak/;
  for (const file of testGroups.unit) expect(readFileSync(resolve(web, file), 'utf8'), file).not.toMatch(marker);
});
test('exclusive 串行，kernel 有限并行，三组同时启动且预构建', () => {
  expect(projects.map(p => p.test.name).sort()).toEqual(['exclusive', 'kernel', 'unit']);
  expect(projects.find(p => p.test.name === 'exclusive')!.test.fileParallelism).toBe(false);
  expect(projects.find(p => p.test.name === 'kernel')!.test.maxWorkers).toBe(4);
  expect(projects.every(p => p.test.sequence.groupOrder === 0)).toBe(true);
  expect(config.test!.fileParallelism).not.toBe(false);
  expect(config.test!.globalSetup).toEqual(['./test/fixtures/global-setup.ts']);
});
test('配置与成员清单不按环境增减测试', () => {
  for (const file of ['vitest.config.ts', 'test-groups.ts']) {
    expect(readFileSync(resolve(web, file), 'utf8')).not.toMatch(/process\.env|import\.meta\.env/);
  }
});
