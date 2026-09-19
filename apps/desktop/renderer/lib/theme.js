// Appearance.
//
// Three values, and only one of them is a colour decision: `system` defers to
// macOS and tracks it live, so switching the OS appearance repaints the window
// without a restart. The stylesheet carries both palettes and is switched by
// `data-theme` on `<html>`; nothing here knows what a colour is.
//
// The preference itself lives in the core's global config, like every other
// piece of state. This module holds only the resolved answer, which is a
// function of the preference and the OS — not a second copy of it.

const DARK_QUERY = '(prefers-color-scheme: dark)';

let preference = 'system';
let mediaQuery = null;

/** The three values the core accepts. */
export const THEMES = Object.freeze(['system', 'light', 'dark']);

/** What is actually painted right now: `light` or `dark`, never `system`. */
export function resolved() {
  if (preference === 'dark' || preference === 'light') return preference;
  return systemPrefersDark() ? 'dark' : 'light';
}

function systemPrefersDark() {
  return typeof window !== 'undefined' && typeof window.matchMedia === 'function'
    ? window.matchMedia(DARK_QUERY).matches
    : false;
}

/**
 * Applies a preference. Safe to call with the same value repeatedly — the
 * router re-reads config on every refresh, and reassigning the attribute on
 * each tick would be a repaint the user can see.
 */
export function apply(next) {
  preference = THEMES.includes(next) ? next : 'system';
  const want = resolved();
  const root = document.documentElement;
  if (root.dataset.theme !== want) root.dataset.theme = want;
}

/**
 * Starts tracking the OS appearance. Only meaningful while the preference is
 * `system`, but the listener stays attached either way: it is one cheap
 * callback, and detaching and reattaching it on every preference change is
 * more state than the problem deserves.
 */
export function watchSystem() {
  if (mediaQuery || typeof window === 'undefined' || !window.matchMedia) return;
  mediaQuery = window.matchMedia(DARK_QUERY);
  mediaQuery.addEventListener('change', () => {
    if (preference === 'system') apply(preference);
  });
}
