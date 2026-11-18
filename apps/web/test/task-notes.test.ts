import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createRoutesStub } from 'react-router';
import { expect, it, vi } from 'vitest';
import { TaskNotes } from '../app/components/task-notes.tsx';
import type { TaskPayload } from '../app/components/task-detail-data.ts';
import { WriteGateProvider } from '../app/lib/live.tsx';
import { handleTaskAction } from '../app/components/task-action.ts';
const render = (state: NonNullable<TaskPayload['task']['state']>, notes: TaskPayload['notes'] = [], result = {}, allowed = true) => {
  const Stub = createRoutesStub([{ path: '/', Component: () => createElement(WriteGateProvider, { value: { allowed, reason: '内核离线' } }, createElement(TaskNotes, { data: { task: { id: 'T-1', title: '', state }, sessions: [], notes }, result })) }]);
  return renderToStaticMarkup(createElement(Stub));
};
it('各非终态显示表单，结束只显示列表与说明', () => {
  for (const state of ['draft','queued','active','paused','failed','stopped']) expect(render({state})).toContain('提交备注');
  for (const state of ['done','cancelled']) { const html=render({state}); expect(html).not.toContain('<form'); expect(html).toContain('任务已结束，备注不会再被读到'); }
});
it('只有 active await_merge 出 reopen', () => {
  for (const node of ['intake','implement','await_merge','merging','main_check','cleanup']) expect(render({state:'active',node}).includes('name="reopen"')).toBe(node==='await_merge');
  expect(render({state:'stopped',at:'await_merge'})).not.toContain('name="reopen"');
});
it('未送达与送达文案含角色轮次，正文原样且转义', () => {
  const note={n:1,text:'<script>\n备注',at:'2026-11-15T00:00:00Z'};
  expect(render({state:'draft'},[note])).toContain('等待下一轮');
  const html=render({state:'done'},[{...note,delivered:{role:'impl',round:1,at:note.at}}]);
  expect(html).toContain('已送达 · 实现 第 1 轮'); expect(html).toContain('&lt;script&gt;\n备注'); expect(html).toContain('#1');
});
it('warning 与内核拒绝原文显示', async () => {
  expect(render({state:'active',node:'merging'},[],{warning:'合并期间提醒'})).toContain('role="status">合并期间提醒');
  for(const error of ['任务已在等你合并；加 --reopen','修复预算已用完，请先 autome extend','授权拒绝','内核超时','服务不可达']) {
    const form=new FormData();form.set('intent','note');form.set('text','备注');
    const result=await handleTaskAction(form,'T-1',{gateway:{write:vi.fn().mockResolvedValue({body:{ok:false,error}})}} as never);
    expect(result).toEqual({error});expect(render({state:'active'},[],result)).toContain(error);
  }
});
it('空串空白及 8001 拒绝，1 与 8000 保留原文', async () => {
  for(const text of ['', ' \n ', 'x'.repeat(8001), 'x', 'x'.repeat(8000)]) {
    const write=vi.fn().mockResolvedValue({body:{ok:true,payload:{n:1}}});const f=new FormData();f.set('intent','note');f.set('text',text);
    const result=await handleTaskAction(f,'T-1',{gateway:{write}} as never);
    if(!text.trim()||text.length>8000){expect(result).toHaveProperty('error');expect(write).not.toHaveBeenCalled();}else{expect(result).toHaveProperty('ok',true);expect(write).toHaveBeenCalledWith({op:'task.note',params:{task_id:'T-1',text,reopen:false}});}
  }
  expect(render({state:'draft'})).toContain('maxLength="8000"');
});
it('离线禁用并显示原因',()=>{expect(render({state:'draft'},[],{},false)).toContain('<fieldset disabled=""');expect(render({state:'draft'},[],{},false)).toContain('内核离线');});
it('旧内核无 notes 仍可显示，未知角色不误标',()=>{expect(render({state:'draft'})).toContain('备注正文');expect(render({state:'active'},[{n:1,text:'x',at:'2026-11-15',delivered:{role:'future',round:2,at:'2026-11-15'}}])).toContain('future 第 2 轮');});
it('备注正文按 Markdown 渲染：列表与行内代码', () => {
  const html = render({ state: 'draft' }, [{ n: 1, text: '说明 `code`\n\n- 甲\n- 乙', at: '2026-11-15T00:00:00Z' }]);
  expect(html).toContain('<code>code</code>'); expect(html).toContain('<ul>'); expect(html).toContain('<li>甲');
});
