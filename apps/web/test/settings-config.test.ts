import capturedConfig from './fixtures/m02-config-get.json' with { type: 'json' };
import populatedConfig from './fixtures/m05-config-resolved.json' with { type: 'json' };
import { CONFIG_WARNINGS, configWarning } from '../app/lib/config-warnings.ts';
import { CONFIG_LABELS } from '../app/lib/admin-labels.ts';
import { roleLabel } from '../app/lib/labels.ts';
import { createElement } from 'react';
import { renderToString } from 'react-dom/server';
import { createMemoryRouter, RouterProvider, RouterContextProvider } from 'react-router';
import { expect, test, vi } from 'vitest';
import { readFileSync, readdirSync, existsSync } from 'node:fs';
import { action, loader, SettingsView } from '../app/routes/projects.$id.settings.tsx';
import { servicesContext, type WebServices } from '../app/load-context.ts';
import type { SettingsData } from '../app/components/settings-data.ts';
import { configRows } from '../app/components/config-view.ts';
import { WriteGateProvider } from '../app/lib/live.tsx';
import { createAdmin } from '../server/admin.ts';
import fixture from './fixtures/w9-replies.json' with { type: 'json' };
export const base: SettingsData = { project: { id: 'p', path: '/tmp/p', display_name: '测试', onboarding: { onboarding: 'in_progress', step: 3 } }, files: [], session: { state: 'none' } };
export function args(services: WebServices, form: Record<string, string> = {}) { const context = new RouterContextProvider(); context.set(servicesContext, services); return { context, params: { id: 'p' }, request: new Request('http://localhost/projects/p/settings', { method: 'POST', body: new URLSearchParams(form) }), url: new URL('http://localhost/projects/p/settings'), pattern: '/projects/:id/settings' }; }
export function html(data: SettingsData, allowed = true) { const router = createMemoryRouter([{ path: '/', element: createElement(WriteGateProvider, { value: { allowed, reason: '离线' } }, createElement(SettingsView, { data })) }]); try { return renderToString(createElement(RouterProvider, { router })); } finally { router.dispose(); } }
const ok = (payload: unknown) => ({ status: 200, body: { ok: true, payload } });
test('1 真实配置逐行、null、未知键和省略字段', () => {
  const resolved = fixture.config.resolved;
  const rows = configRows(resolved);
  const expected: Record<string, string> = {};
  function leaves(value: unknown, key = '') { if (key === 'roles') return; if (value !== null && typeof value === 'object') Object.entries(value).forEach(([k,v]) => leaves(v, key ? key + '.' + k : k)); else expected[key] = value === null ? '未配置' : String(value); }
  leaves(resolved);
  expect(rows.filter(r => !r.key.startsWith('roles.'))).toHaveLength(Object.keys(expected).length);
  for (const [key,value] of Object.entries(expected)) expect(rows.filter(r => r.key === key).map(r => r.value)).toEqual([value]);
  resolved.roles.forEach((r,i) => { const row = rows.find(row => row.key === `roles.${i}`)!; expect(row.label).toBe('角色 ' + roleLabel(r.role) + '（' + r.role + '）'); expect(row.value).toContain(r.config.runtime); expect(row.value).toContain(r.config.effort ?? '未配置'); expect(row.value).toContain('已启用'); });
  expect(rows).toHaveLength(Object.keys(expected).length + resolved.roles.length);
  const extra = configRows({ ...resolved, x_new: 1, toString: 0 }); expect(extra).toContainEqual({ key: 'x_new', label: 'x_new', value: '1' }); expect(extra).toContainEqual({ key: 'toString', label: 'toString', value: '0' });
  for (const key of ['gate_run','gate_quick','setup']) {
    expect(rows.some(r => r.key === key)).toBe(true);
    expect(configRows({ gate: null }).some(r => r.key === key)).toBe(false);
  }
  expect(configRows({ empty: '', zero: 0, flag: false })).toEqual([{ key:'empty',label:'empty',value:'' },{key:'zero',label:'zero',value:'0'},{key:'flag',label:'flag',value:'false'}]);
});
test('2 validate 警告中文逐条显示', async () => {
  const warnings = [{ kind: 'session_minutes_out_of_range', min: 5, value: 1 }, { kind: 'future', detail: '原文' }];
  const read = vi.fn(async ({ method }: {method:string}) => ok(method === 'project.list' ? { projects: [{project:base.project}] } : method === 'project.get' ? {} : method === 'config.get' ? fixture.config : method === 'config.validate' ? { violations: warnings } : {files:[]}));
  const data = await loader(args({gateway:{read,write:vi.fn()}})); expect(data.config?.warnings).toEqual(warnings); const text = html(data); for (const word of ['会话时长 1 分钟低于下限 5 分钟','future','detail=原文']) expect(text).toContain(word);
});
test('3 配置路径使用 AUTOME_HOME 并仅缩写家目录内路径', () => {
  const admin = createAdmin({core:{call:vi.fn()},userHome:'/tmp/home',automeHome:'/tmp/home/custom'});
  expect(admin.configPaths('/tmp/home/p')).toEqual(['~/custom/config.toml','~/p/.autome/config.toml']);
  expect(admin.configPaths('/tmp/home-other/p')[1]).toBe('/tmp/home-other/p/.autome/config.toml');
  expect(createAdmin({core:{call:vi.fn()},userHome:'/tmp/home',automeHome:'/tmp/external'}).configPaths('/tmp/home')[0]).toBe('/tmp/external/config.toml');
});
test('4 配置失败独立显示，其他卡照常', async () => {
  for (const error of ['TOML parse error 原文','拒绝授权','读盘失败','请求超时','内核未连接']) {
    const read = vi.fn(async ({method}:{method:string}) => method === 'config.get' ? {status:409,body:{ok:false,error}} : ok(method === 'project.list' ? {projects:[{project:base.project}]} : {files:[]}));
    const data = await loader(args({gateway:{read,write:vi.fn()}})); expect(data.config?.error).toBe(error); const text=html(data); for(const word of [error,'Onboarding','移除项目']) expect(text).toContain(word); expect(text).not.toContain('data-config-key'); expect(read.mock.calls.some(([call]) => call.method === 'config.validate')).toBe(false);
  }
});
test('5 名字严格比较，错误不写，正确才写；离线禁用', async () => {
  const read=vi.fn(async()=>ok({projects:[{project:base.project}]})), write=vi.fn(async()=>ok({removed:'p'}));
  for(const name of ['', ' test', '测试 ', '测', 'TEST']) expect(await action(args({gateway:{read,write}}, {intent:'remove',name}))).toHaveProperty('error','已拒绝：输入的项目名不一致');
  expect(write).not.toHaveBeenCalled(); expect(html(base)).toMatch(/<button disabled=""[^>]*>移除项目/);
  const result=await action(args({gateway:{read,write}}, {intent:'remove',name:'测试'})); expect(result).toBeInstanceOf(Response); expect((result as Response).headers.get('Location')).toBe('/'); expect(write).toHaveBeenCalledWith({op:'project.remove',params:{project_id:'p'}});
  const offline=html(base,false); expect(offline.match(/<button/g)?.length).toBe(offline.match(/<button[^>]*disabled/g)?.length);
});
test('6 移除说明和目录明确且配置只读', () => { const text=html({...base,config:{rows:configRows(fixture.config.resolved),paths:[],warnings:[]}}); for(const word of ['只是取消注册','不会删除任何文件','/tmp/p']) expect(text).toContain(word); const card=text.split('配置（只读）')[1].split('移除项目')[0]; expect(card).not.toMatch(/<(input|textarea|button)/); });
test('7 新增源码没有删除文件调用或导入', () => {
  const paths=['server/paths.ts','server/admin.ts','server/onboarding-disk.ts','server/terminal.ts',...['app/routes','app/components'].flatMap(dir=>readdirSync(dir).filter(f=>/^(projects\.(new|\$id\.settings)|settings-|config-view|add-project|path-complete|admin-live|sidebar-footer|alerts|env)/.test(f)).map(f=>dir+'/'+f))];
  for(const path of paths.filter(existsSync)) {const source=readFileSync(path,'utf8'); expect(source,path).not.toMatch(/\b(rmSync|unlinkSync|rmdirSync)\b|\bfs(\.promises)?\.(rm|unlink|rmdir)\(/); for(const match of source.matchAll(/import\s*{([^}]+)}\s*from\s*['"]node:fs(?:\/promises)?['"]/g)) expect(match[1],path).not.toMatch(/\b(rm|unlink|rmdir)\b/); }
});
test('8 八种配置警告中文包含各自字段', () => {
  const cases = [
    [{kind:'same_model',evaluator:'review',generator:'plan',identity:'model-X'}, ['评测方','评审','设计','model-X']],
    [{kind:'skill_not_visible',role:'plan',skill:'skill-X',runtime:'codex'}, ['设计','skill-X','codex','看不到']],
    [{kind:'skill_not_found',role:'plan',skill:'skill-X'}, ['设计','skill-X','未安装']],
    [{kind:'parallel_out_of_range',value:99,min:1,max:8}, ['并行数 99','1–8']],
    [{kind:'design_rounds_out_of_range',value:0}, ['设计轮数 0']],
    [{kind:'session_minutes_out_of_range',value:1,min:5}, ['会话时长 1','5 分钟']],
    [{kind:'budget_factor_out_of_range',value:0}, ['预算系数 0']],
    [{kind:'both_default_models',evaluator:'review',generator:'plan'}, ['评审','设计','默认模型']],
  ] as const;
  for (const [warning, words] of cases) for (const word of words) expect(configWarning(warning)).toContain(word);
});
test('9 ConfigViolation 的全部变体都有中文，新增变体会失败', () => {
  const source = readFileSync('../../crates/autome-domain/src/config.rs','utf8');
  const variants = (s: string) => [...s.split('enum ConfigViolation {')[1].split('\n}')[0].matchAll(/^ {4}([A-Z]\w*) \{/gm)].map(m => m[1].replace(/[A-Z]/g,c=>'_'+c.toLowerCase()).slice(1)).sort();
  expect(Object.keys(CONFIG_WARNINGS).sort()).toEqual(variants(source));
  expect(Object.keys(CONFIG_WARNINGS).sort()).not.toEqual(variants(source.replace('enum ConfigViolation {','enum ConfigViolation {\n    FutureWarning {')));
});
test('10 未知警告保留 kind 与 key=value，包括空值和对象', () => {
  for (const kind of ['future','toString','__proto__','']) expect(configWarning({kind,value:0,detail:'',extra:{x:1}})).toBe(`${kind} · value=0 · detail= · extra={"x":1}`);
});
test('11 真实 config.get 全部行键有标签并翻译角色', () => {
  const rows = configRows(populatedConfig);
  for (const row of rows) if (row.key.startsWith('roles.')) expect(row.label).toMatch(/^角色 .+（.+）$/); else expect(Object.hasOwn(CONFIG_LABELS,row.key),row.key).toBe(true);
  expect(rows.find(r=>r.key==='roles.0')?.label).toBe('角色 设计（plan）');
  expect(rows.find(r=>r.key==='roles.3')?.label).toBe('角色 实现（impl）');
  for (const key of ['gate.command','gate.timeout_minutes','gate_quick.command','gate_quick.timeout_minutes']) expect(CONFIG_LABELS[key]).toMatch(/[\u4e00-\u9fff]/);
});

test('12 全量配置每个叶子都有行与标签', () => {
  const resolved = { ...capturedConfig.resolved, gate_quick_concurrency: 2, setup: { command: 'true', timeout_minutes: 5 }, merge_after: { command: 'true', timeout_minutes: 5 } };
  const rows = configRows(resolved); const keys: string[] = [];
  function visit(value: unknown, key = '') { if (key === 'roles') return; if (value !== null && typeof value === 'object') Object.entries(value).forEach(([k, v]) => visit(v, key ? key + '.' + k : k)); else keys.push(key); } visit(resolved);
  expect(rows.filter(r => !r.key.startsWith('roles.')).map(r => r.key)).toEqual(keys);
  keys.forEach(key => expect(CONFIG_LABELS[key], key).toBeTruthy());
});
test('13 默认段全部且只有它们有默认徽标', () => {
  const defaults = capturedConfig.defaults; const rows = configRows(capturedConfig.resolved, defaults);
  for (const row of rows) expect(Boolean(row.isDefault)).toBe(defaults.includes(row.key.split('.')[0]));
  const text = html({ ...base, config: { rows, paths: [], warnings: [] } }); expect(text.split('>默认</span>').slice(1)).toHaveLength(rows.filter(r => r.isDefault).length);
});
test('14 角色 provenance 区分全局项目默认', () => {
  const rows = configRows({ roles: ['global', 'project', 'default'].map(provenance => ({ role: 'impl', config: {}, provenance })) });
  expect(rows.map(r => r.source)).toEqual(['全局', '项目', '默认']);
  const text = html({ ...base, config: { rows, paths: [], warnings: [] } }); for (const source of ['全局', '项目', '默认']) expect(text).toContain('>' + source + '</span>');
});
