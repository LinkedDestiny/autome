import { createElement } from 'react';
import { renderToString } from 'react-dom/server';
import { createMemoryRouter, RouterProvider, createRoutesStub } from 'react-router';
import { expect, test, vi } from 'vitest';
import { EnvView, action, loader, shouldRevalidate } from '../app/routes/env.tsx';
import { createLoadContext, type WebServices } from '../app/load-context.ts';
import { WriteGateProvider } from '../app/lib/live.tsx';
import { envData, snapshot, recipe } from './fixtures/w9-env.ts';
import type { EnvData, EnvResult } from '../app/components/env-model.ts';
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
  let handler: LiveMessageListener=()=>{}; const revalidate=vi.fn(); const unsubscribe=vi.fn();
  const release=subscribeAdminRevalidate(fn=>{handler=fn;return unsubscribe;},revalidate); handler('env.changed',{type:'env.changed'}); expect(revalidate).toHaveBeenCalledTimes(1); handler('unrelated',{type:'unrelated'}); expect(revalidate).toHaveBeenCalledTimes(1); release(); expect(unsubscribe).toHaveBeenCalled();
  await revalidateForEvents([{seq:1,kind:'env.probing',subject:'environment',payload:{}}],()=>{expect(shouldRevalidate({defaultShouldRevalidate:false,currentUrl:new URL('http://localhost/env'),nextUrl:new URL('http://localhost/env'),currentParams:{}} as Parameters<typeof shouldRevalidate>[0])).toBe(true);});
});
