// The only module that touches `window.autome`.
//
// Three jobs, and they are all about not lying to the user:
//
//  1. **Connection is observed, never assumed.** The renderer starts
//     `connected = false` and only flips true when a read actually returns.
//     `onCoreStatus` fires when the sidecar dies or restarts, but Main sends
//     nothing at all on a healthy start — so treating "no bad news" as good
//     news would paint a fully-live UI over a core that never came up.
//     Requirement: never render unverified state as verified.
//
//  2. **A failed read means the screen does not know.** Reads that reject
//     mark the core unreachable, which disables every registered write
//     control and shows the banner. The alternative — leaving the last good
//     render on screen with live-looking buttons — offers the user a merge
//     that cannot happen.
//
//  3. **A failed write is shown, not swallowed.** `attempt()` is the single
//     path for every mutation; it catches the rejection and hands the core's
//     own sentence to a notification unchanged (see lib/notify.js).

import { notify } from './notify.js';

const bridge = () => (typeof window !== 'undefined' ? window.autome : undefined);

let connected = false;
const connectionListeners = new Set();
// Write controls register themselves so a connection change can disable them
// all at once. A WeakSet would not be iterable; entries are removed when the
// screen that owns them is replaced.
let writeControls = [];

export function isConnected() {
  return connected;
}

export function onConnectionChange(listener) {
  connectionListeners.add(listener);
  return () => connectionListeners.delete(listener);
}

export function setConnected(next) {
  const value = Boolean(next);
  if (value === connected) return;
  connected = value;
  applyConnectionToControls();
  for (const listener of connectionListeners) listener(connected);
}

/**
 * Registers a control that performs a write, so it can be disabled while the
 * core is unreachable. Returns the element, so call sites read as
 * `parent.appendChild(registerWrite(button))`.
 */
export function registerWrite(el) {
  writeControls.push(el);
  el.classList.add('write');
  applyOne(el);
  return el;
}

/** Called by the router before a screen is replaced. */
export function resetWriteControls() {
  writeControls = [];
}

function applyOne(el) {
  if (connected) {
    el.removeAttribute('aria-disabled');
    el.removeAttribute('title');
    if ('disabled' in el) el.disabled = false;
  } else {
    el.setAttribute('aria-disabled', 'true');
    if ('disabled' in el) el.disabled = true;
    el.title = '内核不可达，写操作已禁用';
  }
}

function applyConnectionToControls() {
  writeControls = writeControls.filter((el) => el.isConnected !== false);
  for (const el of writeControls) applyOne(el);
}

/**
 * Electron wraps a rejected `ipcRenderer.invoke` as
 * `Error invoking remote method 'autome:write': Error: <message>`. The core's
 * message is the part the user is meant to read; the transport frame around
 * it is noise that would push the real sentence off the end of a
 * notification. Stripping the frame is not editing the message.
 */
export function coreMessage(err) {
  const raw = err && err.message ? String(err.message) : String(err);
  const match = /Error invoking remote method '[^']*':\s*(?:Error:\s*)?([\s\S]*)$/.exec(raw);
  return (match ? match[1] : raw).trim() || '内核没有给出原因';
}

/**
 * Runs one read. Resolves with the payload, or rejects after marking the core
 * unreachable — callers that want a screen to survive a dead core should use
 * `readOr`.
 */
export async function read(name, ...args) {
  const api = bridge();
  if (!api || !api.read || typeof api.read[name] !== 'function') {
    setConnected(false);
    throw new Error(`preload 没有暴露 read.${name}`);
  }
  try {
    const payload = await api.read[name](...args);
    setConnected(true);
    return payload;
  } catch (err) {
    setConnected(false);
    throw err;
  }
}

/** A read whose failure is expected to be survivable: returns `fallback`. */
export async function readOr(fallback, name, ...args) {
  try {
    return await read(name, ...args);
  } catch {
    return fallback;
  }
}

/**
 * The single path for every mutation.
 *
 * On success: an optional success notification, then `onDone` (normally the
 * router's refresh). On failure: an error notification carrying the core's
 * message verbatim. Never throws — a caller that wants to branch can read the
 * returned `{ ok }`.
 */
export async function attempt({ label, run, success: successText, onDone }) {
  const api = bridge();
  if (!api || !api.write) {
    notify('error', label, '内核不可达，写操作已禁用');
    return { ok: false, error: new Error('内核不可达') };
  }
  try {
    const result = await run(api.write);
    setConnected(true);
    if (successText !== false) notify('success', label, successText || undefined);
    if (typeof onDone === 'function') await onDone(result);
    return { ok: true, result };
  } catch (err) {
    // The core writes its refusals for users, in Chinese. Show the sentence.
    notify('error', label, coreMessage(err));
    return { ok: false, error: err };
  }
}

/** Subscribes to core-pushed change notices. The payload is never trusted as
 *  state — it only says that something moved, so the screen re-reads. */
export function onEvent(handler) {
  const api = bridge();
  if (!api || typeof api.onEvent !== 'function') return () => {};
  return api.onEvent(handler);
}

export function onCoreStatus(handler) {
  const api = bridge();
  if (!api || typeof api.onCoreStatus !== 'function') return () => {};
  return api.onCoreStatus(handler);
}
