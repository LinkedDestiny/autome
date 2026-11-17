import { Icon } from './icons.tsx';
import { TEST_IDS } from '../lib/testids.ts';
import { formatClock, useClock } from '../lib/clock.tsx';
import { useEffect, useRef, useState } from 'react';
import { Link } from 'react-router';
import { subscribeLiveMessages } from '../lib/live-client.ts';
import { createBellClient, EMPTY_BELL, type BellSnapshot } from './bell-client.ts';
import { resolveTarget, TARGETS } from './bell-targets.ts';
import { browserSystemNotifier, showSystemNotification, type SystemNotifier, type SystemPermission } from './bell-system.ts';


export function BellView({ snapshot, open, onToggle, onRead, onAll, targets = TARGETS, permission = 'unsupported', onRequest }: {
  snapshot: BellSnapshot; open: boolean; onToggle: () => void;
  onRead: (id: string) => Promise<void>; onAll: () => Promise<void>; targets?: typeof TARGETS;
  permission?: SystemPermission; onRequest?: () => void;
}) {
  const clock = useClock();
  return <div className="bell">
    <button type="button" aria-label={`通知，${snapshot.unread} 条未读`} aria-expanded={open} onClick={onToggle}
      className="bell-button" data-testid={TEST_IDS.bellButton}>
      <Icon name="bell" />
      {snapshot.unread > 0 && <span className="bell-badge">{snapshot.unread >= 100 ? '99+' : snapshot.unread}</span>}
    </button>
    {open && <section aria-label="通知列表" className="bell-panel" data-testid={TEST_IDS.bellPanel}>
      <header className="bell-header">
        <strong>通知</strong><button type="button" disabled={snapshot.unread === 0} onClick={() => { void onAll(); }}>全部已读</button>
      </header>
      {permission === 'default' && <button type="button" onClick={onRequest} className="bell-request">开启系统通知</button>}
      {permission === 'denied' && <p className="bell-message">系统通知已被浏览器拒绝，铃铛照常提醒</p>}
      {snapshot.error && <p role="alert" className="bell-error">{snapshot.error}</p>}
      {!snapshot.items.length && <p className="bell-empty">还没有通知</p>}
      <ul className="bell-list" data-scroll-region>
        {snapshot.items.map(item => {
          const target = resolveTarget(item, targets);
          return <li key={item.id} className="bell-item" data-testid={TEST_IDS.bellItem} data-task-id={item.task_id ?? undefined}>
            {!item.read_at && <span aria-label="未读" className="bell-unread">●</span>}
            {target.href ? <Link to={target.href} onClick={async event => {
              if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) { void onRead(item.id); return; }
              event.preventDefault();
              await onRead(item.id);
              window.location.assign(target.href!);
            }}>{item.title}</Link> : <span>{item.title}</span>}
            {item.body && <p className="bell-body">{item.body}</p>}
            {target.note && <p className="bell-note">{target.note}</p>}
            <div className="bell-footer">
              <time dateTime={item.created_at} className="bell-time">{formatClock(item.created_at, clock)}</time>
              {!item.read_at && <button type="button" onClick={() => { void onRead(item.id); }}>标为已读</button>}
            </div>
          </li>;
        })}
      </ul>
    </section>}
  </div>;
}

export function NotificationBell({ systemNotifier }: { systemNotifier?: SystemNotifier } = {}) {
  const notifier = useRef<SystemNotifier | null>(null);
  const [permission, setPermission] = useState<SystemPermission>('unsupported');
  const [snapshot, setSnapshot] = useState(EMPTY_BELL);
  const [open, setOpen] = useState(false);
  const client = useRef<ReturnType<typeof createBellClient> | null>(null);
  useEffect(() => {
    const system = systemNotifier ?? browserSystemNotifier(window);
    notifier.current = system;
    setPermission(system.permission());
    const controller = createBellClient({ fetch: window.fetch.bind(window), onChange: setSnapshot,
      onNotification: (message, items) => showSystemNotification(system, document.hidden, message, items),
    });
    client.current = controller;
    const unsubscribe = subscribeLiveMessages(controller.handleMessage);
    void controller.start();
    return () => { unsubscribe(); controller.stop(); client.current = null; };
  }, [systemNotifier]);
  return <BellView snapshot={snapshot} permission={permission} onRequest={() => {
    void notifier.current?.request().then(setPermission);
  }} open={open} onToggle={() => setOpen(value => !value)}
    onRead={async id => { await client.current?.markRead(id); }} onAll={async () => { await client.current?.markAll(); }} />;
}
