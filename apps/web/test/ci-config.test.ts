import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { parse } from 'yaml';
import { expect, test } from 'vitest';
import browserConfig from '../playwright.config.ts';

const root = resolve(import.meta.dirname, '../../..');
const workflow = parse(readFileSync(resolve(root, '.github/workflows/ci.yml'), 'utf8'));
const local = readFileSync(resolve(root, 'scripts/ci-local.sh'), 'utf8');
const steps = readFileSync(resolve(root, 'scripts/ci/steps.sh'), 'utf8');
const web = workflow.jobs.web;
test('串行浏览器套件为 29 例及初始化清理预留预算，仍受 CI 上限约束', () => {
  // 360 s already expired after 20 passing cases on the host. Budget the
  // 29-case suite at 20 s/case plus 120 s for both fixture servers and cleanup.
  expect(browserConfig.globalTimeout).toBeGreaterThanOrEqual(29 * 20_000 + 120_000);
  expect(browserConfig.globalTimeout).toBeLessThan(workflow.jobs.e2e['timeout-minutes'] * 60_000);
  expect(browserConfig.workers).toBe(1);
  expect(browserConfig.timeout).toBe(90_000);
  expect(browserConfig.retries).toBe(0);
});
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
  for (const job of ['cli-package', 'cli-release']) {
    expect(workflow.jobs[job].steps.find((step: Step) => step.uses === 'actions/setup-node@v4').with['node-version']).toBe('22');
  }
});

test('CI 按顺序运行 Web 检查，测试前先构建内核', () => {
  expect(web.steps.filter((step: Step) => step.run).map((step: Step) => step.run)).toEqual([
    'npm ci', 'npm run lint', 'npm run typecheck', 'cargo build -p automed -p autome-cli', 'npm run build', 'npm test',
  ]);
});

test('所有 job 都有正数 timeout-minutes', () => {
  for (const job of Object.values(workflow.jobs) as Record<string, unknown>[]) {
    expect(job['timeout-minutes']).toBeTypeOf('number');
    expect(job['timeout-minutes']).toBeGreaterThan(0);
  }
});

test('本地使用锁文件指纹安装 Web', () => {
  expect(steps).toContain("prep 'Web install' sh scripts/ci/install-node.sh apps/web");
  expect(steps.match(/prep 'Web install'/g)).toHaveLength(1);
});

test('本地五步独立声明完整输入，位于 Contract 后、package 分支外', () => {
  expect(steps.indexOf("step 'Web lint'")).toBeGreaterThan(steps.indexOf("step 'Contract'"));
  expect(steps.match(/^step 'Web .*$/gm)).toEqual(
    ['lint', 'typecheck', 'build', 'test', 'e2e'].map(name =>
      `step 'Web ${name}' '${['test', 'e2e'].includes(name) ? 'apps/web crates protocol plugins apps/desktop' : 'apps/web'}' ${name === 'test' ? '--cases vitest ' : ''}npm run ${name} --prefix apps/web`),
  );
  expect(local.indexOf('. "$root/scripts/ci/steps.sh"')).toBeLessThan(local.indexOf('if [ "$with_package" -eq 1 ]'));
});

test('本地门禁在 Web test 之前已构建内核', () => {
  const build = steps.indexOf("prep 'Build core' cargo build -p automed -p autome-cli");
  expect(build).toBeGreaterThanOrEqual(0);
  expect(build).toBeLessThan(steps.indexOf("step 'Web test'"));
});

test('CLI package 隔离 home 与 prefix，成功和安装失败均清理并保留退出码', () => {
  const fragment = local.match(/run 'CLI package' (sh -c '[\s\S]*?\n {2}')/)?.[1];
  expect(fragment).toBeDefined();
  const sandbox = mkdtempSync(resolve(tmpdir(), 'cli package 测试-'));
  try {
    mkdirSync(resolve(sandbox, 'scripts'));
    mkdirSync(resolve(sandbox, 'tmp'));
    writeFileSync(resolve(sandbox, 'scripts/package-cli.sh'), 'exit 0\n');
    writeFileSync(resolve(sandbox, 'scripts/install-cli.sh'), `
set -eu
printf '%s\\n%s\\n' "$PREFIX" "$AUTOME_HOME" > paths
test -d "$PREFIX"
test -d "$AUTOME_HOME"
printf installed > "$AUTOME_HOME/marker"
test "$1" = --from
if [ "$INSTALL_EXIT" -ne 0 ]; then exit "$INSTALL_EXIT"; fi
printf '#!/bin/sh\\nprintf version > version-ran\\n' > "$PREFIX/autome"
chmod +x "$PREFIX/autome"
`);
    for (const status of [0, 37]) {
      rmSync(resolve(sandbox, 'version-ran'), { force: true });
      const result = spawnSync('sh', ['-c', fragment!], {
        cwd: sandbox,
        env: { ...process.env, TMPDIR: resolve(sandbox, 'tmp'),
          AUTOME_HOME: resolve(sandbox, 'caller-home'), PREFIX: resolve(sandbox, 'caller-prefix'),
          INSTALL_EXIT: String(status) },
        encoding: 'utf8', timeout: 10_000,
      });
      expect(result.error).toBeUndefined();
      expect(result.status, result.stderr).toBe(status);
      const paths = readFileSync(resolve(sandbox, 'paths'), 'utf8').trim().split('\n');
      expect(paths).toHaveLength(2);
      expect(paths[0]).not.toBe(paths[1]);
      for (const path of paths) {
        expect(path).not.toBe(resolve(sandbox, 'caller-home'));
        expect(path).not.toBe(resolve(sandbox, 'caller-prefix'));
        expect(existsSync(path)).toBe(false);
      }
      expect(readdirSync(resolve(sandbox, 'tmp'))).toEqual([]);
      expect(existsSync(resolve(sandbox, 'version-ran'))).toBe(status === 0);
      expect(existsSync(resolve(sandbox, 'caller-home'))).toBe(false);
      expect(existsSync(resolve(sandbox, 'caller-prefix'))).toBe(false);
    }
  } finally {
    rmSync(sandbox, { recursive: true, force: true });
  }
});

test('Rust 一次跑完所有目标，脚本测试纳入本地与 CI', () => {
  expect(steps).toContain('cargo test --workspace --all-targets --no-fail-fast -- --nocapture');
  for (const name of ['Test', 'End to end', 'Adapter flags exist']) {
    expect(workflow.jobs.rust.steps.find((step: Step) => step.name === name).run).toContain('--no-fail-fast');
  }
  expect(workflow.jobs.scripts.steps.some((step: Step) => step.run === 'node --test scripts/test/*.test.mjs')).toBe(true);
  expect(steps).toContain("step 'Scripts' '' node --test scripts/test/*.test.mjs");
});

test('Bundle 校验 autome 并运行打包面板冒烟，Desktop 保留回归', () => {
  expect(local).toContain('Resources/core/autome" --version');
  expect(local).toContain("run 'Bundle launches the panel'");
  const steps: Step[] = workflow.jobs.package.steps;
  expect(steps.find(step => step.name === 'The bundle carries a runnable core')?.run).toContain('Resources/core/autome" --version');
  expect(steps.find(step => step.name === 'Bundle launches the panel')?.run).toContain('npm --prefix apps/desktop run smoke');
  expect(workflow.jobs.desktop.steps.some((step: Step) => step.run === 'cargo build -p automed')).toBe(false);
  expect(workflow.jobs.desktop.steps.some((step: Step) => step.run === 'cargo build -p autome-cli')).toBe(true);
});

test('e2e job 安装指定浏览器并独立运行限时浏览器测试', () => {
  const job = workflow.jobs.e2e;
  expect(job['runs-on']).toBe('macos-14');
  expect(job['timeout-minutes']).toBe(15);
  expect(job.steps.filter((step: Step) => step.run).map((step: Step) => step.run)).toEqual([
    'npm ci', 'cargo build -p automed -p autome-cli', 'npm run build', 'npx playwright install chromium', 'npm run e2e',
  ]);
  expect(job.steps.find((step: Step) => step.uses === 'actions/cache@v4').with.key).toContain('1.64.1');
});

test('两处依赖目录均比较锁文件指纹且安装成功才写入', () => {
  for (const [directory, label] of [['web', 'Web'], ['desktop', 'Desktop']]) {
    expect(steps).toContain(`prep '${label} install' sh scripts/ci/install-node.sh apps/${directory}`);
  }
  const installer = readFileSync(resolve(root, 'scripts/ci/install-node.sh'), 'utf8');
  expect(installer).toContain('set -eu');
  expect(installer).toContain('shasum -a 256 package-lock.json');
  expect(installer).toContain('cat node_modules/.autome-lock.sha256');
  expect(installer.indexOf('npm ci')).toBeLessThan(installer.indexOf('> node_modules/.autome-lock.sha256'));
});
