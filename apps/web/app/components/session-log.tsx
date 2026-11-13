import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { createSessionLogController } from './session-log-controller.ts';
import { initialLogState, viewOf, type LogAction, type LogState, type LogUpdate } from './session-log-model.ts';

export type LogSession = { id: string; label: string; round: number; running: boolean; started_at: string };
export const SESSION_LOG_TEXT = {
  running: '正在运行', ended: '已结束', pause: '暂停滚动', jump: '跳到最新', unseen: '有新内容',
  earlier: '查看更早内容', top: '跳到开头', later: '查看后面内容', empty: '还没有日志输出',
  loading: '正在加载日志', retry: '重新加载日志', paused: '自动滚动暂停中', following: '正在跟随日志',
  before: (bytes: number) => `前面还有约 ${Math.ceil(bytes / 1024)} KB 未显示`,
  after: (bytes: number) => `后面还有约 ${Math.ceil(bytes / 1024)} KB 未显示`,
};
type ScrollElement = { scrollTop: number; scrollHeight: number; clientHeight: number };
export function applyScroll(element: ScrollElement, scroll?: LogUpdate['scroll']) {
  if (scroll === 'bottom') element.scrollTop = element.scrollHeight;
  if (scroll === 'top') element.scrollTop = 0;
}
export function isAtBottom(element: ScrollElement) { return element.scrollHeight - element.clientHeight - element.scrollTop <= 4; }

export function SessionLogView({ session, state, open, onToggle, onAction, textRef }: {
  session: LogSession; state: LogState; open: boolean; onToggle: (open: boolean) => void;
  onAction: (action: LogAction) => void; textRef?: React.Ref<HTMLPreElement>;
}) {
  const view = viewOf(state), t = SESSION_LOG_TEXT;
  return <details className="session-log" open={open} onToggle={event => onToggle(event.currentTarget.open)}>
    <summary>第 {session.round} 轮 · {session.label} · {session.running ? t.running : t.ended}</summary>
    {open && <div className="session-log-body">
      <div className="session-log-toolbar">
        <button type="button" disabled={!state.follow} onClick={() => onAction({ type: 'pause' })}>{t.pause}</button>
        <button type="button" onClick={() => onAction({ type: 'jump' })}>{t.jump}{state.unseen ? `（${t.unseen}）` : ''}</button>
      </div>
      {view.hiddenBefore > 0 && <div className="session-log-toolbar"><span>{t.before(view.hiddenBefore)}</span>
        <button type="button" onClick={() => onAction({ type: 'earlier' })}>{t.earlier}</button>
        <button type="button" onClick={() => onAction({ type: 'top' })}>{t.top}</button></div>}
      <pre className="session-log-text" ref={textRef} tabIndex={0} onScroll={event => onAction({ type: 'scrolled', atBottom: isAtBottom(event.currentTarget) })}>{view.text || t.empty}</pre>
      {view.hiddenAfter > 0 && <div className="session-log-toolbar"><span>{t.after(view.hiddenAfter)}</span>
        <button type="button" onClick={() => onAction({ type: 'later' })}>{t.later}</button></div>}
      <p className="session-log-status">{state.notice || (state.phase === 'subscribing' ? t.loading : state.follow ? t.following : t.paused)}</p>
      {state.phase === 'rejected' && <button type="button" onClick={() => onAction({ type: 'retry' })}>{t.retry}</button>}
    </div>}
  </details>;
}

export function SessionLog({ session }: { session: LogSession }) {
  const [open, setOpen] = useState(false);
  const [update, setUpdate] = useState<LogUpdate>(() => ({ state: initialLogState(session.id) }));
  const controller = useRef<ReturnType<typeof createSessionLogController> | null>(null);
  const text = useRef<HTMLPreElement>(null);
  const scroll = useRef<LogUpdate['scroll']>(undefined);
  useEffect(() => {
    if (!open) return;
    const current = createSessionLogController({ sessionId: session.id, onChange: value => {
      if (value.scroll) scroll.current = value.scroll;
      setUpdate(value);
    } });
    controller.current = current;
    current.open();
    return () => { current.close(); controller.current = null; scroll.current = undefined; };
  }, [open, session.id]);
  useLayoutEffect(() => {
    if (text.current) applyScroll(text.current, scroll.current);
    scroll.current = undefined;
  }, [update]);
  return <SessionLogView session={session} state={update.state} open={open} onToggle={setOpen}
    onAction={action => controller.current?.dispatch(action)} textRef={text} />;
}
