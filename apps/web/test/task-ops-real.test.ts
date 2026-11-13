import { mkdtempSync, readFileSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { afterAll,beforeAll,expect,it } from 'vitest';
import { CoreSupervisor } from '../server/core/supervisor.ts';
import { createGateway } from '../server/gateway.ts';
import { handleTaskAction,WRITE_OPS } from '../app/components/task-action.ts';
import { availableOps,opsInput,type Intent } from '../app/components/task-ops.ts';
import type { TaskPayload } from '../app/components/task-detail-data.ts';
import { leftoverProcesses,teardown } from './fixtures/process-leak.ts';
const wt=resolve(import.meta.dirname,'../../..'),root=mkdtempSync(join(tmpdir(),'w7-ops-')),home=join(root,'autome-home');
const core=new CoreSupervisor({home,binaryPath:join(root,'core-wrapper')});
const services={gateway:createGateway({core,home})};
const ids=()=>existsSync(join(root,'task-ids'))?readFileSync(join(root,'task-ids'),'utf8').trim().split('\n'):[];
const cleanup=()=>teardown({root,core,taskIds:ids()});
let names:Record<string,string>={};
beforeAll(async()=>{try{
  await promisify(execFile)('bash',[join(wt,'apps/web/test/fixtures/w7-states.sh'),root,wt],{timeout:170_000,maxBuffer:1024*1024});
  names=Object.fromEntries(readFileSync(join(root,'state-ids'),'utf8').trim().split('\n').map(l=>l.split(' ')));
  await core.start({timeoutMs:30_000});
}catch(error){try{await cleanup();}catch(cleanupError){throw new AggregateError([error,cleanupError],`setup: ${String(error)}; cleanup: ${String(cleanupError)}`,{cause:cleanupError});}throw error;}},180_000);
afterAll(cleanup);
const get=async(id:string)=>{const r=await services.gateway.read({method:'task.get',params:{task_id:id}});expect(r.body.ok).toBe(true);return r.body.payload as TaskPayload;};
const fields:Partial<Record<Intent,Record<string,string>>>={reject:{feedback:'意见'},reply:{info:'补充'},cancel:{confirm:'yes'},rerun:{node:'intake'},extend:{extra_rounds:'1'},convert:{start:'no'}};
const act=async(op:Intent,id:string)=>{const f=new FormData();Object.entries({intent:op,...fields[op]}).forEach(([k,v])=>f.set(k,v));return handleTaskAction(f,id,services);};
async function matrix(slugs:string[],kind:string){
  for(const slug of slugs){const id=names[slug];let data=await get(id);expect(data.task.kind).toBe(kind);
    const expected: Record<string, object> = {
      'matrix-draft': {state:'draft'}, 'matrix-bug-draft':{state:'draft'}, 'matrix-approval':{state:'active',node:'await_design_approval'},
      'matrix-info':{state:'active',node:'await_info'}, 'matrix-not-bug':{state:'active',node:'not_a_bug'},
      'matrix-crash':{state:'failed',reason:{kind:'session_crashed'}}, 'matrix-budget':{state:'failed',reason:{kind:'design_budget'}},
      'matrix-active':{state:'active',node:'intake'}, 'matrix-bug-active':{state:'active',node:'locate'},
      'matrix-paused':{state:'paused'}, 'matrix-stopped':{state:'stopped'}, 'matrix-cancelled':{state:'cancelled'},
    };
    expect(data.task.state,slug).toMatchObject(expected[slug]);
    const config=await services.gateway.read({method:'config.get',params:{project_id:data.project!.id}});
    const ops=availableOps(opsInput(data,config.body.payload));const allowed=new Set(ops.filter(o=>o.enabled).map(o=>o.op));
    for(const op of Object.keys(WRITE_OPS) as Intent[]){
      // decide is independent of task state; feature reply aliases reject; legacy cancel is unsafe.
      if(allowed.has(op)||op==='decide'||op==='merge_cancel'||(kind==='feature'&&op==='reply'))continue;
      const params:Record<string,unknown>={task_id:id,...fields[op]};
      if(op==='rerun')params.node=kind==='bug'?'locate':'intake';
      if(op==='extend')params.extra_rounds=1;
      if(op==='convert')Object.assign(params,{kind:'feature',start:false});
      const result=await services.gateway.write({op:WRITE_OPS[op],params});expect(result.body.ok,`${slug}/${op}`).toBe(false);
      expect((await get(id)).task.state).toEqual(data.task.state);
    }
    if(allowed.has('cancel')){expect(await act('cancel',id)).toHaveProperty('ok',true);data=await get(id);expect(data.task.state?.state).toBe('cancelled');}
  }
}
it('需求真实内核状态矩阵：所有不可用操作拒绝且状态不变',()=>matrix(['matrix-draft','matrix-approval','matrix-crash','matrix-budget','matrix-active','matrix-paused','matrix-stopped','matrix-cancelled'],'feature'));
it('Bug 真实内核状态矩阵',()=>matrix(['matrix-bug-draft','matrix-info','matrix-not-bug','matrix-bug-active'],'bug'));
const cases:[Intent,string,(data:TaskPayload)=>void][]=[
 ['approve','approve-probe',d=>expect(d.task.state?.node??d.task.state?.resume).toBe('implement')],
 ['reject','reject-probe',d=>expect(d.task.state?.node??d.task.state?.resume).toBe('design')],
 ['reply','reply-probe',d=>expect(d.task.state?.node).toBe('locate')],
 ['merge','T-2',d=>expect(['merging','rebase','merge_gate','main_check','cleanup']).toContain(d.task.state?.node)],
 ['start','start-probe',d=>expect(['active','queued']).toContain(d.task.state?.state)],
 ['pause','pause-probe',d=>expect(d.task.state?.state).toBe('paused')],
 ['resume','resume-probe',d=>expect(['active','queued']).toContain(d.task.state?.state)],
 ['stop','stop-probe',d=>expect(d.task.state?.state).toBe('stopped')],
 ['cancel','cancel-probe',d=>expect(d.task.state?.state).toBe('cancelled')],
 ['rerun','rerun-probe',d=>expect(d.task.state?.node).toBe('intake')],
 ['extend','extend-probe',d=>expect(d.task.state?.node).toBe('design')],
];
for(const [op,slug,check] of cases)it(`真实 action：${op}`,async()=>{const id=names[slug]??slug;if(op==='resume')expect(await act('pause',id)).toHaveProperty('ok',true);expect(await act(op,id)).toHaveProperty('ok',true);check(await get(id));});
it('真实 action：convert 创建新需求并取消原 Bug，留在原页面提示新 id',async()=>{
  const oldId=names['convert-probe'];const result=await act('convert',oldId);expect(result).toHaveProperty('ok',true);
  if(!('convertedTaskId' in result))throw Error('missing converted id');
  const next=await get(result.convertedTaskId!);expect(next.task.kind).toBe('feature');expect(next.task.state?.state).toBe('draft');
  expect((await get(oldId)).task.state?.state).toBe('cancelled');
});
it('内核拒绝原文透传 action',async()=>{const raw=await services.gateway.write({op:'task.start',params:{task_id:'T-1'}});expect(raw.body.ok).toBe(false);expect(await act('start','T-1')).toEqual({error:raw.body.error});});
it('收尾无存活进程',async()=>{await cleanup();expect(leftoverProcesses(root)).toEqual([]);expect(existsSync(root)).toBe(false);});
