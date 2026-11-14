import { createHash } from 'node:crypto';
import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, expect, test, vi } from 'vitest';
import { createElement } from 'react';
import { renderToString } from 'react-dom/server';
import { RouterContextProvider, createMemoryRouter, RouterProvider } from 'react-router';
import { w9Kernel } from './fixtures/w9-replies.ts';
import { servicesContext } from '../app/load-context.ts';
import { action, loader, SettingsView } from '../app/routes/projects.$id.settings.tsx';
import { loader as rootLoader, shouldRevalidate } from '../app/root.tsx';
import { revalidateForEvents } from '../app/lib/live-routes.ts';
import type { LedgerEvent } from '../server/eventbus-protocol.ts';
import type { ShellData } from '../app/components/shell-data.ts';
import fixture from './fixtures/w9-replies.json' with { type: 'json' };
const closes: (() => Promise<void>)[]=[];
afterEach(async()=>{for(const close of closes.splice(0).reverse()) await close();});
async function setup() {
  const s=await w9Kernel(); closes.push(s.close); const path=s.repo('single'); const {project}=await s.core.call('project.add',{path}) as {project:{id:string}};
  const context=new RouterContextProvider(); context.set(servicesContext,s); const args={context,params:{id:project.id},request:new Request('http://localhost/'),url:new URL('http://localhost/'),pattern:'/projects/:id/settings'};
  const submit=()=>action({...args,request:new Request('http://localhost/',{method:'POST',body:new URLSearchParams({intent:'remove',name:'single'})})});
  const draft=()=>s.core.call('task.create',{project_id:project.id,kind:'bug',brief:'# Test',draft:true}) as Promise<{task:{id:string}}>;
  return {...s,path,id:project.id,args,submit,draft};
}
function html(data: Awaited<ReturnType<typeof loader>>) {const router=createMemoryRouter([{path:'/',element:createElement(SettingsView,{data})}]);try{return renderToString(createElement(RouterProvider,{router}));}finally{router.dispose();}}
function snapshot(root:string): Record<string,unknown> {const result:Record<string,unknown>={}; function visit(dir:string){ for(const entry of readdirSync(join(root,dir),{withFileTypes:true})){const path=join(dir,entry.name); if(entry.isDirectory())visit(path);else {const bytes=readFileSync(join(root,path));result[path]=[bytes.length,createHash('sha1').update(bytes).digest('hex')];}}}visit('');return result;}
test('1 真实配置值逐行一致且重新加载显示 validate 警告',async()=>{
  const s=await setup();const reply=await s.core.call('config.get',{project_id:s.id}) as {resolved:typeof fixture.config.resolved}; expect(reply.resolved).toEqual(fixture.config.resolved);
  const data=await loader(s.args); const rows=data.config!.rows; function verify(value:unknown,key=''){if(key==='roles')return;if(value!==null&&typeof value==='object')Object.entries(value).forEach(([k,v])=>verify(v,key?key+'.'+k:k));else expect(rows.find(r=>r.key===key)?.value).toBe(value===null?'未配置':String(value));} verify(reply.resolved);
  const markup=html(data); for(const row of rows){expect(markup).toContain(`data-config-key="${row.key}"`);expect(markup).toContain(row.value);}
  writeFileSync(join(s.path,'.autome/config.toml'),'[loop]\nsession_minutes = 1\n');const changed=await loader(s.args);expect(changed.config?.warnings).toEqual([{kind:'session_minutes_out_of_range',min:5,value:1}]);expect(html(changed)).toContain('会话时长 1 分钟低于下限 5 分钟');
});
test('2 草稿任务阻止移除并原样返回内核拒绝',async()=>{const s=await setup();await s.draft();expect(await s.submit()).toEqual({error:'still-running tasks must finish or be cancelled first: T-1'});expect((await loader(s.args)).project?.id).toBe(s.id);});
test('3 取消任务后移除、跳转、事件刷新 root 且文件快照不变',async()=>{
 const s=await setup();const {task}=await s.draft();await s.core.call('task.cancel',{task_id:task.id});
 const router=createMemoryRouter([{path:'/',loader:()=>rootLoader(s.args),shouldRevalidate}]);
 try{await vi.waitFor(()=>expect(router.state.initialized).toBe(true));expect((router.state.loaderData['0'] as ShellData).projects).toHaveLength(1);const before=snapshot(s.path);expect(Object.keys(before).some(p=>p.startsWith('.git/'))).toBe(true);
 const response=await s.submit();expect(response).toBeInstanceOf(Response);expect((response as Response).headers.get('Location')).toBe('/');
 const events=(await s.core.call('events.since',{after_seq:0,limit:1000}) as {events:LedgerEvent[]}).events.filter(e=>e.kind==='project.removed');expect(events).toHaveLength(1);await revalidateForEvents(events,()=>router.revalidate());expect((router.state.loaderData['0'] as ShellData).projects).toHaveLength(0);expect(snapshot(s.path)).toEqual(before);
 }finally{router.dispose();}
});
test('4 坏 TOML 不阻止设置页与移除',async()=>{const s=await setup();writeFileSync(join(s.path,'.autome/config.toml'),'[broken');const direct=await s.gateway.read({method:'config.get',params:{project_id:s.id}});const data=await loader(s.args);expect(data.project?.id).toBe(s.id);expect(data.config?.error).toBe(direct.body.error);expect(data.config?.error).toContain('TOML');expect(html(data)).toContain('移除项目');expect(await s.submit()).toBeInstanceOf(Response);});
