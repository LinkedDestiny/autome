// Notifications — the design document's `.notif` stack, top right.
//
// This is where errors become visible. Requirement 5 ("错误是显示的，不是吞掉
// 的") plus the core's own habit of writing its messages in Chinese for users
// means the right thing to do with a rejected write is to put the core's
// sentence on screen unchanged, not to translate it, summarise it, or replace
// it with "操作失败".

import { h, icon, text } from './dom.js';

const ICONS = { success: 'check', info: 'bell', warning: 'warn', error: 'x' };
const DEFAULT_DURATION_MS = 4500;
// An error stays until dismissed. Everything else is an acknowledgement the
// user does not need to read twice; an error is the only thing they may need
// to copy out of the window.
const ERROR_DURATION_MS = 0;

function stack() {
  return document.getElementById('notif-stack');
}

/**
 * @param {'success'|'info'|'warning'|'error'} type
 * @param {string} title
 * @param {string} [description] shown verbatim — do not pre-format core text
 */
export function notify(type, title, description, duration) {
  const host = stack();
  if (!host) return null;

  const body = h('div.notif__body', [h('div.notif__title', { text: title })]);
  if (description) body.appendChild(h('div.notif__desc', { text: description }));

  const close = h('button.notif__close', { type: 'button', 'aria-label': '关闭' }, [text('×')]);
  const el = h(`div.notif.notif--${type}`, { role: 'status' }, [
    h('span.notif__ico', [icon(ICONS[type] || 'bell')]),
    body,
    close,
  ]);

  const dismiss = () => {
    if (el.classList.contains('leaving')) return;
    el.classList.add('leaving');
    setTimeout(() => el.remove(), 260);
  };
  close.addEventListener('click', (event) => {
    event.stopPropagation();
    dismiss();
  });

  host.appendChild(el);
  const ms = duration === undefined ? (type === 'error' ? ERROR_DURATION_MS : DEFAULT_DURATION_MS) : duration;
  if (ms > 0) setTimeout(dismiss, ms);
  return el;
}

export const success = (title, description) => notify('success', title, description);
export const info = (title, description) => notify('info', title, description);
export const warning = (title, description) => notify('warning', title, description);
export const error = (title, description) => notify('error', title, description);
