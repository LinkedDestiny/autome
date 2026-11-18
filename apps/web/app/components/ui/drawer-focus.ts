// Query on each key press: saving and editing can enable or remove controls.
export function manageDrawerFocus(container: HTMLElement) {
  const previous = document.activeElement;
  const controls = () => Array.from(container.querySelectorAll<HTMLElement>('button, input, select, textarea, a[href], [tabindex]'))
    .filter(element => element.tabIndex >= 0 && !element.matches(':disabled') && !element.closest('[hidden], [inert]') && element.getClientRects().length > 0);
  (controls()[0] ?? container).focus();
  const keydown = (event: KeyboardEvent) => {
    if (event.key !== 'Tab') return;
    const items = controls(), first = items[0], last = items[items.length - 1];
    if (!first || !items.includes(document.activeElement as HTMLElement)) {
      event.preventDefault();
      (event.shiftKey ? last ?? container : first ?? container).focus();
    } else if (event.shiftKey && document.activeElement === first) {
      event.preventDefault(); last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault(); first.focus();
    }
  };
  container.addEventListener('keydown', keydown);
  return () => {
    container.removeEventListener('keydown', keydown);
    if (previous instanceof HTMLElement && previous.isConnected) previous.focus();
  };
}
