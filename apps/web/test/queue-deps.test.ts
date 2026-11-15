import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createRoutesStub } from 'react-router';
import { expect, test, vi } from 'vitest';
import { afterInput, dependencyReadOnly } from '../app/components/queue-deps.ts';
import { QueueControls } from '../app/components/queue-controls.tsx';
import { TaskDependencies } from '../app/components/task-dependencies.tsx';
import { action } from '../app/routes/projects.$id.tsx';
import { handleTaskAction } from '../app/components/task-action.ts';
import { createLoadContext } from '../app/load-context.ts';
import type { TaskPayload } from '../app/components/task-detail-data.ts';
import { WriteGateProvider } from '../app/lib/live.tsx';
const data: TaskPayload = { task: { id: 'T-1', title: '甲', kind: 'feature', state: { state: 'draft' }, after: ['T-2'] }, sessions: [], project: { id: 'P' } };
const task = { id: 'T-1', title: '甲', kind: 'feature', project_id: 'P', state: { state: 'queued' } };
const form = (params: Record<string, string>) => { const f = new FormData(); for (const [k,v] of Object.entries(params)) f.set(k,v); return f; };
async function move(params: Record<string, string>, error?: string) {
  const write = vi.fn(async () => ({ status: error ? 409 : 200, body: error ? { ok: false, error } : { ok: true } }));
  const result = await action({ request: new Request('http://localhost/projects/P', { method: 'POST', body: form({ intent: 'move', task_id: 'T-1', ...params }) }), params: { id: 'P' }, context: createLoadContext({ gateway: { read: vi.fn(), write } })(), url: new URL('http://localhost/projects/P'), pattern: '/projects/:id' });
  return { result, write };
}
test('1 move 三种互斥参数由项目 action 原样组装', async () => {
  for (const [key,value] of [['top','yes'],['before','T-2'],['after','T-2']]) {
    const { write } = await move({ [key]: value }); expect(write).toHaveBeenCalledWith({ op: 'task.move', params: { task_id: 'T-1', [key]: key === 'top' ? true : value } });
  }
  for (const params of [{}, { top: 'no' }, { top: 'yes', before: 'T-2' }, { before: 'T-2', after: 'T-3' }] as Record<string, string>[]) expect((await move(params)).write).not.toHaveBeenCalled();
});
test('2 非排队参照及授权、离线、超时拒绝保留内核原文', async () => {
  for (const error of ['参照任务 T-2 不在排队中', '拒绝授权', '内核未连接', '请求超时']) expect((await move({ before: 'T-2' }, error)).result).toEqual({ ok: false, error });
});
test('3 依赖可改覆盖草稿排队及批准前所有迁移与历史实现防线', () => {
  for (const kind of ['feature', 'bug']) for (const state of ['draft','queued']) expect(dependencyReadOnly({ ...data, task: { ...data.task, kind, state: { state } } })).toBeNull();
  for (const node of ['intake','design','review','adjudicate','await_design_approval']) for (const state of [{ state: 'active', node }, { state: 'queued', resume: node }, { state: 'paused', resume: node }, { state: 'stopped', at: node }, { state: 'failed', at: node }]) expect(dependencyReadOnly({ ...data, task: { ...data.task, state } })).toBeNull();
  expect(dependencyReadOnly({ ...data, sessions: [{ id: 'S', label: '实现', round: 1, started_at: '', running: false, kind: { kind: 'role', role: 'impl' } }] })).toBe('依赖只能在设计批准之前修改');
});
test('4 依赖不可改三种原因及未知状态', () => {
  for (const kind of ['feature','bug']) for (const state of ['done','cancelled']) expect(dependencyReadOnly({ ...data, task: { ...data.task, kind, state: { state } } })).toBe('任务已结束');
  expect(dependencyReadOnly({ ...data, task: { ...data.task, kind: 'bug', state: { state: 'active', node: 'fix' } } })).toBe('Bug 任务开始后不能修改依赖');
  for (const node of ['implement','audit','gate','await_merge','merging','main_check','cleanup']) expect(dependencyReadOnly({ ...data, task: { ...data.task, state: { state: 'active', node } } })).toBe('依赖只能在设计批准之前修改');
  expect(dependencyReadOnly({ ...data, task: { ...data.task, state: undefined } })).toBe('任务状态未知');
});
test('5 保存与清空依赖发送列表并保留拒绝原文', async () => {
  for (const clear of [false,true]) {
    const write = vi.fn(async () => ({ status: 200, body: { ok: true } }));
    await handleTaskAction(form({ intent: 'set_after', after: 'T-2', ...(clear ? {clear:'yes'} : {}) }), 'T-1', { gateway: { read: vi.fn(), write } });
    expect(write).toHaveBeenCalledWith({ op: 'task.set_after', params: { task_id:'T-1', after: clear ? [] : ['T-2'] } });
  }
  const error = '这样设置会形成依赖环：T-1 → T-2 → T-1';
  expect(await handleTaskAction(form({ intent:'set_after',after:'T-2' }), 'T-1', { gateway: { read: vi.fn(), write: async () => ({status:409,body:{ok:false,error}}) } })).toEqual({ error });
});
test('6 输入域零一八九项及空串格式前导零边界', () => {
  for (const size of [0,1,8,9]) { const f = new FormData(); for (let i=0;i<size;i++) f.append('after',`T-${i+1}`); expect(Boolean(afterInput(f).error)).toBe(size>8); }
  for (const id of ['', 'T-x', 'T-1,T-2']) expect(afterInput(form({after:id}))).toHaveProperty('error');
  expect(afterInput(form({after:'T-001'}))).toEqual({after:['T-001']});
});
test('7 调序控件使用原生可聚焦元素且离线禁用', () => {
  const Stub = createRoutesStub([{ path:'/', Component: () => createElement(WriteGateProvider,{value:{allowed:false,reason:'离线'}}, createElement(QueueControls,{task,queue:[task,{...task,id:'T-2'}]})) }]);
  const html = renderToStaticMarkup(createElement(Stub));
  expect(html).toContain('<summary>调整顺序</summary>'); expect(html).toContain('<select'); expect(html).toContain('<fieldset disabled=""');
  for (const name of ['top','before','after']) expect(html).toContain(`name="${name}"`);
});
test('8 依赖区显示当前状态与可改表单只读原因', () => {
  for (const state of ['draft','done']) {
    const Stub = createRoutesStub([{ id:'root', path:'/', Component: () => createElement(TaskDependencies,{data:{...data,task:{...data.task,state:{state}}}}) }]);
    const html = renderToStaticMarkup(createElement(Stub,{ hydrationData:{loaderData:{root:{tasks:[{...task,id:'T-2',state:{state:'cancelled'}}]}}} }));
    expect(html).toContain('T-2 · 已取消'); expect(html.includes('保存依赖')).toBe(state==='draft');
    if (state==='done') expect(html).toContain('任务已结束');
  }
});
