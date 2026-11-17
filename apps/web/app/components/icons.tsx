const paths = {
  home: <><path d="M4 11.5 12 4l8 7.5M6.5 10v9h11v-9"/></>,
  plus: <><path d="M12 5v14M5 12h14"/></>,
  gear: <><circle cx="12" cy="12" r="3.2"/><path d="M12 3v2.5M12 18.5V21M3 12h2.5M18.5 12H21M5.6 5.6l1.8 1.8M16.6 16.6l1.8 1.8M5.6 18.4l1.8-1.8M16.6 7.4l1.8-1.8"/></>,
  bell: <><path d="M6 16.5V11a6 6 0 0 1 12 0v5.5l1.8 2H4.2zM10 21h4"/></>,
  search: <><circle cx="10.5" cy="10.5" r="6"/><path d="m15.5 15.5 4.5 4.5"/></>,
  hand: <><path d="M8 12V6a1.5 1.5 0 0 1 3 0v5m0-1V4.5a1.5 1.5 0 0 1 3 0V10m0-3.5a1.5 1.5 0 0 1 3 0V14c0 4-2.5 7-6.5 7S5 18.5 4.5 15.5L4 13a1.5 1.5 0 0 1 3-.5l1 1.5"/></>,
  check: <><path d="m5 12.5 4.5 4.5L19 7.5"/></>,
  route: <><circle cx="6" cy="6" r="2.5"/><circle cx="18" cy="18" r="2.5"/><path d="M8.5 6H15a3 3 0 0 1 0 6H9a3 3 0 0 0 0 6h6.5"/></>,
  bag: <><path d="M5 9h14l-1 11H6zM9 9V7a3 3 0 0 1 6 0v2"/></>,
  leaf: <><path d="M5 19c0-8 5-13 14-14 0 9-5 14-13 14zM5 19l7-7"/></>,
};
export function Icon({ name }: { name: keyof typeof paths }) {
  return <svg className="ic" width="18" height="18" viewBox="0 0 24 24" aria-hidden="true" focusable="false">{paths[name]}</svg>;
}
