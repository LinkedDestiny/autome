import { useEffect, useRef, useState } from 'react';
import { Link } from 'react-router';
import { subscribeLiveMessages } from '../lib/live-client.ts';
import { createBellClient, EMPTY_BELL, type BellSnapshot } from './bell-client.ts';
import { resolveTarget, TARGETS } from './bell-targets.ts';

function localTime(value: string) {
  const date = new Date(value);
  const pad = (part: number) => String(part).padStart(2, '0');
  return `${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

export function BellView({ snapshot, open, onToggle, onRead, onAll, targets = TARGETS }: {
  snapshot: BellSnapshot; open: boolean; onToggle: () => void;
  onRead: (id: string) => Promise<void>; onAll: () => Promise<void>; targets?: typeof TARGETS;
}) {
  return <div style={{ position: 'fixed', top: 16, right: 20, zIndex: 10, fontFamily: 'system-ui, sans-serif', color: '#1f2937' }}>
    <button type="button" aria-label={`通知，${snapshot.unread} 条未读`} aria-expanded={open} onClick={onToggle}
      style={{ padding: '8px 12px', background: '#fff', border: '1px solid #d1d5db', borderRadius: 6, cursor: 'pointer' }}>
      <svg aria-hidden="true" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6"><path d="M18 8a6 6 0 0 0-12 0c0 7-3 7-3 9h18c0-2-3-2-3-9M10 21h4" /></svg>
      {snapshot.unread > 0 && <span style={{ marginLeft: 6, color: '#b91c1c', fontWeight: 600 }}>{snapshot.unread >= 100 ? '99+' : snapshot.unread}</span>}
    </button>
    {open && <section aria-label="通知列表" style={{ position: 'absolute', right: 0, marginTop: 8, width: 'min(380px, calc(100vw - 40px))', maxHeight: '60vh', overflowY: 'auto', background: '#fff', border: '1px solid #d1d5db', borderRadius: 6, boxShadow: '0 8px 24px #00000014' }}>
      <header style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', padding: 16 }}>
        <strong>通知</strong><button type="button" disabled={snapshot.unread === 0} onClick={() => { void onAll(); }}>全部已读</button>
      </header>
      {snapshot.error && <p role="alert" style={{ padding: '0 16px', color: '#b91c1c' }}>{snapshot.error}</p>}
      {!snapshot.items.length && <p style={{ padding: '0 16px 16px' }}>还没有通知</p>}
      <ul style={{ listStyle: 'none', margin: 0, padding: 0 }}>
        {snapshot.items.map(item => {
          const target = resolveTarget(item, targets);
          return <li key={item.id} style={{ padding: 16, borderTop: '1px solid #e5e7eb', overflowWrap: 'anywhere' }}>
            {!item.read_at && <span aria-label="未读" style={{ color: '#2563eb', marginRight: 6 }}>●</span>}
            {target.href ? <Link to={target.href} onClick={async event => {
              if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) { void onRead(item.id); return; }
              event.preventDefault();
              await onRead(item.id);
              window.location.assign(target.href!);
            }}>{item.title}</Link> : <span>{item.title}</span>}
            {item.body && <p style={{ margin: '8px 0', fontSize: 14 }}>{item.body}</p>}
            {target.note && <p style={{ color: '#6b7280', fontSize: 12 }}>{target.note}</p>}
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 12, marginTop: 8 }}>
              <time dateTime={item.created_at} style={{ color: '#6b7280', fontSize: 12 }}>{localTime(item.created_at)}</time>
              {!item.read_at && <button type="button" onClick={() => { void onRead(item.id); }}>标为已读</button>}
            </div>
          </li>;
        })}
      </ul>
    </section>}
  </div>;
}

export function NotificationBell() {
  const [snapshot, setSnapshot] = useState(EMPTY_BELL);
  const [open, setOpen] = useState(false);
  const client = useRef<ReturnType<typeof createBellClient> | null>(null);
  useEffect(() => {
    const controller = createBellClient({ fetch: window.fetch.bind(window), onChange: setSnapshot });
    client.current = controller;
    const unsubscribe = subscribeLiveMessages(controller.handleMessage);
    void controller.start();
    return () => { unsubscribe(); controller.stop(); client.current = null; };
  }, []);
  return <BellView snapshot={snapshot} open={open} onToggle={() => setOpen(value => !value)}
    onRead={async id => { await client.current?.markRead(id); }} onAll={async () => { await client.current?.markAll(); }} />;
}
