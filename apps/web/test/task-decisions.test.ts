import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createRoutesStub } from 'react-router';
import { expect,it,vi } from 'vitest';
import { TaskDecisions } from '../app/components/task-decisions.tsx';
import type { TaskPayload } from '../app/components/task-detail-data.ts';
import { WriteGateProvider } from '../app/lib/live.tsx';
import { MAX_PROSE_LENGTH } from '../server/gates/write-gate.ts';
import { handleTaskAction } from '../app/components/task-action.ts';
const item={kind:'backlog',item_id:'B-01',consumed:false,disposition:'none',text:'正文',carried:false};
function render(decisions:TaskPayload['decisions'],allowed=true) {
 const Stub=createRoutesStub([{path:'/',Component:()=>createElement(WriteGateProvider,{value:{allowed,reason:'内核离线'}},createElement(TaskDecisions,{decisions}))}]);
 return renderToStaticMarkup(createElement(Stub));
}
it('条目、当前表态、carried 与 consumed 逐项呈现',()=>{
 for(const [disposition,label] of [['none','未表态'],['include','纳入'],['ignore','忽略'],['ruled','已裁定：决定全文']]) {
  const html=render([{...item,disposition,ruling:'决定全文',carried:true},{...item,item_id:'B-99',consumed:true}]);
  for(const text of ['B-01','正文',label,'已交给进行中的一轮'])expect(html).toContain(text);
  expect(html).not.toContain('B-99');
 }
 expect(render([])).toContain('暂无待表态');
});
it('两种表单、撤销免验证、离线禁用与裁定输入边界',async()=>{
 const backlog=render([item]);for(const value of ['include','ignore','none'])expect(backlog).toContain(`value="${value}"`);
 expect(backlog).not.toContain('textarea');
 const dispute=render([{...item,kind:'dispute',item_id:'D-01'}]);
 for(const text of ['name="ruling"',`maxLength="${MAX_PROSE_LENGTH}"`,'value="ruled"','formNoValidate'])expect(dispute).toContain(text);
 expect(render([item],false)).toContain('<fieldset disabled=""');
 const write=vi.fn().mockResolvedValue({status:200,body:{ok:true,payload:{}}});
 for(const ruling of ['', ' ', 'x'.repeat(MAX_PROSE_LENGTH+1), 'x'.repeat(MAX_PROSE_LENGTH)]) {
  write.mockClear();const form=new FormData();Object.entries({intent:'decide',kind:'dispute',item_id:'D-01',disposition:'ruled',ruling}).forEach(([k,v])=>form.set(k,v));
  const result=await handleTaskAction(form,'T-1',{gateway:{read:vi.fn(),write}});
  expect(Boolean(result.error)).toBe(ruling.length!==MAX_PROSE_LENGTH);expect(write).toHaveBeenCalledTimes(ruling.length===MAX_PROSE_LENGTH?1:0);
 }
});
it('表态下一轮生效，不暗示中断当前会话',()=>{expect(render([item])).toContain('表态不会打断正在跑的一轮，下一轮生效');});
