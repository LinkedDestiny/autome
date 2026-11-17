import { dependencyCandidates } from '../app/components/queue-deps.ts';
import { readFileSync } from 'node:fs';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createRoutesStub } from 'react-router';
import { expect, test, vi } from 'vitest';
import { TASK_TEMPLATES, switchKind } from '../app/components/task-templates.ts';
import { validateNewTask } from '../app/components/task-new-action.ts';
import NewTask, { action } from '../app/routes/projects.$id.tasks.new.tsx';
import { createLoadContext, type WebServices } from '../app/load-context.ts';
import { WriteGateProvider } from '../app/lib/live.tsx';
import { MAX_BRIEF_BYTES } from '../server/gates/write-gate.ts';
import { UPLOAD_MAX_BYTES, UPLOAD_MAX_FILES } from '../server/uploads-limits.ts';
const plugin = readFileSync(new URL('../../../plugins/autome/skills/autome-task/scripts/check-card.sh', import.meta.url), 'utf8');
const lists = [...plugin.matchAll(/split\("([^"]+)"/g)].map(m => m[1].split(' '));
const headings = (body: string) => [...body.matchAll(/^## (.+)$/gm)].map(m => m[1]);
export function form(intent = 'start') { const form = new FormData(); for (const [k,v] of Object.entries({kind:'feature', brief:'# 测试\n正文', intent})) form.set(k,v); return form; }
export function submit(form: FormData, services: WebServices) {
  return action({ request: new Request('http://localhost/projects/P/tasks/new', { method: 'POST', body: form }), params: { id:'P' }, url: new URL('http://localhost/projects/P/tasks/new'), pattern: '/projects/:id/tasks/new', context: createLoadContext(services)() });
}
function render(allowed = true, exists = true, error?: string) {
  const Stub = createRoutesStub([{ id:'root', path:'/projects/:id/tasks/new', Component: () => createElement(WriteGateProvider, { value: { allowed, reason: allowed ? '' : '连接中断，正在重连' } }, createElement(NewTask)) }]);
  return renderToStaticMarkup(createElement(Stub, { initialEntries:['/projects/P/tasks/new'], hydrationData: { loaderData: { root: { projects: exists ? [{id:'P', name:'项目甲'}] : [] } }, actionData: error ? {root:{error}} : undefined } }));
}
const uploads = { describe: (id: string) => id === 'gone' ? null : {name:id === 'duplicate' ? 'a' : id, size:1} } as WebServices['uploads'];
test('1 feature template matches plugin headings', () => { expect(TASK_TEMPLATES.feature.split('\n')[0]).toBe('# <标题>'); expect(headings(TASK_TEMPLATES.feature)).toEqual(lists.find(x => x[0] === '背景')); });
test('2 bug template explicitly retains optional suffix', () => { const list = lists.find(x => x[0] === '现象')!; expect(TASK_TEMPLATES.bug.split('\n')[0]).toBe('# <标题>'); expect(headings(TASK_TEMPLATES.bug)).toEqual([...list.slice(0,5), list[5]+'（可选）']); });
test('3 type switches preserve edited bodies in both directions', () => {
  for (const from of ['feature','bug'] as const) for (const to of ['feature','bug'] as const) {
    for (const body of [TASK_TEMPLATES[from], '', ' \n']) expect(switchKind(body,from,to)).toEqual({body:TASK_TEMPLATES[to],kept:false});
    expect(switchKind('# 我的正文',from,to)).toEqual({body:'# 我的正文',kept:true});
  }
});
test('4 validation errors and inclusive byte/count boundaries', () => {
  expect(validateNewTask(TASK_TEMPLATES.feature,[],uploads)).toContain('请把第一行');
  expect(validateNewTask('中'.repeat(Math.ceil(MAX_BRIEF_BYTES/3)),[],uploads)).toContain('64KB');
  expect(validateNewTask('a'.repeat(MAX_BRIEF_BYTES),[],uploads)).toBeNull();
  expect(validateNewTask('# a',Array.from({length:UPLOAD_MAX_FILES},(_,i)=>String(i)),uploads)).toBeNull();
  expect(validateNewTask('# a',Array(UPLOAD_MAX_FILES+1).fill('a'),uploads)).toContain('最多');
  expect(validateNewTask('# a',['gone'],uploads)).toContain('不存在');
  expect(validateNewTask('# a',['a','duplicate'],uploads)).toContain('附件重名：a');
  expect(validateNewTask('# a',['a'],uploads)).toBeNull();
  expect(validateNewTask('',[],uploads)).toBeNull(); // Kernel owns empty/missing title errors.
});
test('5 action passes exact create fields and rejects malformed form', async () => {
  for (const intent of ['start','draft']) for (const kind of ['feature','bug']) {
    const write = vi.fn(async () => ({ status:200,body:{ok:true} })); const input=form(intent); input.set('kind',kind); input.append('attachments','a');
    await submit(input,{gateway:{write,read:vi.fn()},uploads});
    expect(write).toHaveBeenCalledWith({op:'task.create',params:{project_id:'P',kind,brief:'# 测试\r\n正文',draft:intent==='draft',attachments:['a'],after:[]}});
  }
  const write=vi.fn();
  for (const [key,value] of [['kind','other'],['intent','other']]) { const input=form();input.set(key,value); expect(await submit(input,{gateway:{write,read:vi.fn()}})).toHaveProperty('error'); }
  expect(write).not.toHaveBeenCalled();
});
test('6 gateway rejection text reaches page unchanged', async () => {
  for (const [status,error] of [[409,'任务卡正文不能为空'],[400,'已拒绝：错误'],[503,'内核未连接：离线'],[504,'请求超时']] as const) {
    expect(await submit(form(),{gateway:{read:vi.fn(),write:async()=>({status,body:{ok:false,error}})}})).toEqual({error});
    expect(render(true,true,error)).toContain(error);
  }
  expect(await submit(form(),{gateway:null})).toEqual({error:'内核未连接：服务未接入网关'});
});
test('7 SSR form fields, limits and write gate', () => {
  const html=render(); expect(html).toContain('type="radio"');expect(html).toContain('&lt;标题&gt;');expect(html).toContain('name="brief"');expect(html).toContain('type="file"');
  expect(html).toContain(`${UPLOAD_MAX_BYTES/1024/1024} MB`);expect(html).toContain(`${UPLOAD_MAX_FILES} 个`);
  for (const title of ['创建并开始','存为待开始']) {expect(html).toContain(title);expect(render(false)).toMatch(new RegExp(`<button[^>]*disabled=""[^>]*>${title}`));}
  expect(render(false)).toContain('连接中断，正在重连');
});
test('8 successful action redirects to project', async () => { const result=await submit(form(),{gateway:{read:vi.fn(),write:async()=>({status:200,body:{ok:true}})}});expect(result).toBeInstanceOf(Response);expect((result as Response).headers.get('Location')).toBe('/projects/P'); });
test('9 unknown project shows explicit message', () => {expect(render(true,false)).toContain('项目不存在');expect(render(true,false)).not.toContain('<form');});

test('10 依赖候选限同项目未结束任务排除自己且倒序', () => {
  const tasks = ['draft', 'queued', 'active', 'done', 'cancelled'].map((state, i) => ({ id: `T-${i+1}`, title: state, state: { state }, kind: 'feature', project_id: 'P' }));
  tasks.push({ ...tasks[0], id: 'T-10', project_id: 'other' });
  expect(dependencyCandidates(tasks, 'P', 'T-2').map(t => t.id)).toEqual(['T-3', 'T-1']);
});
test('11 新建提交带 after，空和上限合法超限拒绝', async () => {
  for (const size of [0, 1, 8, 9]) {
    const input = form(); for (let i = 1; i <= size; i++) input.append('after', `T-${i}`);
    const write = vi.fn(async () => ({ status: 200, body: { ok: true } }));
    const result = await submit(input, { gateway: { read: vi.fn(), write } });
    if (size > 8) { expect(result).toHaveProperty('error'); expect(write).not.toHaveBeenCalled(); }
    else expect(write).toHaveBeenCalledWith(expect.objectContaining({ params: expect.objectContaining({ after: input.getAll('after') }) }));
  }
});

test('新控件保留提交字段，附件视觉隐藏且可聚焦', () => {
  const text = render();
  expect(text).toContain('class="seg"');
  for (const name of ['kind', 'brief', 'intent']) expect(text).toContain('name="' + name + '"');
  expect(text).not.toContain('name="type"');
  expect(text).toContain('class="inp"');
  expect(text).toContain('class="visually-hidden" type="file"');
  expect(text).not.toContain('tabindex="-1"');
  const input = form(); input.append('attachments', 'file'); input.append('after', 'T-1');
  const write = vi.fn(async () => ({ status: 200, body: { ok: true } }));
  return submit(input, { gateway: { read: vi.fn(), write }, uploads }).then(() => {
    expect(write).toHaveBeenCalledWith(expect.objectContaining({ params: expect.objectContaining({ attachments: ['file'], after: ['T-1'] }) }));
  });
});
