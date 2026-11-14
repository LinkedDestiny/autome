import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { Readable } from 'node:stream';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { afterAll, beforeAll, expect, test } from 'vitest';
import { CoreSupervisor } from '../server/core/supervisor.ts';
import { createGateway } from '../server/gateway.ts';
import { createUploads } from '../server/uploads.ts';
import { createNewTask } from '../app/components/task-new-action.ts';
import { boardColumns, loadBoard } from '../app/components/board-data.ts';
const wt=resolve(import.meta.dirname,'../../..'), root=mkdtempSync(join(tmpdir(),'w6-new-')), home=join(root,'home');
const core=new CoreSupervisor({home,binaryPath:join(root,'core-wrapper')});
const uploads=createUploads(home), gateway=createGateway({core,home,uploads});
const command=promisify(execFile); let projectId:string; const activeIds:string[]=[]; const started=performance.now();
beforeAll(async()=>{await command('sh',[join(wt,'apps/web/test/fixtures/w6-project.sh'),root,wt],{timeout:60_000});await core.start({timeoutMs:30_000});const result=await gateway.read({method:'project.list'});projectId=(result.body.payload as {projects:{project:{id:string}}[]}).projects[0].project.id;},120_000);
afterAll(async()=>{try {for(const id of activeIds) await command(join(wt,'target/debug/autome'),['stop',id],{cwd:join(root,'repo'),env:{...process.env,AUTOME_HOME:home},timeout:30_000});}finally{await core.stop();rmSync(root,{recursive:true,force:true,maxRetries:5,retryDelay:100});}process.stdout.write(`W6 new real setup/tests/cleanup ${Math.ceil(performance.now()-started)} ms\n`);});
function form(intent:string,brief:string,id?:string){const data=new FormData();data.set('kind','feature');data.set('intent',intent);data.set('brief',brief);if(id)data.append('attachments',id);return data;}
test('1 start creates running card and copies original attachment bytes',async()=>{
  const bytes=Buffer.from('附件逐字核对'); const info=await uploads.receive({ stream: Readable.from([bytes]), name: encodeURIComponent('参考.txt') });
  expect(await createNewTask(form('start','# 开始任务\n正文',info.id),projectId,{gateway,uploads})).toBeInstanceOf(Response);
  const board=await loadBoard({gateway},projectId);if('error' in board)throw Error(board.error);
  const task=board.tasks.find(t=>t.title==='开始任务')!;activeIds.push(task.id);
  expect(boardColumns(board.tasks,board.now).find(c=>c.id==='running')!.tasks.map(t=>t.id)).toContain(task.id);
  const detail=(await gateway.read({method:'task.get',params:{task_id:task.id}})).body.payload as {worktree:string;task:{doc_dir:string}};
  expect(readFileSync(join(detail.worktree,detail.task.doc_dir,'attachments','参考.txt'))).toEqual(bytes);
},120_000);
test('2 draft stays todo and retains its staged attachment',async()=>{
  const bytes=Buffer.from('稍后开始');const info=await uploads.receive({ stream: Readable.from([bytes]), name: encodeURIComponent('稍后.txt') });
  expect(await createNewTask(form('draft','# 草稿任务\n正文',info.id),projectId,{gateway,uploads})).toBeInstanceOf(Response);
  const board=await loadBoard({gateway},projectId);if('error' in board)throw Error(board.error);
  const task=board.tasks.find(t=>t.title==='草稿任务')!; if(task.state.state==='active')activeIds.push(task.id);
  expect(task.state.state).toBe('draft');expect(boardColumns(board.tasks,board.now)[0].tasks.map(t=>t.id)).toContain(task.id);
  expect(uploads.resolve(info.id)).not.toBeNull();expect(readFileSync(uploads.resolve(info.id)!)).toEqual(bytes);
},120_000);
test('3 missing and empty titles preserve real kernel rejection',async()=>{
  expect(await createNewTask(form('draft','###   '),projectId,{gateway,uploads})).toMatchObject({error:expect.stringContaining('任务卡正文缺少标题')});
  expect(await createNewTask(form('draft',''),projectId,{gateway,uploads})).toMatchObject({error:expect.stringContaining('任务卡正文不能为空')});
},120_000);
