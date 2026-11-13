import { execFile } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { promisify } from 'node:util';
import { WebSocket } from 'ws';
import { expect, test, vi } from 'vitest';
import { startServer } from '../server/main.ts';

async function scenario(check: (url: string, stop: () => Promise<void>, taskId: string, projectId: string) => Promise<void>) {
  const root=mkdtempSync(join(tmpdir(),'w6-listen-')),home=join(root,'home'),wt=resolve(import.meta.dirname,'../../..');
  const command=promisify(execFile);let running:Awaited<ReturnType<typeof startServer>>|undefined;let taskId:string|undefined;const started=performance.now();
  const cli=(...args:string[])=>command(join(wt,'target/debug/autome'),args,{cwd:join(root,'repo'),env:{...process.env,AUTOME_HOME:home},timeout:30_000});
  try {
    await command('sh',[join(wt,'apps/web/test/fixtures/w6-project.sh'),root,wt],{timeout:60_000});
    running=await startServer({port:0,mode:'development',home,binaryPath:join(root,'core-wrapper')});
    await vi.waitFor(async()=>expect((await running!.gateway.read({method:'project.list'})).status).toBe(200),{timeout:20_000});
    const result=await running.gateway.read({method:'project.list'});const projectId=(result.body.payload as {projects:{project:{id:string}}[]}).projects[0].project.id;
    const created=await running.gateway.write({op:'task.create',params:{project_id:projectId,brief:'# 实时任务\n正文',kind:'feature',draft:false}});expect(created.body.ok).toBe(true);
    const project=await running.gateway.read({method:'project.get',params:{project_id:projectId}});taskId=(project.body.payload as {tasks:{id:string}[]}).tasks[0].id;
    const address=running.server.address();if(!address||typeof address==='string')throw Error('no address');
    await check(`http://127.0.0.1:${address.port}`,async()=>{await cli('stop',taskId!);},taskId,projectId);
  } finally {if(taskId)await cli('stop',taskId).catch(()=>{});await running?.close();rmSync(root,{recursive:true,force:true});process.stdout.write(`W6 listening setup/tests/cleanup ${Math.ceil(performance.now()-started)} ms\n`);}
}
test('1 external stop reaches websocket within 4500 ms',()=>scenario(async(url,stop,taskId)=>{
 const ws=new WebSocket(url.replace('http:','ws:')+'/ws',{origin:url});
 try {
 await new Promise<void>((yes,no)=>{const timer=setTimeout(()=>no(Error('hello timeout')),5000);ws.on('message',raw=>{if(JSON.parse(String(raw)).type==='hello'){clearTimeout(timer);yes();}});ws.once('error',e=>{clearTimeout(timer);no(e);});});
 const start=performance.now();
 const received=new Promise<void>((yes,no)=>{const timer=setTimeout(()=>no(Error('task.updated timeout 4500 ms')),4500);ws.on('message',raw=>{const m=JSON.parse(String(raw));if(m.type==='event'&&m.kind==='task.updated'&&m.subject===taskId&&m.payload?.state?.state==='stopped'){clearTimeout(timer);yes();}});});
 await Promise.all([stop(),received]);const elapsed=performance.now()-start;process.stdout.write(`W6 stop to websocket ${Math.ceil(elapsed)} ms\n`);expect(elapsed).toBeLessThanOrEqual(4500);
 }finally{ws.terminate();}
}),120_000);
test('2 new SSR request after stop places card in needs-you column',()=>scenario(async(url,stop,taskId,projectId)=>{
 await stop();const response=await fetch(url+'/projects/'+projectId);expect(response.status).toBe(200);const html=await response.text();const column=html.split('aria-labelledby="board-needs_you"')[1]?.split('</section>')[0];expect(column).toContain(taskId);expect(column).toContain('实时任务');expect(html.split('aria-labelledby="board-running"')[1]?.split('</section>')[0]).not.toContain(`>${taskId}<`);
}),120_000);
