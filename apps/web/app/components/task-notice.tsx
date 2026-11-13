import { noticeView } from './task-notice-model.ts';
export function TaskNotice({ view }: { view: ReturnType<typeof noticeView> }) {
  if (!view.messages.length && !view.next) return null;
  return <aside className={`task-notice${view.failed ? ' is-error' : ''}`} role="status">
    {view.messages.map((message, index) => <p key={index}>{message}</p>)}
    {view.next && <p>{view.next}</p>}
  </aside>;
}
