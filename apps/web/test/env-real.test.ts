import { afterEach, expect, test, vi } from 'vitest';
import { w9Kernel } from './fixtures/w9-replies.ts';
import { createAdmin } from '../server/admin.ts';
import { loader } from '../app/routes/env.tsx';
import { createLoadContext } from '../app/load-context.ts';
import type { EnvSnapshot, InstallRecipe } from '../app/components/env-model.ts';
import { recipe } from './fixtures/w9-env.ts';
const closes:(()=>Promise<void>)[]=[];
afterEach(async()=>{for(const close of closes.splice(0).reverse())await close();});
async function setup(){const s=await w9Kernel();closes.push(s.close);return s;}
test('1 真实环境全部组件逐字段一致，recipe 替身字段对齐',async()=>{
  const s=await setup();let snapshot:EnvSnapshot;
  await vi.waitFor(async()=>{snapshot=await s.core.call('env.get',{}) as EnvSnapshot;expect(snapshot.probed).toBe(true);expect(snapshot.probing).toBe(false);},{timeout:20000});
  const data=await loader({context:createLoadContext(s)(),params:{},request:new Request('http://localhost/env'),url:new URL('http://localhost/env'),pattern:'/env'});
  expect(data.snapshot).toEqual(snapshot!); expect(Object.keys(data.recipes)).toHaveLength(snapshot!.environment!.components.length);
  const real=await s.core.call('env.install_recipe',{component:'claude'}) as InstallRecipe;
  expect(Object.keys(real).sort()).toEqual(Object.keys(recipe).sort()); expect(Object.keys(real.recipe).sort()).toEqual(Object.keys(recipe.recipe).sort());
  expect(real.recipe.command).toBe(recipe.recipe.command);expect(real.login_command).toBe(recipe.login_command);
});
test('2 真实 recipe 决定终端命令且恶意 command 被拒',async()=>{
  const s=await setup(), terminal=vi.fn<(command: string) => Promise<void>>(async()=>{});createAdmin({...s,core:s.core,terminal}).register(s.gateway);
  for(const op of ['env.install','env.login']) expect((await s.gateway.write({op,params:{component:'git',command:'rm -rf ~'}})).status).toBe(400);expect(terminal).not.toHaveBeenCalled();
  const real=await s.core.call('env.install_recipe',{component:'claude'}) as InstallRecipe;
  const result=await s.gateway.write({op:'env.install',params:{component:'claude'}});
  if(real.prerequisite_ok){expect(result.status).toBe(200);expect(terminal).toHaveBeenLastCalledWith(real.recipe.command);}else{expect(result.status).toBe(409);expect(terminal).not.toHaveBeenCalled();}
  await s.gateway.write({op:'env.login',params:{component:'codex'}});expect(terminal).toHaveBeenLastCalledWith('codex login');
});
test('3 强制检测账本事件与 tick generation 推送',async()=>{
  const s=await setup(),publish=vi.fn();const admin=createAdmin({...s,publish});
  await vi.waitFor(async()=>{const env=await s.core.call('env.get',{}) as EnvSnapshot;expect(env.probed&&!env.probing).toBe(true);},{timeout:20000});
  const baseline=await s.core.tick();admin.onTick(baseline);expect(publish).toHaveBeenCalledTimes(1);
  expect((await s.gateway.write({op:'env.detect',params:{force:true}})).status).toBe(200);
  expect((await s.core.call('events.since',{after_seq:0,limit:1000}) as {events:{kind:string}[]}).events.some(e=>e.kind==='env.probing')).toBe(true);
  await vi.waitFor(async()=>{const report=await s.core.tick();admin.onTick(report);expect(Number(report.env_generation)).toBeGreaterThan(Number(baseline.env_generation));},{timeout:20000});
  expect(publish.mock.calls.slice(1).some(([message])=>message.type==='env.changed'&&message.generation>Number(baseline.env_generation))).toBe(true);
});
