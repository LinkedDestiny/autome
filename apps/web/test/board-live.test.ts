import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createMemoryRouter, createRoutesStub } from 'react-router';
import { afterAll, beforeAll, expect, test, vi } from 'vitest';
import { CoreSupervisor } from '../server/core/supervisor.ts';
import { createGateway } from '../server/gateway.ts';
import { openWebStore } from '../server/store.ts';
import { action, loader, shouldRevalidate } from '../app/routes/projects.$id.tsx';
import { createLoadContext, type WebServices } from '../app/load-context.ts';
import { boardColumns } from '../app/components/board-data.ts';
import { TaskCard } from '../app/components/task-card.tsx';
import { cardView } from '../app/components/task-card-model.ts';
import { WriteGateProvider } from '../app/lib/live.tsx';
import { revalidateForEvents } from '../app/lib/live-routes.ts';
import { publishLiveMessage, subscribeLiveMessages } from '../app/lib/live-client.ts';
import { revalidatesOnMessage, subscribeNotificationRevalidate } from '../app/components/board-live.ts';
import type { LedgerEvent } from '../server/eventbus-protocol.ts';
const wt=resolve(import.meta.dirname,'../../..'), root=mkdtempSync(join(tmpdir(),'w6-live-')), home=join(root,'home');
const core=new CoreSupervisor({home,binaryPath:join(root,'core-wrapper')}), gateway=createGateway({core,home});
const command=promisify(execFile), started=performance.now(); let projectId:string;
const cli=(...args:string[])=>command(join(wt,'target/debug/autome'),args,{cwd:join(root,'repo'),env:{...process.env,AUTOME_HOME:home},timeout:30_000});
function args(services:WebServices={gateway}) {return {request:new Request('http://localhost/projects/'+projectId),params:{id:projectId},context:createLoadContext(services)(),url:new URL('http://localhost/projects/'+projectId),pattern:'/projects/:id'};}
async function submit(id:string,intent='start',services:WebServices={gateway}) {const form=new FormData();form.set('intent',intent);form.set('task_id',id);return action({...args(services),request:new Request('http://localhost/projects/P',{method:'POST',body:form})});}
async function board(){const b=await loader(args());if('error' in b)throw Error(b.error);return b;}
beforeAll(async()=>{await command('sh',[join(wt,'apps/web/test/fixtures/w6-project.sh'),root,wt,'1'],{timeout:60_000});await core.start({timeoutMs:30_000});const r=await gateway.read({method:'project.list'});projectId=(r.body.payload as {projects:{project:{id:string}}[]}).projects[0].project.id;},120_000);
afterAll(async()=>{try{for(const t of (await board()).tasks.filter(t=>['active','queued'].includes(t.state.state)))await cli('stop',t.id);}finally{await core.stop();rmSync(root,{recursive:true,force:true});}process.stdout.write(`W6 board-live setup/tests/cleanup ${Math.ceil(performance.now()-started)} ms\n`);},120_000);
test('1 event prefixes and non-event navigation',async()=>{
 const url=new URL('http://localhost/projects/P'); const a={currentUrl:url,nextUrl:url,currentParams:{},nextParams:{},defaultShouldRevalidate:false};
 for(const kind of ['task.updated','bug.updated','merge.done','gate.done','session.done','runtime.cooldown','config.updated','integrity_warning',''])await revalidateForEvents([{kind,seq:1,subject:'T-1',payload:{}}],()=>{expect(shouldRevalidate(a)).toBe(!['config.updated','integrity_warning',''].includes(kind));});
 expect(shouldRevalidate({...a,defaultShouldRevalidate:true})).toBe(true);expect(shouldRevalidate(a)).toBe(false);
});
test('2 only draft renders start; closed gate disables with reason',()=>{
 for(const state of ['draft','queued','active','stopped','done'])for(const allowed of [true,false]){
 const Stub=createRoutesStub([{path:'/',Component:()=>createElement(WriteGateProvider,{value:{allowed,reason:'连接中断，正在重连'}},createElement(TaskCard,{task:{id:'T-1',title:'测试',kind:'feature',project_id:'P',state:{state}},detail:{queue_position:1} as never,context:{now:new Date().toISOString(),fixRounds:0,cooldowns:[],unread:{}}}))}]);
 const html=renderToStaticMarkup(createElement(Stub));expect(html.includes('>开始</button>')).toBe(state==='draft');if(state==='draft'&&!allowed){expect(html).toMatch(/<button[^>]*disabled=""/);expect(html).toContain('连接中断');}if(state==='queued')expect(html).toContain('排队第 1 位');
 }
});
test('3 action exact operation, unknown intent, failures and empty id',async()=>{
 const write=vi.fn(async()=>({status:200,body:{ok:true,payload:{started:true}}}));await submit('T-1','start',{gateway:{write,read:vi.fn()}});expect(write).toHaveBeenCalledWith({op:'task.start',params:{task_id:'T-1'}});
 for(const intent of ['other',''])expect(await submit('T-1',intent,{gateway:{write,read:vi.fn()}})).toMatchObject({data:{error:'未知操作'},init:{status:400}});expect(write).toHaveBeenCalledTimes(1);
 for(const [status,error] of [[409,'任务不能开始'],[400,'已拒绝：task_id'],[503,'内核未连接'],[504,'请求超时']] as const)expect(await submit('','start',{gateway:{read:vi.fn(),write:async()=>({status,body:{ok:false,error}})}})).toEqual({ok:false,error});
 expect(await submit('T-1','start',{gateway:null})).toEqual({error:'内核未连接：服务未接入网关'});
 expect(await submit('')).toMatchObject({ok:false,error:expect.stringContaining('task_id 必须是非空字符串')});
});
test('4 CLI draft starts into running with empty slot',async()=>{
 await cli('task','new','开始测试','--draft','-p',projectId);const t=(await board()).tasks[0];expect(t.state.state).toBe('draft');expect(await submit(t.id)).toMatchObject({ok:true,payload:{started:true}});const b=await board();expect(boardColumns(b.tasks,b.now)[1].tasks.map(t=>t.id)).toContain(t.id);
},120_000);
test('5 full slot queues without start button',async()=>{
 await cli('task','new','排队测试','--draft','-p',projectId);const t=(await board()).tasks.find(t=>t.state.state==='draft')!;expect(await submit(t.id)).toMatchObject({ok:true,payload:{started:false}});const b=await board();const queued=b.tasks.find(x=>x.id===t.id)!;expect(boardColumns(b.tasks,b.now)[0].tasks).toContain(queued);expect(cardView(queued,b.details![t.id],b.cardContext!).activity).toBe('排队第 1 位');
},120_000);
test('6 external stop event moves persistent router card to needs-you',async()=>{
 const router=createMemoryRouter([{path:'/projects/:id',loader:()=>loader(args()),shouldRevalidate}],{initialEntries:['/projects/'+projectId]});try{
 await vi.waitFor(()=>expect(router.state.initialized).toBe(true));const before=router.state.loaderData['0'] as Awaited<ReturnType<typeof board>>;const t=boardColumns(before.tasks,before.now)[1].tasks[0];expect(t).toBeDefined();await cli('stop',t.id);
 const events=((await core.call('events.since',{after_seq:0,limit:1000})) as {events:LedgerEvent[]}).events.filter(e=>e.kind==='task.updated'&&e.subject===t.id);expect(events.length).toBeGreaterThan(0);await revalidateForEvents(events,()=>router.revalidate());const b=router.state.loaderData['0'] as typeof before;expect(boardColumns(b.tasks,b.now)[2].tasks.map(t=>t.id)).toContain(t.id);expect(boardColumns(b.tasks,b.now)[1].tasks.map(t=>t.id)).not.toContain(t.id);
 }finally{router.dispose();}
},120_000);
test('7 real notification store and production subscription revalidate persistent loader',async()=>{
 const store=openWebStore(home);const b=await board(),t=b.tasks.find(t=>t.state.state==='stopped')!;let seq=100;
 const insert=()=>store.commit({cursor:{name:'board-test',seq:seq++},notification:{kind:'await_design_approval',project_id:projectId,task_id:t.id,title:'等批准',body:null,event_seq:seq}})!;
 const row=insert();const router=createMemoryRouter([{path:'/',loader:()=>loader(args({gateway,unread:store})),shouldRevalidate}]);let unsubscribe=()=>{};
 try{await vi.waitFor(()=>expect(router.state.initialized).toBe(true));const badge=()=>{const b=router.state.loaderData['0'] as Awaited<ReturnType<typeof board>>;return cardView(b.tasks.find(x=>x.id===t.id)!,b.details![t.id],b.cardContext!).badge;};expect(badge()).toContain('1');
 unsubscribe=subscribeNotificationRevalidate(subscribeLiveMessages,()=>router.revalidate());store.markRead([row.id]);publishLiveMessage('notification-read',{type:'notification-read'});await vi.waitFor(()=>expect(badge()).toBe('等你处理'));
 insert();publishLiveMessage('notification',{type:'notification'});await vi.waitFor(()=>expect(badge()).toContain('1'));unsubscribe();store.markAllRead();publishLiveMessage('notification-read',{type:'notification-read'});expect(badge()).toContain('1');expect(router.state.revalidation).toBe('idle');
 for(const type of ['notification','notification-read','event','hello',''])expect(revalidatesOnMessage(type)).toBe(type.startsWith('notification'));
 expect(readFileSync(new URL('../app/routes/projects.$id.tsx',import.meta.url),'utf8')).toContain('useNotificationRevalidate();');expect(readFileSync(new URL('../app/components/board-live.ts',import.meta.url),'utf8')).toContain('useEffect(() => subscribeNotificationRevalidate(subscribeLiveMessages, revalidate)');
 }finally{unsubscribe();router.dispose();store.close();}
},120_000);
