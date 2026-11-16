import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { expect, test } from 'vitest';
import { leftoverProcesses } from './fixtures/process-scan.ts';
const web = resolve(import.meta.dirname, '..');
const harness = join(web, 'e2e/harness');
function run(body: string, limit = 10_000, quick = false, service = false) {
  const output = resolve(web, '../../.autome/output');
  mkdirSync(output, { recursive: true });
  const root = mkdtempSync(join(output, 'pw-gate-')); 
  const reportDir = join(root, 'report');
  try {
    writeFileSync(join(root, 'package.json'), JSON.stringify({ type: 'module' }));
    if (service) leftoverProcesses(root); // Refuse to start a service when cleanup cannot inspect it.
    writeFileSync(join(root, 'case.spec.ts'), `import { test, expect } from ${JSON.stringify(join(web, 'node_modules/@playwright/test/index.mjs'))};\ntest('fixture', async () => { ${body} });`);
    writeFileSync(join(root, 'setup.ts'), `import { recordTiming } from ${JSON.stringify(join(harness, 'budget.ts'))};\n${service ? `import { spawn } from 'node:child_process'; import { mkdirSync } from 'node:fs'; import { register } from ${JSON.stringify(join(harness, 'reap.ts'))};` : ''}\nexport default () => { recordTiming({label:'setup',duration:1}); ${service ? `const root = ${JSON.stringify(join(root, 'owned'))}; mkdirSync(root); register({root}); const child = spawn(process.execPath, ['-e','setInterval(()=>{},1000)', root], {detached:true,stdio:'ignore'}); register({root,pid:child.pid,group:child.pid}); child.unref();` : ''} };`);
    writeFileSync(join(root, 'teardown.ts'), `import { recordTiming } from ${JSON.stringify(join(harness, 'budget.ts'))}; export default () => recordTiming({label:'teardown',duration:1});`);
    writeFileSync(join(root, 'playwright.config.ts'), `export default {testDir:'.', outputDir:${JSON.stringify(join(root, 'results'))}, workers:1, globalTimeout:${limit}, globalSetup:'./setup.ts',globalTeardown:'./teardown.ts',reporter:[[${JSON.stringify(join(harness, 'gate-reporter.ts'))},{declaredTimeout:${limit}}]]};`);
    const result = spawnSync(process.execPath, [join(web, 'node_modules/@playwright/test/cli.js'), 'test', '-c', join(root, 'playwright.config.ts')], { cwd: web, encoding: 'utf8', timeout: 30_000, env: { ...process.env, CI_E2E_REPORT_DIR: reportDir, CI_E2E_STAGE: quick ? 'quick' : '', NO_COLOR: '1', FORCE_COLOR: '0' } });
    expect(result.error).toBeUndefined();
    const output = result.stdout + result.stderr;
    expect(existsSync(join(reportDir, 'report.json')), output).toBe(true);
    const report = JSON.parse(readFileSync(join(reportDir, 'report.json'), 'utf8'));
    if (process.env.UPDATE_PW_GATE_FIXTURE === '1' && report.margin.low && !report.margin.enforced && result.status === 0) {
      const destination = join(web, 'test/fixtures/pw-gate'); mkdirSync(destination, { recursive: true });
      writeFileSync(join(destination, 'report-margin-low.json'), JSON.stringify(report, null, 2) + '\n');
    }
    expect(leftoverProcesses(root)).toEqual([]);
    if (service) expect(existsSync(join(root, 'owned'))).toBe(false);
    return { status: result.status, report, output };
  } finally { rmSync(root, { recursive: true, force: true }); }
}
test('真实运行包含 setup/teardown', () => { const r = run('expect(true).toBe(true)'); expect(r.status, r.output).toBe(0); expect(r.report.timings.map((r: { label: string }) => r.label)).toEqual(['setup', 'teardown']); }, 40_000);
test('快检过余量线改写真实退出码', () => { const r = run('await new Promise(r=>setTimeout(r,3500))', 4000, true); expect(r.status, r.output).toBe(1); expect(r.report.margin.enforced).toBe(true); }, 40_000);
test('合并前门禁过余量线退出零并保留真实样本', () => {
  const r = run('await new Promise(r=>setTimeout(r,3500))', 4000); expect(r.status, r.output).toBe(0); expect(r.report.margin).toEqual({ low: true, enforced: false }); expect(r.output).toContain('余量低于 20%');
  const destination = join(web, 'test/fixtures/pw-gate'); mkdirSync(destination, { recursive: true });
  // Refresh only explicitly; normal test runs never mutate the tracked sample.
  if (process.env.UPDATE_PW_GATE_FIXTURE === '1') writeFileSync(join(destination, 'report-margin-low.json'), JSON.stringify(r.report, null, 2) + '\n');
}, 40_000);
test('真实 poll 两秒预算一点五秒过半', () => { const r = run('const start=Date.now(); await expect.poll(()=>Date.now()-start,{ timeout: 2000, intervals:[50] }).toBeGreaterThan(1500)'); expect(r.status, r.output).toBe(0); expect(r.report.overHalf).toContainEqual(expect.objectContaining({ budget: 2000 })); }, 40_000);
test('预算截断后回收登记的服务及临时根', () => { const r = run('await new Promise(r=>setTimeout(r,10000))', 2000, false, true); expect(r.status).toBe(1); expect(r.report.cut).toBe(true); expect(r.report.errors).not.toEqual(expect.arrayContaining([expect.objectContaining({ message: expect.stringContaining('清理失败') })])); }, 40_000);
