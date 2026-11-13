import { expect, it, vi } from 'vitest';
import { handleTaskAction, WRITE_OPS } from '../app/components/task-action.ts';
import { MAX_PROSE_LENGTH } from '../server/gates/write-gate.ts';
const form=(intent:string,fields:Record<string,string>={})=>{ const f=new FormData(); Object.entries({intent,...fields}).forEach(([k,v])=>f.set(k,v)); return f; };
const setup=()=>({gateway:{write:vi.fn().mockResolvedValue({status:200,body:{ok:true,payload:{task:{id:'T-2'}}}}),read:vi.fn().mockResolvedValue({body:{ok:true,payload:{task:{state:{state:'stopped',at:'design'},merge:{authorized:true}}}}})}});
it('14 种意图精确映射一次写请求',async()=>{
  const extra:Record<string,Record<string,string>>={reject:{feedback:'意见'},reply:{info:'补充'},cancel:{confirm:'yes'},rerun:{node:'rebase'},extend:{extra_rounds:'2'},convert:{start:'yes'},decide:{kind:'backlog',item_id:'B-1',disposition:'include'}};
  const params:Record<string,object>={reject:{feedback:'意见'},reply:{info:'补充'},merge_cancel:{cancel:true},rerun:{node:'rebase'},extend:{extra_rounds:2},convert:{kind:'feature',start:true},decide:extra.decide};
  for(const [intent,op] of Object.entries(WRITE_OPS)) {const s=setup();expect(await handleTaskAction(form(intent,extra[intent]),'T-1',s)).toMatchObject({ok:true});expect(s.gateway.write).toHaveBeenCalledExactlyOnceWith({op,params:{task_id:'T-1',...params[intent]}});}
});
it('400/409/503/504 原文逐字透传',async()=>{
  for(const status of [400,409,503,504]){const s=setup();s.gateway.write.mockResolvedValue({status,body:{ok:false,error:`原文 ${status}\n详细原因`}});expect(await handleTaskAction(form('start'),'T-1',s)).toEqual({error:`原文 ${status}\n详细原因`});}
});
it('取消缺少确认不写',async()=>{const s=setup();expect(await handleTaskAction(form('cancel'),'T-1',s)).toHaveProperty('error');expect(s.gateway.write).not.toHaveBeenCalled();});
it('未知意图、轮数边界、空输入、超长、非法节点与旧内核撤销均拒绝',async()=>{
  const cases=[form('__proto__'),form('other'),...['','0','-1','1.5','1e3','Infinity','4294967296'].map(n=>form('extend',{extra_rounds:n})),form('reject',{feedback:'  '}),form('reply',{info:'x'.repeat(MAX_PROSE_LENGTH+1)}),form('rerun',{node:'../x'}),form('decide',{kind:'backlog',item_id:'B-1',disposition:'ruled',ruling:'x'})];
  for(const f of cases){const s=setup();expect(await handleTaskAction(f,'T-1',s)).toHaveProperty('error');expect(s.gateway.write).not.toHaveBeenCalled();}
  for(const extra_rounds of ['01','1','4294967295']) expect(await handleTaskAction(form('extend',{extra_rounds}),'T-1',setup())).toHaveProperty('ok',true);
  expect(await handleTaskAction(form('reply',{info:'x'.repeat(MAX_PROSE_LENGTH)}),'T-1',setup())).toHaveProperty('ok',true);
  const s=setup();s.gateway.read.mockResolvedValue({body:{ok:true,payload:{task:{state:{state:'active',node:'await_merge'}}}}});
  expect(await handleTaskAction(form('merge_cancel'),'T-1',s)).toHaveProperty('error');expect(s.gateway.write).not.toHaveBeenCalled();
});
it('成功留在本页，合并不声称完成；无网关返回原因',async()=>{
  expect(await handleTaskAction(form('merge'),'T-1',setup())).toEqual({ok:true,notice:'已提交合并'});
  expect(await handleTaskAction(form('start'),'T-1',{gateway:null})).toHaveProperty('error');
});
