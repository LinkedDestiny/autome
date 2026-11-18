import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createRoutesStub } from 'react-router';
import { expect,it } from 'vitest';
import { TaskOps } from '../app/components/task-ops-view.tsx';
import { availableOps, type OpsInput } from '../app/components/task-ops.ts';
import { WriteGateProvider } from '../app/lib/live.tsx';
import { MAX_PROSE_LENGTH } from '../server/gates/write-gate.ts';
const render=(input:OpsInput,allowed=true,query='')=>{
  const Stub=createRoutesStub([{path:'/tasks/:id',Component:()=>createElement(WriteGateProvider,{value:{allowed,reason:allowed?'':'内核离线：测试'}},createElement(TaskOps,{ops:availableOps(input)}))}]);
  return renderToStaticMarkup(createElement(Stub,{initialEntries:[`/tasks/T-1${query}`]}));
};
const input:OpsInput={kind:'feature',state:{state:'active',node:'await_design_approval'},roles:null,mergeQueue:false,openDecisions:false};
it('状态决定按钮集合',()=>{const html=render(input);expect(html).toContain('value="approve"');expect(html).toContain('value="reject"');expect(html).not.toContain('value="stop"');});
it('离线所有提交按钮置灰并显示原因',()=>{const html=render(input,false);expect(html).toContain('内核离线：测试');for(const tag of html.match(/<button\b[^>]*>/g)??[])expect(tag).toContain('disabled');expect(html).not.toContain('confirm=cancel');});
it('文本输入上限来自写网关',()=>{for(const i of [input,{...input,kind:'bug',state:{state:'active',node:'await_info'}}])expect(render(i)).toContain(`maxLength="${MAX_PROSE_LENGTH}"`);});
it('取消先 GET 确认，第二步才有 POST 和 confirm=yes',()=>{const first=render(input);expect(first).toContain('confirm=cancel');expect(first).not.toContain('value="yes"');const second=render(input,true,'?confirm=cancel');expect(second).toContain('确认取消这个任务');expect(second).toContain('name="confirm" value="yes"');});
it('无可见操作与结果时隐藏，结果仍可见且不提示无关授权',()=>{
 for(const state of ['done','cancelled']) for(const allowed of [true,false]) expect(render({...input,state:{state},openDecisions:true},allowed)).not.toContain('task-ops');
 for(const result of [{},{notice:'操作完成'},{error:'操作失败'}]) {
  const Stub=createRoutesStub([{path:'/',Component:()=>createElement(WriteGateProvider,{value:{allowed:false,reason:'未授权'}},createElement(TaskOps,{ops:[],result}))}]);
  const html=renderToStaticMarkup(createElement(Stub));
  expect(html.includes('task-ops')).toBe(!!(result.notice||result.error)); expect(html).not.toContain('未授权');
  if(result.notice) expect(html).toContain(result.notice); if(result.error) expect(html).toContain(result.error);
 }
 expect(render(input)).toContain('task-ops');
});
