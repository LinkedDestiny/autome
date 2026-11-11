import { useEffect, useRef, useState } from 'react';
import { useRevalidator } from 'react-router';
import type { CoreMessage } from '../../server/eventbus-protocol.ts';
import { createLiveClient, type ConnectionState } from './live-client.ts';
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
      url: url.href,
      createSocket: address => new window.WebSocket(address),
      onConnection: setConnection,
      onCore: setCore,
      onEvents: events => { void revalidateForEvents(events, () => revalidate.current()); },
      onReload: () => { void revalidateAll(() => revalidate.current()); },
    });
    return () => client.stop();
  }, []);
  return { connection, core };
}

export function LiveBanner({ connection }: { connection: ConnectionState }) {
  return connection === 'reconnecting'
    ? <div role="status" style={{ position: 'sticky', top: 0, zIndex: 1, padding: '8px 12px', color: '#713f12', background: '#fef3c7' }}>连接中断，正在重连</div>
    : null;
}
