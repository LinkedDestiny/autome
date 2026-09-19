// Drawers and modals — the design document's `.drawer` / `.modal` layers.
//
// One drawer element and one modal element exist in index.html and are filled
// on demand, rather than one hidden element per panel as in the mock. The
// mock could afford eight static drawers because its content was fixed; here
// every panel's content comes from a read, and a stale drawer that is merely
// hidden is a drawer showing last week's merge statistics the next time it
// opens.

import { clear, h, text } from './dom.js';
import { resetWriteControls } from './api.js';

let current = null;
let onCloseHook = null;

function windowEl() {
  return document.getElementById('window');
}

export function close() {
  if (!current) return;
  const drawer = document.getElementById('drawer');
  const drawerMask = document.getElementById('drawer-mask');
  const modalMask = document.getElementById('modal-mask');
  drawer.classList.remove('open');
  drawerMask.classList.remove('open');
  modalMask.classList.remove('open');
  const win = windowEl();
  if (win) win.classList.remove('recessed');
  clear(drawer);
  clear(document.getElementById('modal'));
  current = null;
  const hook = onCloseHook;
  onCloseHook = null;
  if (typeof hook === 'function') hook();
}

/**
 * Opens the right-hand drawer.
 *
 * @param {object} spec
 * @param {string} spec.title
 * @param {Node|Node[]} spec.body
 * @param {Node[]} [spec.footer] buttons; the caller wires their own close
 * @param {Function} [spec.onClose]
 */
export function openDrawer({ title, body, footer, onClose }) {
  close();
  const drawer = document.getElementById('drawer');
  clear(drawer);
  drawer.appendChild(
    h('div.drawer__head', [
      h('h2.drawer__title', { text: title }),
      h('button.modal__close', { type: 'button', 'aria-label': '关闭', onClick: close }, [text('×')]),
    ])
  );
  const bodyEl = h('div.drawer__body');
  appendAll(bodyEl, body);
  drawer.appendChild(bodyEl);
  if (footer && footer.length) drawer.appendChild(h('div.drawer__foot', footer));

  document.getElementById('drawer-mask').classList.add('open');
  drawer.classList.add('open');
  const win = windowEl();
  if (win) win.classList.add('recessed');
  current = 'drawer';
  onCloseHook = onClose || null;
  return drawer;
}

/**
 * Opens the centred modal. `lead` is the design's 20px sentence under the
 * title; `body` is the detail block.
 */
export function openModal({ title, lead, body, footer, onClose }) {
  close();
  const modal = document.getElementById('modal');
  clear(modal);
  const clip = h('div.modal__clip', [
    h('div.modal__head', [
      h('h2.modal__title', { text: title }),
      h('button.modal__close', { type: 'button', 'aria-label': '关闭', onClick: close }, [text('×')]),
    ]),
  ]);
  if (lead) clip.appendChild(h('p.modal__lead', { text: lead }));
  const bodyEl = h('div.modal__body');
  appendAll(bodyEl, body);
  clip.appendChild(bodyEl);
  if (footer && footer.length) clip.appendChild(h('div.modal__foot', footer));
  modal.appendChild(clip);

  document.getElementById('modal-mask').classList.add('open');
  current = 'modal';
  onCloseHook = onClose || null;
  return modal;
}

function appendAll(host, content) {
  if (!content) return;
  for (const node of Array.isArray(content) ? content : [content]) {
    if (node) host.appendChild(node);
  }
}

/** The plain "取消"/"稍后" button every panel ends up needing. */
export function closeButton(label) {
  return h('button.modal__btn', { type: 'button', onClick: close }, [text(label || '取消')]);
}

/** Wires the mask clicks and Escape once, at startup. */
export function installOverlayHandlers() {
  const drawerMask = document.getElementById('drawer-mask');
  const modalMask = document.getElementById('modal-mask');
  if (drawerMask) drawerMask.addEventListener('click', close);
  if (modalMask) {
    modalMask.addEventListener('click', (event) => {
      // Only the backdrop closes; a click inside the blob must not.
      if (event.target === modalMask) close();
    });
  }
  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') close();
  });
}

/** Called by the router: an overlay must not survive a screen change. */
export function closeForNavigation() {
  close();
  resetWriteControls();
}
