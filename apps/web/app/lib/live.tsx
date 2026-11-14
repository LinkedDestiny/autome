import { TEST_IDS } from './testids.ts';
import { createContext, useContext, useEffect, useRef, useState } from 'react';
import { useRevalidator } from 'react-router';
import type { CoreMessage } from '../../server/eventbus-protocol.ts';
import { createLiveClient, publishLiveMessage, setLiveSender, type ConnectionState } from './live-client.ts';
import { revalidateAll, revalidateForEvents } from './live-routes.ts';

export function useLiveEvents() {
  const revalidator = useRevalidator();
  const revalidate = useRef(revalidator.revalidate);
  useEffect(() => { revalidate.current = revalidator.revalidate; }, [revalidator.revalidate]);
  const [connection, setConnection] = useState<ConnectionState>('connecting');
  const [core, setCore] = useState<CoreMessage>();
  useEffect(() => {
    const url = new URL('/ws', window.location.href);
    url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
    const client = createLiveClient({
      url: url.href, onMessage: publishLiveMessage,
      createSocket: address => new window.WebSocket(address),
      onConnection: setConnection,
      onCore: setCore,
      onEvents: events => { void revalidateForEvents(events, () => revalidate.current()); },
      onReload: () => { void revalidateAll(() => revalidate.current()); },
    });
    setLiveSender(client.send);
    return () => { setLiveSender(undefined); client.stop(); };
  }, []);
  return { connection, core };
}

type LiveStatus = { connection: ConnectionState; core?: CoreMessage; kernelError?: string };
export function liveBanner({ connection, core, kernelError }: LiveStatus) {
  if (connection === 'reconnecting') return { tone: 'status' as const, text: '连接中断，正在重连' };
  if (core?.status === 'outdated') return { tone: 'alert' as const, text: core.reason ?? '' };
  const error = core?.status === 'offline' ? (core.reason ?? '内核已离线') : !core ? kernelError : undefined;
  return error === undefined ? null : { tone: 'alert' as const, text: `内核离线：${error}，写操作暂不可用` };
}
export function writeGate({ connection, core }: LiveStatus) {
  if (connection === 'connecting') return { allowed: false, reason: '正在连接服务' };
  if (connection === 'reconnecting') return { allowed: false, reason: '连接中断，正在重连' };
  if (core?.status === 'outdated') return { allowed: false, reason: core.reason ?? '' };
  if (core?.status === 'connected') return { allowed: true, reason: '' };
  return { allowed: false, reason: core ? `内核离线：${core.reason ?? '内核已离线'}` : '正在连接服务' };
}
const WriteGateContext = createContext(writeGate({ connection: 'connecting' }));
export const WriteGateProvider = WriteGateContext.Provider;
export function useWriteGate() { return useContext(WriteGateContext); }

type RecoveryStatus = { core?: CoreMessage; kernelOk: boolean };
export function needsKernelReload({ core, kernelOk }: RecoveryStatus) {
  return core?.status === 'connected' && kernelOk === false;
}
export function useKernelRecovery(status: RecoveryStatus) {
  const { revalidate } = useRevalidator();
  const needed = needsKernelReload(status);
  const previous = useRef(false);
  useEffect(() => {
    const rising = needed && !previous.current;
    previous.current = needed;
    if (rising) void revalidateAll(() => revalidate());
  }, [needed, revalidate]);
}
export function LiveBanner(status: LiveStatus) {
  const banner = liveBanner(status);
  return banner ? <div role={banner.tone} className="live-banner" data-testid={TEST_IDS.liveBanner} data-state={status.connection === 'reconnecting' ? 'reconnecting' : status.core?.status === 'outdated' ? 'outdated' : 'offline'}>{banner.text}</div> : null;
}
