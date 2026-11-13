import { readFileSync } from 'node:fs';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { expect, it, vi } from 'vitest';
import { changesView, detailTab, type TaskChanges } from '../app/components/task-changes-model.ts';
import { TaskChangesPanel } from '../app/components/task-changes.tsx';
import { loadTaskDetail, type TaskPayload } from '../app/components/task-detail-data.ts';
const changes = JSON.parse(readFileSync(new URL('./fixtures/w7/changes.json', import.meta.url), 'utf8')) as TaskChanges;
const data = JSON.parse(readFileSync(new URL('./fixtures/w7/feature.json', import.meta.url), 'utf8')) as TaskPayload;
it('真实回包的文件、总行数、提交数与提交标题均呈现', () => {
  const view = changesView(changes, data);
  expect(view.files).toEqual(changes.files); expect(view.commits).toBe(changes.commits);
  expect([view.added, view.deleted]).toEqual([changes.total_added, changes.total_deleted]);
  const html = renderToStaticMarkup(createElement(TaskChangesPanel, { data, changes: { payload: changes } }));
  for (const file of changes.files!) expect(html).toContain(file.path);
  for (const subject of changes.subjects!) expect(html).toContain(subject);
  expect(html).toContain(`+${changes.total_added}`);
});
it('疑似删除口径、空集合、单个文件、两种阈值及紧邻边界', () => {
  for (const [removed, total, warning] of [[0,0,false],[1,1,false],[2,4,true],[2,5,false],[4,10,false],[5,11,true]] as const) {
    const files = Array.from({ length: total }, (_, i) => ({path: String(i), added: i < removed ? 0 : 2, deleted: 3}));
    expect(changesView({available:true,files},data)).toMatchObject({suspected:removed,warning});
  }
  expect(changesView({available:true,files:[{path:'binary',added:0,deleted:0}]},data).suspected).toBe(0);
});
it('落后提示、未提交路径、纳入 Backlog 及读取失败', () => {
  expect(changesView(changes,data).rebase).toContain('先自动 rebase');
  expect(changesView({...changes,rebase_first:false,repos:[{behind:true}]},data).rebase).toContain('先自动 rebase');
  const view=changesView({...changes,mergeable:false,blocked_by:{kind:'dirty_worktree',paths:['a','b']},pending_decisions:{included:1,ruled:0}},data);
  expect(view.rebase).toBeNull();expect(view.blocked).toEqual(['a','b']);expect(view.included).toContain('不会直接合并');
  expect(changesView({...changes,rebase_first:false,repos:[]},data).rebase).toBeNull();
  for (const error of ['读取失败 EACCES','已拒绝：授权失效','内核离线','请求超时']) expect(renderToStaticMarkup(createElement(TaskChangesPanel,{data,changes:{error}}))).toContain(error);
  expect(changesView({available:false},data).available).toBe(false);
});
it('合并进度逐节点与失败迁移，只有 done 完成', () => {
  for (const [node,index] of [['await_merge',-1],['rebase',0],['merge_gate',0],['merging',0],['main_check',1],['cleanup',1]] as const) {
    const payload={...data,task:{...data.task,state:{state:'active',node}}};
    expect(changesView(changes,payload).progress.findIndex(step=>step.current)).toBe(index);
    const failed={...payload,task:{...payload.task,state:{state:'failed',at:node,reason:{kind:'protocol',detail:'失败原文'}}}};
    expect(changesView(changes,failed).failure).toBeTruthy();
    expect(changesView(changes,failed).progress[2].current).toBe(false);
  }
  expect(changesView(undefined,{...data,task:{...data.task,state:{state:'done'}}}).progress[2].current).toBe(true);
});
it('仅合并段与授权后读 changes，默认标签及显式文档选择', async () => {
  for(const state of [{state:'draft'},{state:'done'},{state:'failed',at:'merge_gate'},...['await_merge','rebase','merge_gate','merging','main_check','cleanup'].map(node=>({state:'active',node}))]) {
    for(const authorized of [false,true]) {
      const payload={...data,task:{...data.task,state,merge:{authorized}}};
      const read=vi.fn(async (request: unknown)=>({status:200,body:{ok:true as const,payload:(request as {method:string}).method==='task.get'?payload:{}}}));
      await loadTaskDetail({gateway:{read,write:vi.fn()}},'T-1',new URL('http://localhost/tasks/T-1'));
      expect(read.mock.calls.some(([request])=>(request as {method:string}).method==='task.changes')).toBe(authorized||state.state==='active');
      expect(detailTab(payload,new URLSearchParams())).toBe(authorized||state.state==='active'?'changes':'docs');
      expect(detailTab(payload,new URLSearchParams('tab=decisions'))).toBe('decisions');
      expect(detailTab(payload,new URLSearchParams('doc=feat.md'))).toBe('docs');
    }
  }
});
