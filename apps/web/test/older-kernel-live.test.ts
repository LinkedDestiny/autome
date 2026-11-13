import { expect, test, vi } from 'vitest';
import { mapCoreStatus } from '../server/eventbus-protocol.ts';
import { createLiveClient, type LiveSocket } from '../app/lib/live-client.ts';
import { liveBanner, writeGate, needsKernelReload } from '../app/lib/live.tsx';
import { createEventBus } from '../server/eventbus.ts';
const reason = '这个内核（旧）比推进过账本的内核（新）旧，已停止推进；请重新安装或设置 AUTOMED_BIN';
const core = { type: 'core' as const, status: 'outdated' as const, reason };
test('mapCoreStatus 保留 outdated 原因', () => { expect(mapCoreStatus({ state: 'outdated', reason, attempts: 0 })).toEqual(core); });
test('hello 从服务端到 live-client 保留原因', () => {
 const bus = createEventBus({call:async()=>({events:[]}),log:vi.fn()});
 const socket: LiveSocket = {send:vi.fn(),close:vi.fn(),onmessage:null,onclose:null,onerror:null};
 const onCore=vi.fn(); const client=createLiveClient({url:'ws://localhost/ws',createSocket:()=>socket,onCore,onEvents:vi.fn(),onReload:vi.fn(),onConnection:vi.fn()});
 bus.setCoreStatus({state:'outdated',reason,attempts:0});
 bus.connect({send:text=>socket.onmessage?.call(socket as unknown as WebSocket,{data:text} as MessageEvent),close:vi.fn()});
 expect(onCore).toHaveBeenCalledWith(core); client.stop(); bus.close();
});
test('横幅逐字原文', () => { expect(liveBanner({connection:'open',core})).toEqual({tone:'alert',text:reason}); });
test('写闸门拒绝并给原文', () => { expect(writeGate({connection:'open',core})).toEqual({allowed:false,reason}); });
test('outdated 不触发 reload', () => { expect(needsKernelReload({core,kernelOk:false})).toBe(false); });
