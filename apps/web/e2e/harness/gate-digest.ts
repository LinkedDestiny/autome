import { spawnSync } from 'node:child_process';
import type { TestError } from '@playwright/test/reporter';
import type { Budget } from './budget.ts';
export type Case = { title: string; location: { file: string; line: number; column?: number }; status: string; duration: number; errors: TestError[] };
export function clip(text: string, bytes: number) {
  if (Buffer.byteLength(text) <= bytes) return text;
  let result = '';
  for (const char of text) {
    if (Buffer.byteLength(result + char + '…') > bytes) break;
    result += char;
  }
  return result + '…';
}
export function listCases(): Case[] {
  // Preserve selection/configuration flags; replace only reporters and list mode.
  const args = process.argv.slice(2).filter((arg, i, all) => arg !== '--list' && arg !== '--reporter' && all[i - 1] !== '--reporter' && !arg.startsWith('--reporter='));
  const child = spawnSync(process.execPath, [process.argv[1], ...args, '--list', '--reporter=json'], {
    encoding: 'utf8', timeout: 30_000, maxBuffer: 16 * 1024 * 1024,
    env: { ...process.env, NO_COLOR: '1', FORCE_COLOR: '0' },
  });
  if (child.error || child.status !== 0) throw Error(child.error?.message || child.stderr || `--list exit ${child.status}`);
  const report = JSON.parse(child.stdout);
  if (report.errors?.length) throw Error(JSON.stringify(report.errors));
  const cases: Case[] = [];
  type ListedSuite = { title: string; suites?: ListedSuite[]; specs?: { title: string; file: string; line: number; column: number; tests: { projectName?: string }[] }[] };
  function visit(suite: ListedSuite, parents: string[]) {
    const path = [...parents, suite.title];
    for (const spec of suite.specs || []) for (const test of spec.tests) cases.push({ title: [...path, test.projectName, spec.title].filter(Boolean).join(' › '), location: { file: spec.file, line: spec.line, column: spec.column }, status: 'not_run', duration: 0, errors: [] });
    for (const child of suite.suites || []) visit(child, path);
  }
  for (const suite of report.suites || []) visit(suite, []);
  return cases;
}
export type DigestReport = { cases: Case[]; errors: TestError[]; timings: Budget[]; overHalf: Budget[]; summary: string; warnings: string[]; effectiveTimeout?: number; declaredTimeout: number };
export function cutReason(status: string, pending: boolean, setup: boolean, maxFailures: number, effective: number, budget: boolean) {
  if (!pending) return '';
  if (status === 'timedout') return budget ? `被浏览器预算截断（${effective / 1000} 秒），未执行的用例留给合并前门禁` : `整套被总超时截断（globalTimeout ${effective / 1000} 秒，${maxFailures ? `maxFailures ${maxFailures}` : '未设 maxFailures'}）`;
  if (setup && status === 'failed') return 'globalSetup 失败，整套未执行';
  return '运行中止，原因见非用例错误';
}
export function renderDigest(report: DigestReport, path: string, reason: string) {
  const render = (detail: number, errorLimit: number) => {
    const location = (value: Case['location']) => `${value.file}:${value.line}${value.column ? `:${value.column}` : ''}`;
    const error = (value: TestError, limit: number) => `${value.location ? location(value.location) + '\n' : ''}${clip(value.stack || value.message || value.value || '', limit)}`;
    return [
      `完整原文（未截）：${path}`, report.summary,
      ...(report.effectiveTimeout !== undefined && report.effectiveTimeout !== report.declaredTimeout ? [`生效上限 ${report.effectiveTimeout / 1000} 秒`] : []),
      ...report.warnings, reason,
      '失败用例：', ...report.cases.filter(c => ['failed', 'timedOut'].includes(c.status)).map(c => `${location(c.location)} ${c.title}\n${c.errors.map(e => error(e, detail)).join('\n')}`),
      '未执行用例：', ...report.cases.filter(c => ['not_run', 'interrupted'].includes(c.status)).map(c => `${location(c.location)} ${c.title}${c.status === 'interrupted' ? `（被中断，已运行 ${c.duration / 1000} 秒）` : ''}`),
      '非用例错误：', ...report.errors.map(e => error(e, errorLimit)),
      '各用例耗时：', ...report.cases.map(c => `${location(c.location)} ${c.title}: ${c.duration} ms`),
      '准备与收尾耗时：', ...report.timings.map(r => `${r.label}: ${r.duration} ms`),
      '预算过半：', ...report.overHalf.map(r => `${r.label}: ${r.duration} ms / 预算 ${r.budget} ms（过半）`),
    ].filter(Boolean).join('\n');
  };
  let text = render(1024, 2048);
  for (const limit of [768, 512, 300]) {
    if (Buffer.byteLength(text) <= 12 * 1024) break;
    text = render(limit, limit);
  }
  return `autome-gate-digest begin Web e2e\n${text}\nautome-gate-digest end`;
}
