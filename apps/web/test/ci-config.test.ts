import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { parse } from 'yaml';
import { expect, test } from 'vitest';

const root = resolve(import.meta.dirname, '../../..');
const workflow = parse(readFileSync(resolve(root, '.github/workflows/ci.yml'), 'utf8'));
const local = readFileSync(resolve(root, 'scripts/ci-local.sh'), 'utf8');
const web = workflow.jobs.web;
type Step = { name?: string; uses?: string; run?: string; with?: Record<string, string> };

test('Web job 使用 macOS 且限时 15 分钟', () => {
  expect(web['runs-on']).toBe('macos-14');
  expect(web['timeout-minutes']).toBe(15);
  expect(web.defaults.run['working-directory']).toBe('apps/web');
  expect(web.steps).toContainEqual({ uses: 'actions/checkout@v4' });
});

test('Node 22、npm 缓存与锁文件安装', () => {
  expect(web.steps.find((step: Step) => step.uses === 'actions/setup-node@v4').with).toEqual({
    'node-version': '22', cache: 'npm', 'cache-dependency-path': 'apps/web/package-lock.json',
  });
  expect(web.steps.find((step: Step) => step.name === 'Install').run).toBe('npm ci');
});

test('CI 按顺序运行四个 Web 检查', () => {
  expect(web.steps.filter((step: Step) => step.run).map((step: Step) => step.run)).toEqual([
    'npm ci', 'npm run lint', 'npm run typecheck', 'npm test', 'npm run build',
  ]);
});

test('所有 job 都有正数 timeout-minutes', () => {
  for (const job of Object.values(workflow.jobs) as Record<string, unknown>[]) {
    expect(job['timeout-minutes']).toBeTypeOf('number');
    expect(job['timeout-minutes']).toBeGreaterThan(0);
  }
});

test('本地只在缺 node_modules 时 npm ci 安装 Web', () => {
  expect(local).toContain("if [ ! -d apps/web/node_modules ]; then\n  run 'Web install' sh -c 'cd apps/web && npm ci'\nfi");
  expect(local.match(/run 'Web install'/g)).toHaveLength(1);
});

test('本地四行独立 run 位于 contract 后、package 分支外', () => {
  const section = local.slice(local.indexOf('# --- job: web'), local.indexOf('# --- job: package'));
  expect(local.indexOf('# --- job: web')).toBeGreaterThan(local.indexOf('# --- job: contract'));
  expect(section).toContain("fi\nrun 'Web lint'");
  expect(section.match(/^run 'Web .*$/gm)).toEqual(
    ['lint', 'typecheck', 'test', 'build'].map(name => `run 'Web ${name}' npm run ${name} --prefix apps/web`),
  );
});
