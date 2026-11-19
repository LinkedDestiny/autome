import { createElement } from 'react';
import { renderToString } from 'react-dom/server';
import { createMemoryRouter, RouterProvider, createRoutesStub } from 'react-router';
import { expect, test, vi } from 'vitest';
import { EnvView, action, loader, shouldRevalidate } from '../app/routes/env.tsx';
import { createLoadContext, type WebServices } from '../app/load-context.ts';
import { WriteGateProvider } from '../app/lib/live.tsx';
import { envData, snapshot, recipe } from './fixtures/w9-env.ts';
import { toolOk, type EnvData, type EnvResult } from '../app/components/env-model.ts';
import { SidebarFooter } from '../app/components/sidebar-footer.tsx';
import Overview from '../app/routes/_index.tsx';
import type { LiveMessageListener } from '../app/lib/live-client.ts';
import { subscribeAdminRevalidate } from '../app/components/admin-live.ts';
import { revalidateForEvents } from '../app/lib/live-routes.ts';
function html(data: EnvData, result?: EnvResult, allowed=true) {
  const router=createMemoryRouter([{path:'/',element:createElement(WriteGateProvider,{value:{allowed,reason:'离线'}},createElement(EnvView,{data,result}))}]);
  try{return renderToString(createElement(RouterProvider,{router}));}finally{router.dispose();}
}
function args(services: WebServices, form: Record<string,string>={}) { return {context:createLoadContext(services)(),params:{},request:new Request('http://localhost/env',{method:'POST',body:new URLSearchParams(form)}),url:new URL('http://localhost/env'),pattern:'/env'}; }
const ok=(payload:unknown)=>({status:200,body:{ok:true,payload}});
test('1 所有组件与四种登录状态；未检测不报缺项', async()=>{
  const text=html(envData); for(const word of ['Git','Claude Code','Codex','iTerm2','git version 1','/bin/git','user@example.com','登录已过期','不需要登录','登录状态未知：检测原文','未安装','需要处理']) expect(text).toContain(word);
  const unprobed=html({...envData,snapshot:{...snapshot,probed:false,probing:true}}); expect(unprobed).toContain('正在检测'); expect(unprobed).not.toContain('未安装'); expect(unprobed).not.toContain('需要处理');
  const read=vi.fn(async()=>ok({...snapshot,probed:false})); await loader(args({gateway:{read,write:vi.fn()}})); expect(read).toHaveBeenCalledTimes(1);
  for(const error of ['拒绝授权','读盘失败','请求超时','内核未连接']) { const data=await loader(args({gateway:{read:vi.fn(async()=>({status:503,body:{ok:false,error}})),write:vi.fn()}})); expect(html(data)).toContain(error); }
});
test('2 命令、前置条件、note 原文与离线写按钮',async()=>{
  const text=html(envData); for(const word of [recipe.recipe.command,recipe.recipe.note!,recipe.login_command!,'npm','已满足']) expect(text).toContain(word);
  const unmet=html({...envData,recipes:{claude:{...recipe,prerequisite_ok:false}}}); expect(unmet).toContain('未满足'); expect(unmet).toMatch(/value="install" disabled/);
  const offline=html(envData,undefined,false); for(const intent of ['detect','install','login']) expect(offline).toMatch(new RegExp(`value="${intent}" disabled`));
  const read=vi.fn(async({method}:{method:string})=>method==='env.get'?ok(snapshot):{status:504,body:{ok:false,error:'recipe 超时'}});
  const data=await loader(args({gateway:{read,write:vi.fn()}})); expect(read).toHaveBeenCalledTimes(5); expect(Object.keys(data.errors)).toHaveLength(4); expect(html(data)).toContain('recipe 超时');
});
test('3 detect 强制检测与所有 action 路由、错误反馈',async()=>{
  const write=vi.fn(async()=>ok({})), services={gateway:{read:vi.fn(),write}};
  await action(args(services,{intent:'detect'})); expect(write).toHaveBeenLastCalledWith({op:'env.detect',params:{force:true}});
  for(const intent of ['install','login']) {await action(args(services,{intent,component:'codex',command:'evil'})); expect(write).toHaveBeenLastCalledWith({op:`env.${intent}`,params:{component:'codex'}});}
  expect(html({...envData,snapshot:{...snapshot,probing:true}})).toMatch(/value="detect" disabled/);
  write.mockClear(); expect(await action(args(services,{intent:''}))).toHaveProperty('error'); expect(write).not.toHaveBeenCalled(); expect(await action(args({gateway:null},{intent:'detect'}))).toHaveProperty('error');
});
test('4 终端不可用给命令原文与复制；成功、失败提示',()=>{
  const text=html(envData,{launched:false,command:'codex login',reason:'Terminal 无法打开'}); for(const word of ['codex login','Terminal 无法打开','复制','自己在终端里执行']) expect(text).toContain(word);
  expect(html(envData,{launched:true})).toContain('已在终端里执行'); expect(html(envData,{error:'拒绝原文'})).toContain('拒绝原文');
});
test('5 首页只在已检测且有缺项时提示',()=>{
  for(const environment of [snapshot,{...snapshot,probed:false},{...snapshot,problems:[]}]){
    const data={kernel:{ok:true},projects:[],tasks:[],environment}; const Stub=createRoutesStub([{id:'root',path:'/',loader:()=>data,Component:Overview}]);
    const text=renderToString(createElement(Stub,{hydrationData:{loaderData:{root:data}}}));
    if(environment.probed&&environment.problems.length){expect(text).toContain('本机环境有 2 项需要处理');expect(text).toContain('href="/env"');}else expect(text).not.toContain('去环境页');
  }
});
test('6 侧栏环境入口',()=>{const router=createMemoryRouter([{path:'/',element:createElement(SidebarFooter)}]);try{expect(renderToString(createElement(RouterProvider,{router}))).toContain('href="/env"');}finally{router.dispose();}});
test('7 env.probing 与 env.changed 触发重载，其他消息不触发',async()=>{
  vi.useFakeTimers();
  let handler: LiveMessageListener=()=>{}; const revalidate=vi.fn(); const unsubscribe=vi.fn();
  const release=subscribeAdminRevalidate(fn=>{handler=fn;return unsubscribe;},revalidate);
  try {
    handler('env.changed',{type:'env.changed'});
    await vi.advanceTimersByTimeAsync(999); expect(revalidate).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1); expect(revalidate).toHaveBeenCalledTimes(1);
    handler('unrelated',{type:'unrelated'});
    await vi.advanceTimersByTimeAsync(1000); expect(revalidate).toHaveBeenCalledTimes(1);
    const probing = revalidateForEvents([{seq:1,kind:'env.probing',subject:'environment',payload:{}}],()=>{expect(shouldRevalidate({defaultShouldRevalidate:false,currentUrl:new URL('http://localhost/env'),nextUrl:new URL('http://localhost/env'),currentParams:{}} as Parameters<typeof shouldRevalidate>[0])).toBe(true);});
    await vi.advanceTimersByTimeAsync(1000); await probing;
  } finally { release(); await vi.runOnlyPendingTimersAsync(); vi.useRealTimers(); }
  expect(unsubscribe).toHaveBeenCalled();
});

test('8 驱动卡片区分本面板、推进中、停止与旧内核', async () => {
  const driver = { pid: 123, command: 'autome core', started_at: '2026-01-01T00:00:00Z', last_tick_at: '2026-01-01T00:00:02Z', driving: true, self: true };
  const drivers = async () => ({ available: true, list: [driver, { ...driver, pid: 456, self: false, driving: false, command: 'autome run' }] });
  const data = await loader(args({ gateway: { read: vi.fn(async () => ok({ ...snapshot, probed: false })), write: vi.fn() }, drivers }));
  const text = html(data);
  for (const word of ['驱动', '本面板', '推进中', '已停止推进', '456', 'autome run', driver.last_tick_at]) expect(text).toContain(word);
  expect(html({ ...envData, drivers: { available: false, list: [] } })).toContain('内核较旧，只能推断');
  expect(html({ ...envData, drivers: { available: false, list: [], error: '读取失败' } })).toContain('驱动信息暂不可用：读取失败');
});

test('工具每项四列，状态路径登录版本与内核独立一行', () => {
  const text = html(envData);
  expect(text.match(/class="env-tool-row"/g)).toHaveLength(5);
  expect(text).toContain('env-dot-warning');
  expect(text).toContain('pill-blue');
  expect(text).toContain('工具检查');
});
test('内核行复用 api instance 信息源，失败明确回退', async () => {
  const kernel = { path: '/real/autome', build_id: 'build-123', source: 'test' };
  const instanceStatus = vi.fn(async () => ({ kernel }));
  const data = await loader(args({ gateway: null, instanceStatus }));
  expect(instanceStatus).toHaveBeenCalledOnce();
  expect(data.kernel).toEqual(kernel);
  expect(html(data)).toContain('/real/autome'); expect(html(data)).toContain('build-123');
  const failed = await loader(args({ gateway: null, instanceStatus: async () => { throw new Error('offline'); } }));
  expect(html(failed)).toContain('内核信息暂不可用');
});
test('驱动绿色图案卡和空驱动、检查摘要', () => {
  expect(html(envData)).toContain('card-pat card-green env-drivers');
  expect(html(envData)).toContain('2 项有问题');
  expect(html({ ...envData, snapshot: { ...snapshot, problems: [] }, drivers: { available: true, list: [] } })).toContain('4 项检查全部正常');
});
test('检测和驱动时间仅在 time 属性保留 ISO，正文为相对时间', () => {
  const at = '2026-01-01T00:00:00Z';
  const text = html({ ...envData, snapshot: { ...snapshot, environment: { ...snapshot.environment!, checked_at: at } }, drivers: { available: true, list: [{ pid: 12, command: 'autome core', started_at: at, last_tick_at: at, self: true, driving: true }] } });
  expect(text.match(/<time /g)).toHaveLength(3);
  expect(text).toContain('title="' + at + '"');
  const visible = text.replace(/<[^>]+>/g, '');
  expect(visible).not.toContain(at);
  for (const word of ['前启动', '前检查过', '前检测过']) expect(visible).toContain(word);
});

test('正常五行标记且安装说明默认收起', () => {
  const components = snapshot.environment!.components.map(c => ({ ...c, present: true, login: { login: 'not_applicable' as const } }));
  const text = html({ ...envData, kernel: { path: '/bin/autome', build_id: 'test', source: 'test' }, snapshot: { ...snapshot, problems: [], environment: { components } } });
  expect(text.match(/data-env-ok="true"/g)).toHaveLength(5);
  expect(text.match(/<details class="env-install">/g)).toHaveLength(4);
  expect(text.match(/<summary class="btn btn-text btn-sm">查看安装方法/g)).toHaveLength(4);
});
test('未安装、过期、未知登录或问题列表中的工具自动展开', () => {
  const base = snapshot.environment!.components[0];
  for (const login of ['ok', 'not_applicable', 'expired', 'unknown'] as const) {
    for (const present of [true, false]) for (const listed of [true, false]) {
      const component = { ...base, present, login: { login } };
      const problems = listed ? [component] : [];
      const expected = present && !listed && ['ok', 'not_applicable'].includes(login);
      expect(toolOk(component, problems)).toBe(expected);
      const text = html({ ...envData, snapshot: { ...snapshot, problems, environment: { components: [component] } } });
      expect(text.includes('<details class="env-install" open="">')).toBe(!expected);
    }
  }
  expect(html({ recipes: {}, errors: {}, snapshot: { ...snapshot, environment: { components: [] }, problems: [] } })).not.toContain('env-install');
});

test('重新检测四种健康状态迁移的 open 属性随健康值变化', () => {
  for (const before of [true, false]) for (const after of [true, false]) {
    const render = (present: boolean) => html({ ...envData, snapshot: { ...snapshot, problems: [], environment: { components: [{ ...snapshot.environment!.components[0], present }] } } });
    expect(render(before).includes('<details class="env-install" open="">')).toBe(!before);
    expect(render(after).includes('<details class="env-install" open="">')).toBe(!after);
  }
});
