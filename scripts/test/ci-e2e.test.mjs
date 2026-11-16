import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, dirname } from 'node:path';
const source = resolve(import.meta.dirname, '../..');
const clean = { ...process.env, STATIC_FAIL: "0", CORE_FAIL: "0", NO_COLOR: "1", FORCE_COLOR: "0" };
for (const key of Object.keys(clean)) if (/^(AUTOME_|CI_|GIT_)/.test(key)) delete clean[key];
function fixture(t) {
  const dir = mkdtempSync(resolve(tmpdir(), 'ci-e2e-')); const root = resolve(dir, 'repo'); mkdirSync(root);
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  cpSync(resolve(source, 'scripts'), resolve(root, 'scripts'), { recursive: true });
  const put = (p, s) => { mkdirSync(dirname(resolve(root, p)), { recursive: true }); writeFileSync(resolve(root, p), s); };
  put('.gitignore', '.autome/\nnode_modules/\n');
  put('apps/web/e2e/harness/reap.ts', `import { appendFileSync } from 'node:fs';\n// test/fixtures/shared.ts\nexport async function reapRegistered() { appendFileSync(process.env.TRACE, 'reap\\n'); return []; }`);
  for (const p of ['e2e/a.spec.ts', 'e2e/b.spec.ts', 'test/fixtures/shared.ts', 'playwright.config.ts', 'app/page.ts', 'package.json', 'package-lock.json']) put(`apps/web/${p}`, '// baseline');
  put('apps/web/package.json', '{"type":"module"}');
  put('apps/web/scripts/quick-related.mjs', `import { writeFileSync } from 'node:fs'; writeFileSync(process.argv[2], "ci_partial related true\\n");`);
  put('scripts/ci/steps.sh', `GLOBAL_INPUTS='scripts'\nNO_GATE='docs .autome .gitignore'\nstep 'Format' 'crates' sh -c 'exit $STATIC_FAIL'\nstep 'Clippy' 'crates' true\nprep 'Web install' true\nprep 'Build core' sh -c 'exit $CORE_FAIL'\nstep 'Web lint' 'apps/web' true\nstep 'Web typecheck' 'apps/web' true\nstep 'Web build' 'apps/web' npm run build --prefix apps/web\nstep 'Web e2e' 'apps/web' npm run e2e --prefix apps/web\n`);
  const bin = resolve(dir, 'bin'); mkdirSync(bin);
  const npm = execFileSync('sh', ['-c', 'command -v npm'], { encoding: 'utf8' }).trim();
  writeFileSync(resolve(bin, 'npm'), `#!/bin/sh\nif [ "$1" = -v ]; then exec '${npm}' -v; fi\nexec node '${dir}/fake.mjs' "$@"\n`, { mode: 0o755 });
  writeFileSync(resolve(dir, 'sample.json'), readFileSync(resolve(source, 'apps/web/test/fixtures/pw-gate/report-margin-low.json')));
  writeFileSync(resolve(dir, 'fake.mjs'), `import fs from 'node:fs';
const mode = process.env.RESULT || 'pass';
fs.appendFileSync(process.env.TRACE, JSON.stringify({ args: process.argv.slice(2), stage: process.env.CI_E2E_STAGE, deadline: process.env.CI_E2E_DEADLINE }) + '\\n');
if (process.argv[3] === 'build') process.exit(Number(process.env.BUILD_FAIL || 0));
const r = JSON.parse(fs.readFileSync('${dir}/sample.json'));
if (mode !== 'margin') { r.margin = {low:false,enforced:false}; r.warnings=[]; }
if (mode === 'margin' && process.env.CI_E2E_STAGE === 'quick') {r.margin.enforced=true; r.status='failed';}
if (mode.startsWith('cut')) {r.complete=false;r.cut=true;r.status='timedout';r.effectiveTimeout=1000;r.cases.push({status:'not_run'});r.errors=[{message:'Timed out waiting 1s for the test suite to run'}];r.status='failed';}
if (mode === 'cut-fail' || mode === 'fail') {r.cases[0].status='failed';r.status='failed';}
if (mode === 'missing') process.exit(1);
fs.mkdirSync('.autome/output/playwright-gate',{recursive:true});
fs.writeFileSync('.autome/output/playwright-gate/report.json', mode === 'bad' ? '{' : JSON.stringify(r));
console.log(r.warnings.join('\\n'));
process.exit(mode === 'cut-zero' || r.status === 'passed' ? 0 : 1);
`);
  const git = args => execFileSync('git', args, { cwd: root, encoding: 'utf8' }).trim();
  git(['init', '-q']); git(['add', '-A']);git(['-c','core.hooksPath=/dev/null','-c','user.name=T','-c','user.email=t@t','commit','-qm','base']);
  const base = git(['rev-parse','HEAD']); const report = resolve(dir,'report'); const trace = resolve(dir,'trace');
  const run = (args = [], env = {}, full = false) => spawnSync('sh', [full ? 'scripts/ci-local.sh' : 'scripts/ci-quick.sh', ...args], {cwd:root, env:{...clean,PATH:`${bin}:${clean.PATH}`,AUTOME_ROUND_START:base,AUTOME_GATE_REPORT:report,TRACE:trace,...env},encoding:'utf8',timeout:60000});
  return {root, dir, put, run, report:()=>readFileSync(report,'utf8'), trace:()=>readFileSync(trace,'utf8'), change:p=>put(p,'// changed')};
}
const ok = r => assert.equal(r.status,0,r.stdout+r.stderr);
const e2e = r => r.stdout.split('\n').find(l=>l.startsWith('e2e\t'));
const fullChange = f => f.change('apps/web/playwright.config.ts');

test('01 changed spec selects only that file and records partial', t=>{const f=fixture(t);f.change('apps/web/e2e/a.spec.ts');assert.match(e2e(f.run(['--plan'])),/子集 e2e\/a.spec.ts/);ok(f.run());assert.match(f.report(),/partial\t-\tWeb e2e/);assert.match(f.trace(),/e2e\/a.spec.ts/);});
test('02 harness, config and referenced fixture select whole suite with full-gate key',t=>{for(const p of ['apps/web/e2e/harness/new.ts','apps/web/playwright.config.ts','apps/web/test/fixtures/shared.ts']){const f=fixture(t);f.change(p);const q=f.run(['--plan']);ok(q);assert.match(e2e(q),/整套/);const full=f.run(['--plan'],{},true);ok(full);assert.equal(q.stdout.split('\n').find(l=>l.endsWith('\tWeb e2e')),full.stdout.split('\n').find(l=>l.endsWith('\tWeb e2e')));}});
test('03 app change uses specs named in current evidence milestone, union with changed spec',t=>{const f=fixture(t);f.change('apps/web/app/page.ts');f.change('apps/web/e2e/a.spec.ts');f.put('docs/task/evidence/M-02-r3-impl.md','evidence');f.put('docs/task/task.md','### M-02 验收契约\n- 验收命令：npx playwright test e2e/b.spec.ts\n### M-03 验收契约\n');assert.match(e2e(f.run(['--plan'])),/子集 e2e\/a.spec.ts e2e\/b.spec.ts/);ok(f.run());});
test('04 app without evidence or named spec falls back to whole suite',t=>{const f=fixture(t);f.change('apps/web/app/page.ts');assert.match(e2e(f.run(['--plan'])),/整套/);f.put('docs/task/evidence/M-02-r3-impl.md','evidence');f.put('docs/task/task.md','### M-02 验收契约\n- 验收命令：true');assert.match(e2e(f.run(['--plan'])),/整套/);});
test('05 non-Web changes do not run browser; absent baseline is explicit',t=>{const f=fixture(t);f.put('docs/a.md','doc');f.put('scripts/new.sh','true');f.put('crates/README.md','crate');assert.match(e2e(f.run(['--plan'])),/不跑/);ok(f.run());assert.doesNotMatch(f.report(),/Web e2e/);assert.match(e2e(f.run(['--plan'],{AUTOME_ROUND_START:''})),/未提供本轮起点/);});
test('06 zero quick allowance does not consume independent browser allowance',t=>{const f=fixture(t);fullChange(f);ok(f.run([],{CI_QUICK_BUDGET:'0'}));assert.match(f.report(),/not_run/);assert.match(f.report(),/ran\t\w+\tWeb e2e/);});
test('07 insufficient deadline and numeric boundaries never launch browser',t=>{const f=fixture(t);fullChange(f);ok(f.run([],{AUTOME_GATE_DEADLINE:String(Math.floor(Date.now()/1000)+299)}));assert.match(f.report(),/not_run.*剩余 \d+ 秒/);for(const v of ['0','000','239']){ok(f.run([],{CI_E2E_BUDGET:v}));assert.match(f.report(),/not_run.*Web e2e/);}for(const v of ['','-1','1.5','no','9007199254740992'])assert.equal(f.run(['--plan'],{CI_E2E_BUDGET:v}).status,2);ok(f.run(['--plan'],{CI_E2E_BUDGET:'9007199254740'}));writeFileSync(resolve(f.dir,'bin/date'),'#!/bin/sh\necho 1700000000\n',{mode:0o755});ok(f.run([],{CI_E2E_BUDGET:'240'}));assert.match(f.report(),/ran\t\w+\tWeb e2e/);});
test('08 cut without failures is not_run; prior failure still rejects; absent/bad reports fail closed',t=>{const f=fixture(t);fullChange(f);for(const RESULT of ['cut','cut-zero']){ok(f.run([],{RESULT}));assert.match(f.report(),/not_run.*Web e2e/);assert.doesNotMatch(f.report(),/ran\t\w+\tWeb e2e/);}for(const RESULT of ['cut-fail','fail','missing','bad']){assert.equal(f.run([],{RESULT}).status,1);assert.match(f.report(),/failed\t\w+\tWeb e2e/);}assert.match(f.trace(),/reap/);});
test('09 complete pass publishes reusable key, including merge-gate low-margin acceptance',t=>{const f=fixture(t);fullChange(f);for(const RESULT of ['pass','margin']){ok(f.run([], {RESULT}, RESULT==='margin'));const key=/ran\t(\w+)\tWeb e2e/.exec(f.report())[1];const passes=resolve(f.dir,'passes');writeFileSync(passes,`${key}\tT\tmerge_gate\thead\tnow\n`);const p=f.run(['--plan'],{AUTOME_GATE_PASSES:passes},true);ok(p);assert.match(p.stdout,/plan\t沿用\t\w+\tWeb e2e/);writeFileSync(resolve(f.dir,'trace'),'');ok(f.run([],{AUTOME_GATE_PASSES:passes}));assert.match(f.report(),/reused\t\w+\tWeb e2e/);assert.doesNotMatch(f.trace(),/"e2e"/);}});
test('10 subset pass never publishes whole-suite key',t=>{const f=fixture(t);f.change('apps/web/e2e/b.spec.ts');ok(f.run());assert.match(f.report(),/partial\t-\tWeb e2e/);assert.doesNotMatch(f.report(),/ran\t\w+\tWeb e2e/);});
test('11 earlier quick failure still runs browser; browser prerequisite failures omit it',t=>{const f=fixture(t);fullChange(f);assert.equal(f.run([],{STATIC_FAIL:'1'}).status,1);assert.match(f.report(),/ran\t\w+\tWeb e2e/);for(const key of ['BUILD_FAIL','CORE_FAIL']){assert.equal(f.run([],{[key]:'1'}).status,1);assert.match(f.report(),/not_run.*Web e2e/);}});
test('12 quick exports stage and deadline; enforced low margin rejects',t=>{const f=fixture(t);fullChange(f);assert.equal(f.run([],{RESULT:'margin'}).status,1);assert.match(f.trace(),/"stage":"quick","deadline":"\d+"/);assert.match(f.report(),/failed\t\w+\tWeb e2e/);});
test('13 full gate clears inherited quick stage/deadline and retains low-margin warning',t=>{const f=fixture(t);fullChange(f);const r=f.run([],{RESULT:'margin',CI_E2E_STAGE:'quick',CI_E2E_DEADLINE:'1'},true);ok(r);assert.doesNotMatch(f.trace(),/"stage"|"deadline"/);assert.match(r.stdout,/余量低于 20%/);assert.match(f.report(),/ran\t\w+\tWeb e2e/);const report=JSON.parse(readFileSync(resolve(f.root,'.autome/output/playwright-gate/report.json')));assert.deepEqual(report.margin,{low:true,enforced:false});});
