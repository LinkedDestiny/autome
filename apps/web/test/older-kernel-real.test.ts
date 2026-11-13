import { afterEach, expect, test, vi } from 'vitest';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { once } from 'node:events';
import WebSocket from 'ws';
import { AutomedSidecar } from '../server/core/sidecar.ts';
import { CoreSupervisor } from '../server/core/supervisor.ts';
import { startServer } from '../server/main.ts';
const binaryPath=resolve(import.meta.dirname,'../../../target/debug/automed');
const homes:string[]=[];
function home(){const h=mkdtempSync(join(tmpdir(),'older-web-'));homes.push(h);return h;}
afterEach(()=>{vi.restoreAllMocks();for(const h of homes.splice(0))rmSync(h,{recursive:true,force:true});});
test('真实旧内核 hello outdated、读 200、写 409',async()=>{
 const h=home();const seed=new CoreSupervisor({home:h,binaryPath});await seed.start();await seed.stop();
 const running=await startServer({port:0,mode:'development',home:h,binaryPath,kernelEnv:{AUTOME_TEST_BUILD_ID:'0000001-20200101T000000Z-000000000000'}});
 let ws:WebSocket|undefined;
 try {
  await vi.waitFor(()=>expect(running.core.status.state).toBe('outdated'));
  const reason=running.core.status.reason!;
  const address=running.server.address();if(!address||typeof address==='string')throw Error('address');
  const url=`http://127.0.0.1:${address.port}`;
  ws=new WebSocket(url.replace('http:','ws:')+'/ws',{origin:url});
  const [data]=await once(ws,'message');expect(JSON.parse(data.toString())).toMatchObject({type:'hello',core:'outdated',reason});
  const post=(channel:string,body:unknown)=>fetch(`${url}/api/${channel}`,{method:'POST',headers:{'content-type':'application/json',origin:url},body:JSON.stringify(body)});
  expect((await post('read',{method:'dashboard.get'})).status).toBe(200);
  const write=await post('write',{op:'task.create'});expect(write.status).toBe(409);expect(await write.json()).toMatchObject({code:'older_kernel',error:reason});
 }finally{if(ws){const closed=once(ws,'close');ws.close();await closed;}await running.close();}
});
test('真实 autome core 启动日志含路径与标识且参数传到子进程',async()=>{
 const h=home();const log=vi.spyOn(console,'error').mockImplementation(()=>{});
 const cli=resolve(import.meta.dirname,'../../../target/debug/autome');
 const running=await startServer({port:0,mode:'development',home:h,binaryPath:cli,binaryArgs:['core']});
 try{await vi.waitFor(()=>expect(running.core.isConnected()).toBe(true));expect(log.mock.calls.flat().join('\n')).toMatch(new RegExp(`内核：${cli} core（\\d{7}-\\d{8}T\\d{6}Z-[0-9a-f]{12}，来源：env）`));}
 finally{await running.close();}
});
test('真实 stdio 旧内核可读、拒写、接管后恢复',async()=>{
 const h=home();const seed=new CoreSupervisor({home:h,binaryPath});await seed.start();await seed.stop();
 const env={AUTOME_TEST_BUILD_ID:'0000001-20200101T000000Z-000000000000'};
 const log=vi.fn();const old=new CoreSupervisor({home:h,binaryPath,env,log});
 const adopt=new CoreSupervisor({home:h,binaryPath,env:{...env,AUTOME_ALLOW_OLDER_KERNEL:'1'}});
 try {
  await old.start();expect(old.status.state).toBe('outdated');const reason=old.status.reason;
  expect(await old.call('dashboard.get')).toBeTruthy();
  await expect(old.call('task.create')).rejects.toMatchObject({code:'older_kernel',message:reason});
  await old.tick();expect(log.mock.calls.flat().filter(line=>line===`[内核] ${reason}`)).toHaveLength(1);
  await adopt.start();await adopt.stop();await old.tick();expect(old.isConnected()).toBe(true);
  expect(log).toHaveBeenCalledWith('[内核] 已恢复推进');
 }finally{await old.stop();await adopt.stop();}
});

test('强制停止也等待子进程退出后才返回',async()=>{
 const sidecar=new AutomedSidecar({binaryPath:process.execPath,args:['-e','process.stdin.resume(); setInterval(() => {}, 1000)'],env:{AUTOME_HOME:home()}});
 sidecar.start();const child=sidecar._child!;
 await sidecar.stop(10);expect(child.exitCode !== null || child.signalCode !== null).toBe(true);
});
