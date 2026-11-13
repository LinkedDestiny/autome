import { afterEach, expect, test, vi } from 'vitest';
const fake = vi.hoisted(() => ({ report: {} as Record<string, unknown>, order: [] as string[] }));
vi.mock('../server/core/sidecar.ts', () => ({ AutomedSidecar: class {
 start() { fake.order.push('spawn'); }
 async stop() { fake.order.push('stop'); await Promise.resolve(); fake.order.push('reaped'); }
 isOutstanding() { return false; }
 failureReason() { return null; }
 async requestUntilReply(command: { method: string }) { return {outcome:{status:'ok',payload:command.method === 'scheduler.tick' ? fake.report : {read:true}}}; }
} }));
import { CoreSupervisor } from '../server/core/supervisor.ts';
import { createTicker } from '../server/ticker.ts';
const reason='这个内核（旧）比推进过账本的内核（新）旧，已停止推进；请重新安装或设置 AUTOMED_BIN';
function setup() { fake.report={skipped:true,older_kernel:{message:reason,kernel:'旧',ledger:'新'}}; fake.order=[]; const log=vi.fn(); return {c:new CoreSupervisor({home:'/unused',log}),log}; }
afterEach(()=>vi.useRealTimers());
test('握手直接 outdated',async()=>{const {c,log}=setup();await c.start();expect(c.status).toMatchObject({state:'outdated',reason});expect(log).toHaveBeenCalledExactlyOnceWith(`[内核] ${reason}`);await c.stop();});
test('outdated 读请求放行',async()=>{const {c}=setup();await c.start();expect(await c.call('dashboard.get')).toEqual({read:true});await c.stop();});
test('20 间隔只 tick 两次且进入日志一行',async()=>{
 vi.useFakeTimers();const {c,log}=setup();await c.start();const call=vi.fn(()=>c.tick());
 const ticker=createTicker({call,canTick:()=>c.canTick(),isSlow:()=>c.isSlow(),isBusy:()=>c.isTickBusy(),log});
 await vi.advanceTimersByTimeAsync(29999);expect(call).toHaveBeenCalledTimes(0);
 await vi.advanceTimersByTimeAsync(30001);expect(call).toHaveBeenCalledTimes(2);expect(log).toHaveBeenCalledTimes(1);ticker.stop();await c.stop();
});
test('恢复与再次拒绝只在变化时记录',async()=>{
 const {c,log}=setup();await c.start();fake.report={skipped:true,older_kernel:null};await c.tick();expect(c.isConnected()).toBe(true);await c.tick();
 expect(log.mock.calls.flat()).toEqual([`[内核] ${reason}`,'[内核] 已恢复推进']);
 fake.report={older_kernel:{message:reason+'变更',kernel:'旧',ledger:'更新'}};await c.tick();expect(c.isSlow()).toBe(true);await c.tick();expect(log).toHaveBeenCalledTimes(3);await c.stop();
});
test('restart 先 stop 并回收再 spawn',async()=>{const {c}=setup();await c.start();expect(await c.restart()).toEqual({restarted:true});expect(fake.order).toEqual(['spawn','stop','reaped','spawn']);await c.stop();});
