import { readFileSync, existsSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
const path = resolve('.autome/output/playwright-gate/report.json');
try {
  const { reapRegistered } = await import(pathToFileURL(resolve('apps/web/e2e/harness/reap.ts')));
  const errors = await reapRegistered();
  if (errors.length) throw Error(errors.join('\n'));
  const report = JSON.parse(readFileSync(path, 'utf8'));
  const failures = report.cases.filter(c => ['failed', 'timedOut'].includes(c.status));
  if (failures.length || report.margin?.enforced) process.exit(1);
  if (report.cut && report.effectiveTimeout < report.declaredTimeout
      && report.errors.every(error => /Timed out waiting [0-9.]+s for the .+ to run/.test(error.message ?? ''))) {
    console.log(`浏览器用例被预算截断（已跑 ${report.cases.filter(c => !['not_run', 'interrupted'].includes(c.status)).length} / 共 ${report.cases.length} 条），其余留给合并前门禁`);
    process.exit(3);
  }
  if (!report.complete || report.cut || report.status !== 'passed' || process.argv[2] !== '0') process.exit(1);
} catch (error) {
  console.error(`浏览器报告或清理失败：${error.message}`);
  if (existsSync(path)) {
    try {
      const report = JSON.parse(readFileSync(path, 'utf8'));
      report.errors ??= []; report.errors.push({ message: String(error) }); report.status = 'failed';
      writeFileSync(path, JSON.stringify(report, null, 2));
    } catch { /* Keep the original unreadable report for diagnosis. */ }
  }
  process.exitCode = 1;
}
