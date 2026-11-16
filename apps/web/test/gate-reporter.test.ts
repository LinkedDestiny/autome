import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import ts from 'typescript';
import { afterEach, expect, test, vi } from 'vitest';
import type { FullConfig, FullResult, Suite, TestCase, TestResult, TestStep } from '@playwright/test/reporter';
import GateReporter, { literalBudget, margin } from '../e2e/harness/gate-reporter.ts';
import { recordTiming, timedExpect } from '../e2e/harness/budget.ts';
import { register, reapRegistered } from '../e2e/harness/reap.ts';
import { leftoverProcesses } from './fixtures/process-scan.ts';
vi.mock('./fixtures/process-scan.ts', () => ({ leftoverProcesses: vi.fn() }));
const roots: string[] = [];
test('截断报告等待退出回调后再复查和删除登记根', async () => {
  const h = harness();
  const root = join(h.root, 'owned'); mkdirSync(root);
  register({ root, pid: 12345, group: 12345 });
  vi.stubEnv('CI_E2E_REAP_MS', '200');
  let exited = false;
  vi.mocked(leftoverProcesses).mockImplementation(() => exited ? [] : [{ pid: 12345, parent: process.pid, command: root }]);
  const kill = vi.spyOn(process, 'kill').mockImplementation(() => {
    setImmediate(() => { exited = true; });
    return true;
  });
  const { report } = await h.end(2000, 'timedout');
  expect(report.errors).toEqual([]);
  expect(report.cut).toBe(true);
  expect(existsSync(root)).toBe(false);
  expect(JSON.parse(readFileSync(join(h.root, 'live.json'), 'utf8'))).toEqual([]);
  expect(kill).toHaveBeenCalledWith(-12345, 'SIGTERM');
  expect(kill.mock.calls.some(([, signal]) => signal === 'SIGKILL')).toBe(false);
  expect(await reapRegistered()).toEqual([]);
});
test('清理扫描失败保留根与登记并使报告失败', async () => {
  const h = harness(); const root = join(h.root, 'owned'); mkdirSync(root);
  register({ root });
  vi.mocked(leftoverProcesses).mockImplementation(() => { throw Error('scan denied'); });
  const { report, outcome } = await h.end();
  expect(outcome?.status).toBe('failed');
  expect(report.errors).toEqual([expect.objectContaining({ message: expect.stringContaining('scan denied') })]);
  expect(existsSync(root)).toBe(true);
  expect(JSON.parse(readFileSync(join(h.root, 'live.json'), 'utf8'))).toEqual([{ root }]);
});
afterEach(() => { vi.unstubAllEnvs(); vi.restoreAllMocks(); for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true }); });
function harness(declared = 1000) {
  const root = mkdtempSync(join(tmpdir(), 'gate-unit-')); roots.push(root);
  vi.stubEnv('CI_E2E_REPORT_DIR', root); vi.stubEnv('CI_E2E_STAGE', '');
  const log = vi.spyOn(console, 'log').mockImplementation(() => {});
  const reporter = new GateReporter({ declaredTimeout: declared });
  reporter.onBegin({ globalTimeout: declared } as FullConfig, { allTests: () => [] } as unknown as Suite);
  return { reporter, root, log, async end(duration = 100, status = 'passed') {
    const outcome = await reporter.onEnd({ duration, status } as FullResult);
    return { outcome, report: JSON.parse(readFileSync(join(root, 'report.json'), 'utf8')) };
  } };
}
test('余量充足通过并打印', async () => { const h = harness(); expect((await h.end()).outcome?.status).toBe('passed'); expect(h.log).toHaveBeenCalledWith(expect.stringContaining('总耗时 0.1 秒，上限 1 秒')); });
test('快检余量不足改判', async () => { const h = harness(); vi.stubEnv('CI_E2E_STAGE', 'quick'); expect((await h.end(900)).outcome?.status).toBe('failed'); expect(h.log).toHaveBeenCalledWith(expect.stringContaining('余量低于 20%')); });
test('合并前门禁只报告且 digest 含警告', async () => { const h = harness(); const r = (await h.end(900)); expect(r.outcome?.status).toBe('passed'); expect(r.report.margin).toEqual({ low: true, enforced: false }); expect(h.log).toHaveBeenCalledWith(expect.stringContaining('合并前门禁口径：只报告，不判失败')); });
test('恰好百分之八十不判', async () => { expect(margin(800, 1000, false, 'quick').low).toBe(false); expect(margin(801, 1000, false, 'quick').enforced).toBe(true); });
test('不限时只打印', async () => { const h = harness(0); expect((await h.end(9000)).report.margin.low).toBe(false); expect(h.log).toHaveBeenCalledWith(expect.stringContaining('上限 0 秒')); });
test('截短上限仍按声明值比较，截断不比较', async () => { const h = harness(); vi.stubEnv('CI_E2E_STAGE', 'quick'); h.reporter.onBegin({ globalTimeout: 200 } as FullConfig, { allTests: () => [] } as unknown as Suite); expect((await h.end(180)).report.margin.low).toBe(false); expect((await h.end(900, 'timedout')).report.margin.low).toBe(false); });
test('用例预算过半', async () => { const h = harness(); h.reporter.onTestEnd({ title: 'case', timeout: 100 } as TestCase, { duration: 51, attachments: [] } as unknown as TestResult); expect((await h.end()).report.overHalf).toContainEqual({ label: 'case（专设）', duration: 51, budget: 100 }); });
test('字面量断言预算过半且相等不报', async () => {
  const h = harness(); const file = join(h.root, 'source.ts'); writeFileSync(file, 'await expect.poll(() => true, { timeout: 60_000 }).toBe(true);');
  const step = { category: 'expect', title: 'poll', duration: 31_000, location: { file, line: 1, column: 7 } } as TestStep;
  expect(literalBudget(step)).toBe(60_000); h.reporter.onStepEnd({} as TestCase, {} as TestResult, step);
  expect((await h.end()).report.overHalf).toHaveLength(1);
});
test('动态预算附件包括失败耗时', async () => {
  const h = harness(); const attachments: { name: string; body: Buffer }[] = [];
  const info = { attach: async (name: string, options: { body: string }) => { attachments.push({ name, body: Buffer.from(options.body) }); } };
  await expect(timedExpect(info as Parameters<typeof timedExpect>[0], 'dynamic', 1, async () => { await new Promise(resolve => setTimeout(resolve, 5)); throw Error('rejected'); })).rejects.toThrow('rejected');
  h.reporter.onTestEnd({ title: 'case', timeout: 100 } as TestCase, { duration: 5, attachments } as unknown as TestResult);
  expect((await h.end()).report.overHalf).toContainEqual(expect.objectContaining({ label: 'dynamic', budget: 1 }));
});
test('spec 守卫：非字面量 timeout 只允许 timedExpect 回调简写', async () => {
  const directory = resolve(import.meta.dirname, '../e2e'); const violations: string[] = [];
  for (const file of readdirSync(directory).filter(file => file.endsWith('.spec.ts'))) {
    const source = ts.createSourceFile(file, readFileSync(join(directory, file), 'utf8'), ts.ScriptTarget.Latest, true);
    function walk(node: ts.Node) {
      if ((ts.isPropertyAssignment(node) || ts.isShorthandPropertyAssignment(node)) && node.name.getText(source) === 'timeout') {
        let callback: ts.Node | undefined = node.parent;
        while (callback && !ts.isArrowFunction(callback)) callback = callback.parent;
        const allowed = ts.isShorthandPropertyAssignment(node) && callback?.parent && ts.isCallExpression(callback.parent) && callback.parent.expression.getText(source) === 'timedExpect';
        if (!allowed && !(ts.isPropertyAssignment(node) && ts.isNumericLiteral(node.initializer))) violations.push(`${file}:${node.getText(source)}`);
      }
      ts.forEachChild(node, walk);
    } walk(source);
  } expect(violations).toEqual([]);
});
test('harness 计时纳入预算过半', async () => { const h = harness(); recordTiming({ label: 'prepare bash', duration: 80, budget: 150 }); recordTiming({ label: 'prepare()', duration: 90 }); const report = (await h.end()).report; expect(report.timings).toHaveLength(2); expect(report.overHalf).toHaveLength(1); });
test('结构化报告保留字段、错误和未执行状态', async () => {
  const h = harness(); h.reporter.onBegin({ globalTimeout: 1000 } as FullConfig, { allTests: () => [{ titlePath: () => ['pending'], location: { file: 'a', line: 1 }, results: [] }] } as unknown as Suite);
  h.reporter.onError({ message: 'setup failed' }); const { report, outcome } = (await h.end(10, 'failed'));
  expect(outcome?.status).toBe('failed'); expect(report).toMatchObject({ complete: false, cut: false, declaredTimeout: 1000, effectiveTimeout: 1000, errors: [{ message: 'setup failed' }], cases: [{ status: 'not_run' }] });
});
