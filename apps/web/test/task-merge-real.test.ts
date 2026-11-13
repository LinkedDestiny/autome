import { mkdtempSync, readFileSync, existsSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { createElement } from 'react';
import { renderToString } from 'react-dom/server';
import { afterAll, beforeAll, expect, it, vi } from 'vitest';
import { CoreSupervisor } from '../server/core/supervisor.ts';
import { createGateway } from '../server/gateway.ts';
import { createTaskEvents } from '../server/task-events.ts';
import { loadTaskDetail } from '../app/components/task-detail-data.ts';
import { handleTaskAction } from '../app/components/task-action.ts';
import { TaskChangesPanel } from '../app/components/task-changes.tsx';
import { leftoverProcesses, teardown } from './fixtures/process-leak.ts';
const wt=resolve(import.meta.dirname,'../../..'),root=mkdtempSync(join(tmpdir(),'w7-merge-')),home=join(root,'autome-home');
const core=new CoreSupervisor({home,binaryPath:join(root,'core-wrapper')});
const taskEvents=createTaskEvents({call:(method,params)=>core.call(method,params)});
const services={gateway:createGateway({core,home}),taskEvents};
const ids=()=>existsSync(join(root,'task-ids'))?readFileSync(join(root,'task-ids'),'utf8').trim().split('\n'):[];
const cleanup=async()=>{if(existsSync(root))writeFileSync(join(root,'release-main-check'),'');await teardown({root,core,taskIds:ids(),stopServices:()=>taskEvents.close()});};
beforeAll(async()=>{try{
 await promisify(execFile)('bash',[join(wt,'apps/web/test/fixtures/w7-flow.sh'),root,wt],{timeout:150000,maxBuffer:1024*1024});
 await core.start({timeoutMs:30000});
}catch(error){try{await cleanup();}catch(cleanupError){throw new AggregateError([error,cleanupError],'setup and cleanup failed',{cause:cleanupError});}throw error;}},180000);
afterAll(cleanup);
const load=()=>loadTaskDetail(services,'T-1',new URL('http://localhost/tasks/T-1?tab=changes'));
const act=(fields:Record<string,string>)=>{const form=new FormData();Object.entries(fields).forEach(([k,v])=>form.set(k,v));return handleTaskAction(form,'T-1',services);};
it('真实 changes 清单与落后默认分支提示',async()=>{
 const result=await load();expect(result.data?.task.state?.node).toBe('await_merge');
 const html=renderToString(createElement(TaskChangesPanel,{data:result.data!,changes:result.changes}));
 expect(html).toContain('先自动 rebase');expect(html).toContain('docs/feat/feat.md');expect(html).toContain('个提交');
});
it('真实 action 表态 B-01 与 D-01，回包即时更新，再撤销以免阻塞合并',async()=>{
 const decisions: Record<string,string>[] = [{kind:'backlog',item_id:'B-01',disposition:'include'},{kind:'dispute',item_id:'D-01',disposition:'ruled',ruling:'按原方案'}];
 for(const fields of decisions) {
  expect(await act({intent:'decide',...fields})).toHaveProperty('ok',true);
  const detail=await load();expect(detail.data?.decisions?.find(d=>d.item_id===fields.item_id)).toMatchObject({disposition:fields.disposition,...(fields.ruling?{ruling:fields.ruling}:{})});
  if(fields.kind==='backlog')expect(renderToString(createElement(TaskChangesPanel,{data:detail.data!,changes:detail.changes}))).toContain('不会直接合并');
  expect(await act({intent:'decide',kind:fields.kind,item_id:fields.item_id,disposition:'none'})).toHaveProperty('ok',true);
 }
});
it('合并提交后 main_check 挂住期间不是完成，释放后才完成且事件顺序正确',async()=>{
 expect(await act({intent:'merge'})).toEqual({ok:true,notice:'已提交合并'});
 await vi.waitFor(async()=>{await core.call('scheduler.tick',{});const result=await load();if(result.data?.task.state?.node!=='main_check')throw Error(JSON.stringify(result.data?.task.state));}, {timeout:45000,interval:100});
 const waiting=await load();const html=renderToString(createElement(TaskChangesPanel,{data:waiting.data!,changes:waiting.changes}));
 expect(html).toContain('aria-current="step">验证 main');expect(html).not.toContain('aria-current="step">已完成');
 expect(waiting.data?.task.state?.state).toBe('active');
 writeFileSync(join(root,'release-main-check'),'');
 await vi.waitFor(async()=>{await core.call('scheduler.tick',{});const result=await load();if(result.data?.task.state?.state!=='done')throw Error(JSON.stringify(result.data?.task.state));},{timeout:45000,interval:100});
 const done=await load();expect(renderToString(createElement(TaskChangesPanel,{data:done.data!,changes:done.changes}))).toContain('aria-current="step">已完成');
 const states=done.history!.events.filter(e=>e.kind==='task.updated').map(e=>e.payload.state as {state:string;node?:string});
 const merging=states.findIndex(s=>s.node==='merging'||s.node==='main_check'),end=states.findIndex(s=>s.state==='done');expect(merging).toBeGreaterThanOrEqual(0);expect(end).toBeGreaterThan(merging);
},100000);
it('收尾无存活进程',async()=>{await cleanup();expect(leftoverProcesses(root)).toEqual([]);});
