import { createElement } from 'react';
import { renderToString } from 'react-dom/server';
import { readFileSync, readdirSync } from 'node:fs';
import { expect, it, vi } from 'vitest';
import { SessionLog, SessionLogView, SESSION_LOG_TEXT as t, applyScroll, isAtBottom } from '../app/components/session-log.tsx';
import { initialLogState, updateLog, type LogState } from '../app/components/session-log-model.ts';
import { setLiveSender } from '../app/lib/live-client.ts';
const session = { id: 'ses-1', label: '实现', round: 6, running: true, started_at: '' };
const initial = () => initialLogState(session.id);
const render = (state = initial()) => renderToString(createElement(SessionLogView, { session, state, open: true, onToggle: vi.fn(), onAction: vi.fn() }));
function large() {
  let state = updateLog(initial(), { type: 'open' }).state;
  for (let i = 0; i < 12; i++) state = updateLog(state, { type: 'chunk', message: { type: 'log.chunk', session_id: session.id, offset: state.next, next: state.next + 60000, text: '中'.repeat(20000) } }).state;
  return state;
}
it('SSR 收起显示轮次、标签、运行状态且不发送', () => {
  const send = vi.fn(); setLiveSender(send);
  try { for (const running of [true, false]) {
    const html = renderToString(createElement(SessionLog, { session: { ...session, running } }));
    expect(html).toContain('<details'); expect(html).toContain('6'); expect(html).toContain(session.label);
    expect(html).toContain(running ? t.running : t.ended); expect(html).not.toContain('<pre');
  } expect(send).not.toHaveBeenCalled(); } finally { setLiveSender(undefined); }
});
it('展开工具条、窗口分页、空态及 ended/rejected 状态', () => {
  const empty = render(); for (const label of [t.pause, t.jump, t.empty, 'class="session-log-text"']) expect(empty).toContain(label);
  for (const label of [t.earlier, t.top, t.later]) expect(empty).not.toContain(label);
  const state = large(); const tail = render(state);
  for (const label of [t.earlier, t.top, '前面还有约']) expect(tail).toContain(label);
  expect(render(updateLog(state, { type: 'top' }).state)).toContain(t.later);
  for (const phase of ['ended', 'rejected'] as const) expect(render({ ...state, phase, notice: '服务端中文原因' })).toContain('服务端中文原因');
  expect(render({ ...state, phase: 'rejected' })).toContain(t.retry);
});
it('滚动适配器只改变自己的位置；暂停追加不滚动、不抢焦点', () => {
  const element = { scrollTop: 30, scrollHeight: 800, clientHeight: 100, focus: vi.fn(), scrollIntoView: vi.fn() };
  applyScroll(element, 'bottom'); expect(element.scrollTop).toBe(800);
  applyScroll(element, 'top'); expect(element.scrollTop).toBe(0);
  element.scrollTop = 30; applyScroll(element); expect(element.scrollTop).toBe(30);
  const state = updateLog(large(), { type: 'pause' }).state;
  const next = updateLog(state, { type: 'chunk', message: { type: 'log.chunk', session_id: session.id, offset: state.next, next: state.next + 1, text: 'x' } });
  applyScroll(element, next.scroll); expect(element.scrollTop).toBe(30);
  expect(element.focus).not.toHaveBeenCalled(); expect(element.scrollIntoView).not.toHaveBeenCalled();
});
it('跳到最新滚到底；4px 容差两侧边界', () => {
  const element = { scrollTop: 695, scrollHeight: 800, clientHeight: 100 };
  expect(isAtBottom(element)).toBe(false); element.scrollTop = 696; expect(isAtBottom(element)).toBe(true);
  applyScroll(element, updateLog(large(), { type: 'jump' }).scroll); expect(element.scrollTop).toBe(800);
});
it('日志源码不抢焦点、不宣告实时区域', () => {
  for (const name of readdirSync(new URL('../app/components/', import.meta.url)).filter(name => name.startsWith('session-log'))) {
    expect(readFileSync(new URL(`../app/components/${name}`, import.meta.url), 'utf8')).not.toMatch(/\.focus\(|scrollIntoView|autoFocus|aria-live/);
  }
});
it('中文文案表逐条呈现，不泄漏英文界面词', () => {
  const state = large();
  const states: LogState[] = [initial(), state, updateLog(state, { type: 'top' }).state, { ...state, phase: 'subscribing' }, { ...state, phase: 'rejected', follow: false, unseen: true }];
  const html = states.map(render).join('');
  for (const key of ['pause','jump','unseen','earlier','top','later','empty','loading','retry','paused','following'] as const) expect(html).toContain(t[key]);
  expect(html).toContain(t.before(480000)); expect(html).toContain(t.after(480000));
  expect(html).not.toMatch(/Pause|Jump|Loading|latest|No log/);
});
