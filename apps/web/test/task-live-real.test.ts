import { ensureBuilt } from './fixtures/ensure-built.ts';
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { createServer } from 'node:net';
import WebSocket from 'ws';
import { afterAll, beforeAll, expect, it, vi } from 'vitest';
import { startServer } from '../server/main.ts';
import type { CoreSupervisor } from '../server/core/supervisor.ts';
import type { LedgerEvent } from '../server/eventbus-protocol.ts';
import { readTaskEvents } from '../server/task-history.ts';
import { loadTaskDetail } from '../app/components/task-detail-data.ts';
import { shouldRevalidate } from '../app/routes/tasks.$id.tsx';
import { revalidateForEvents } from '../app/lib/live-routes.ts';
import { leftoverProcesses, teardown } from './fixtures/process-leak.ts';
const wt=resolve(import.meta.dirname,'../../..'),root=mkdtempSync(join(tmpdir(),'w7-live-')),home=join(root,'autome-home');
const exec=promisify(execFile),messages:LedgerEvent[]=[],extraIds:string[]=[];
let running:Awaited<ReturnType<typeof startServer>>|undefined,core:CoreSupervisor|undefined,ws:WebSocket|undefined;
let events:{ forTask: (id: string) => ReturnType<typeof readTaskEvents> }|undefined,liveId='';
const ids=()=>[...(existsSync(join(root,'task-ids'))?readFileSync(join(root,'task-ids'),'utf8').trim().split('\n'):[]),...extraIds];
const cleanup=async()=>{
 if(existsSync(root))writeFileSync(join(root,'release-main-check'),'');ws?.terminate();
 await teardown({root,core,taskIds:ids(),stopServices:async()=>{const service=running;running=undefined;await service?.close();}});
};
async function create(slug:string,script:string) {
 const fake=join(root,'repo/.autome/fake',slug);mkdirSync(fake,{recursive:true});writeFileSync(join(fake,'next'),'1');writeFileSync(join(fake,'1.sh'),script);
 const {project}=await core!.call('task.get',{task_id:'T-1'}) as {project:{id:string}};
 const {task}=await core!.call('task.create',{project_id:project.id,slug,brief:`# ${slug}`,draft:true}) as {task:{id:string}};
 extraIds.push(task.id);await core!.call('task.start',{task_id:task.id});return task.id;
}
beforeAll(async()=>{try{
 await new Promise<void>((resolve,reject)=>{const probe=createServer();probe.once('error',reject);probe.listen(0,'127.0.0.1',()=>probe.close(error=>error?reject(error):resolve()));});
 ensureBuilt();
 await exec('bash',[join(wt,'apps/web/test/fixtures/w7-flow.sh'),root,wt],{timeout:150000,maxBuffer:1024*1024});
 running=await startServer({port:0,mode:'production',home,binaryPath:join(root,'core-wrapper')});core=running.gateway.core as CoreSupervisor;
 await vi.waitFor(()=>{if(!core?.isConnected())throw Error('core not ready');});
 events={ forTask: id => readTaskEvents((m,p)=>core!.call(m,p), id) };
 liveId=await create('live-probe',`exec '${root}/bin/w7-sleep' 300\n`);
 await vi.waitFor(async()=>{await core!.call('scheduler.tick',{});const result=await core!.call('task.get',{task_id:liveId}) as {sessions:{running:boolean}[]};if(!result.sessions.some(s=>s.running))throw Error('not running');},{timeout:15000,interval:50});
 const address=running.server.address();if(!address||typeof address==='string')throw Error('no address');
 const origin=`http://127.0.0.1:${address.port}`;ws=new WebSocket(origin.replace('http:','ws:')+'/ws',{origin});let hello=false;
 ws.on('message',data=>{const m=JSON.parse(data.toString());if(m.type==='hello')hello=true;if(m.type==='event')messages.push(m);});
 await vi.waitFor(()=>{if(!hello)throw Error('no hello');});
}catch(error){try{await cleanup();}catch(cleanupError){throw new AggregateError([error,cleanupError],'setup and cleanup failed',{cause:cleanupError});}throw error;}},240000);
afterAll(cleanup);
async function revalidates(event:LedgerEvent,id:string) {
 let matched=false;const url=new URL(`http://localhost/tasks/${id}`);
 await revalidateForEvents([event],()=>{matched=shouldRevalidate({currentUrl:url,nextUrl:url,currentParams:{id},defaultShouldRevalidate:false} as never);});return matched;
}
it('CLI 暂停后一个 tick 内收到事件、重验并返回新状态',async()=>{
 const start=performance.now();
 await exec(process.env.AUTOME_BIN??join(wt,'target/debug/autome'),['pause',liveId],{env:{...process.env,AUTOME_HOME:home},timeout:10000});
 // Pause has no stop's fixed 3s wait: 3s ticker + 50ms polling + 1450ms CLI/RPC budget.
 await vi.waitFor(()=>expect(messages.some(e=>e.subject===liveId&&e.kind==='task.updated'&&(e.payload as {state?:{state:string}}).state?.state==='paused')).toBe(true),{timeout:4500,interval:50});
 expect(performance.now()-start).toBeLessThan(4500);
 const event=messages.find(e=>e.subject===liveId&&e.kind==='task.updated'&&(e.payload as {state?:{state:string}}).state?.state==='paused')!;
 expect(await revalidates(event,liveId)).toBe(true);
 const detail=await loadTaskDetail({gateway:running!.gateway,taskEvents:events},liveId,new URL(`http://localhost/tasks/${liveId}`));expect(detail.data?.task.state?.state).toBe('paused');
});
it('八个前缀使用真实内核事件，本任务重验、其它任务不重验',async()=>{
 await core!.call('task.merge',{task_id:'T-2'});
 await vi.waitFor(async()=>{await core!.call('scheduler.tick',{});const history=await events!.forTask('T-2');if(!history.events.some(e=>e.kind.startsWith('merge.')))throw Error('no merge event');},{timeout:30000,interval:100});
 const cooldownId=await create('cooldown-probe',`printf '%s\\n' '{"type":"turn.failed","error":{"message":"exceeded retry limit, last status: 429"}}' '{"type":"result","is_error":true,"result":"API Error: 429 rate_limit_error"}'\nexit 1\n`);
 await vi.waitFor(async()=>{await core!.call('scheduler.tick',{});if(!(await events!.forTask(cooldownId)).events.some(e=>e.kind==='runtime.cooldown'))throw Error('no cooldown event');},{timeout:20000,interval:100});
 const all=(await Promise.all(ids().map(id=>events!.forTask(id)))).flatMap(result=>result.events);
 for(const prefix of ['task.','session.','gate.','bug.','merge.','rebase.','guard.','runtime.']) {
  const event=all.find(e=>e.kind.startsWith(prefix));expect(event,prefix).toBeDefined();
  expect(await revalidates(event!,event!.subject)).toBe(true);expect(await revalidates(event!,'T-unrelated')).toBe(false);
 }
},60000);
it('收尾无存活进程',async()=>{await cleanup();expect(leftoverProcesses(root)).toEqual([]);});
