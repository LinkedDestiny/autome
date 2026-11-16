import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { type Case, cutReason, listCases, renderDigest } from './gate-digest.ts';
import ts from 'typescript';
import type { FullConfig, FullResult, Reporter, Suite, TestCase, TestError, TestResult, TestStep } from '@playwright/test/reporter';
import { DECLARED_GLOBAL_TIMEOUT_MS } from './limits.ts';
import { type Budget, reportDirectory } from './budget.ts';
import { reapRegistered } from './reap.ts';
export function margin(duration: number, declared: number, cut: boolean, stage?: string) {
  const low = !cut && declared > 0 && duration > declared * 0.8;
  return { low, enforced: low && stage === 'quick' };
}
export function literalBudget(step: Pick<TestStep, 'location'>) {
  if (!step.location) return undefined;
  const source = ts.createSourceFile(step.location.file, readFileSync(step.location.file, 'utf8'), ts.ScriptTarget.Latest, true);
  const position = source.getPositionOfLineAndCharacter(step.location.line - 1, step.location.column - 1);
  let assertion: ts.CallExpression | undefined;
  function find(node: ts.Node) {
    if (node.getStart(source) > position || node.getEnd() < position) return;
    if (ts.isCallExpression(node) && /^expect(?:\.|\()/.test(node.getText(source))) assertion ??= node;
    ts.forEachChild(node, find);
  }
  find(source);
  let budget: number | undefined;
  function read(node: ts.Node) {
    if (ts.isPropertyAssignment(node) && node.name.getText(source) === 'timeout' && ts.isNumericLiteral(node.initializer)) budget = Number(node.initializer.text);
    ts.forEachChild(node, read);
  }
  if (assertion) read(assertion);
  return budget;
}
export default class GateReporter implements Reporter {
  private suite?: Suite;
  private config?: FullConfig;
  private errors: TestError[] = [];
  private budgets: Budget[] = [];
  private started = new Map<string, number>();
  private options: { declaredTimeout?: number };
  constructor(options: { declaredTimeout?: number } = {}) { this.options = options; }
  onBegin(config: FullConfig, suite: Suite) { this.config = config; this.suite = suite; }
  onError(error: TestError) { this.errors.push(error); }
  onStepEnd(_test: TestCase, _result: TestResult, step: TestStep) {
    if (step.category !== 'expect') return;
    try {
      const budget = literalBudget(step);
      if (budget !== undefined) this.budgets.push({ label: `${step.location?.file}:${step.location?.line} ${step.title}`, duration: step.duration, budget });
    } catch (error) { this.errors.push({ message: `断言预算读取失败: ${String(error)}` }); }
  }
  onTestBegin(test: TestCase) { this.started.set(test.id, Date.now()); }
  onTestEnd(test: TestCase, result: TestResult) {
    this.budgets.push({ label: `${test.title}（${test.timeout === 90_000 ? '缺省' : '专设'}）`, duration: result.duration, budget: test.timeout });
    for (const attachment of result.attachments.filter(item => item.name === 'gate-budget')) {
      try { this.budgets.push(JSON.parse((attachment.body || readFileSync(attachment.path!)).toString())); }
      catch (error) { this.errors.push({ message: `预算附件读取失败: ${String(error)}` }); }
    }
  }
  async onEnd(result: FullResult) {
    if (process.argv.includes('--list')) return;
    for (const message of await reapRegistered()) this.errors.push({ message });
    const directory = reportDirectory();
    mkdirSync(directory, { recursive: true });
    const timingFile = join(directory, 'harness.jsonl');
    const timings: Budget[] = existsSync(timingFile) ? readFileSync(timingFile, 'utf8').trim().split('\n').filter(Boolean).map(line => JSON.parse(line)) : [];
    let cases: Case[] = (this.suite?.allTests() || []).map(test => {
      const last = test.results.at(-1);
      // The runner can synthesize skipped results when a deadline expires before
      // the replacement worker starts. These are not intentional test skips.
      const stopped = ['timedout', 'interrupted'].includes(result.status) && last?.status === 'skipped' && test.expectedStatus !== 'skipped';
      const start = this.started.get(test.id);
      return { title: test.titlePath().join(' › '), location: test.location, status: stopped ? (start === undefined ? 'not_run' : 'interrupted') : last?.status || 'not_run', duration: stopped && start !== undefined ? Date.now() - start : last?.duration || 0, errors: last?.errors || [] };
    });
    const setup = !this.suite || !cases.length;
    if (setup && result.status !== 'passed') {
      try { cases = listCases(); }
      catch (error) { this.errors.push({ message: '未能取得用例清单：' + String(error) }); }
    }
    const cut = result.status === 'timedout' || result.status === 'interrupted';
    const complete = !cut && !!this.suite && cases.every(test => test.status !== 'not_run' && test.status !== 'interrupted');
    const declared = this.options.declaredTimeout ?? DECLARED_GLOBAL_TIMEOUT_MS;
    const remaining = margin(result.duration, declared, !complete, process.env.CI_E2E_STAGE);
    const summary = `总耗时 ${result.duration / 1000} 秒，上限 ${declared / 1000} 秒（余量 ${declared ? ((1 - result.duration / declared) * 100).toFixed(1) : '不限'}%）`;
    const warnings = remaining.low ? [`浏览器用例${summary}，余量低于 20%${remaining.enforced ? '' : '；合并前门禁口径：只报告，不判失败'}`] : [];
    const status = remaining.enforced || this.errors.length ? 'failed' as const : result.status;
    const report = { status, duration: result.duration, declaredTimeout: declared, effectiveTimeout: this.config?.globalTimeout, complete, cut, margin: remaining, cases, errors: this.errors, timings, overHalf: [...this.budgets, ...timings].filter(row => row.budget && row.duration > row.budget / 2), summary, warnings };
    const original = process.env.AUTOME_GATE_LOG ? process.env.AUTOME_GATE_LOG + '.playwright.json' : join(directory, 'report.json');
    writeFileSync(original, JSON.stringify(report, null, 2));
    writeFileSync(join(directory, 'report.json'), JSON.stringify(report, null, 2));
    console.log(renderDigest(report, resolve(original), cutReason(result.status, cases.some(c => ['not_run', 'interrupted'].includes(c.status)), setup, this.config?.maxFailures || 0, this.config?.globalTimeout ?? declared, !!process.env.CI_E2E_DEADLINE)));
    return { status };
  }
}
