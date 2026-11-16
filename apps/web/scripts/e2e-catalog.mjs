/** @typedef {{title: string, suites?: ListedSuite[], specs?: {file: string, title: string, tests: {projectName?: string}[]}[]}} ListedSuite */
import { spawnSync } from 'node:child_process';
import { readFileSync, readdirSync, realpathSync } from 'node:fs';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

/** @param {string} web */
export function preparationNames(web) {
  const harness = join(web, 'e2e/harness');
  const names = new Set(['globalSetup', 'globalTeardown', 'setup', 'teardown', 'prepare()']);
  for (const file of readdirSync(harness).filter(name => name.endsWith('.ts'))) {
    names.add(file);
    for (const match of readFileSync(join(harness, file), 'utf8').matchAll(/test\/fixtures\/([\w.-]+)/g)) names.add(match[1]);
  }
  return [...names];
}
/** @param {{file: string, title: string}[]} cases @param {string[]} preparation */
export function renderCatalog(cases, preparation) {
  /** @param {string} value */
  const clean = value => String(value).replace(/[\t\r\n]/g, ' ');
  return ['autome-gate-cases begin Web e2e', ...cases.map(c => `case\t${clean(basename(c.file))}\t${clean(c.title)}`), ...preparation.map(name => `setup\t${clean(name)}`), 'autome-gate-cases end'].join('\n');
}
/** @param {string} web */
export function catalog(web) {
  const child = spawnSync(process.execPath, [join(web, 'node_modules/@playwright/test/cli.js'), 'test', '--list', '--reporter=json'], {
    cwd: web, encoding: 'utf8', timeout: 30_000, maxBuffer: 16 * 1024 * 1024,
    env: { ...process.env, NO_COLOR: '1', FORCE_COLOR: '0' },
  });
  if (child.error || child.status !== 0) throw Error(child.error?.message || child.stderr || `--list exit ${child.status}`);
  const report = JSON.parse(child.stdout);
  if (report.errors?.length) throw Error(JSON.stringify(report.errors));
  /** @type {{file: string, title: string}[]} */
  const cases = [];
  /** @param {{title: string, suites?: ListedSuite[], specs?: {file: string, title: string, tests: {projectName?: string}[]}[]}} suite @param {string[]} parents */
  function visit(suite, parents) {
    const path = [...parents, suite.title];
    for (const spec of suite.specs || []) for (const test of spec.tests) cases.push({ file: spec.file, title: [...path, spec.title, test.projectName].filter(Boolean).join(' › ') });
    for (const child of suite.suites || []) visit(child, path);
  }
  for (const suite of report.suites || []) visit(suite, []);
  return renderCatalog(cases, preparationNames(web));
}
if (process.argv[1] && realpathSync(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { console.log(catalog(resolve(dirname(fileURLToPath(import.meta.url)), '..'))); }
  catch (error) { console.log(`未取得浏览器用例清单：${error instanceof Error ? error.message : String(error)}`); }
}
