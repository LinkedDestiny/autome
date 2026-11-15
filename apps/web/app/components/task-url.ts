/** Change only owned query keys; callers keep independent tab/document/window state. */
export function withParams(search: URLSearchParams, { set = {}, remove = [] }: { set?: Record<string, string>; remove?: string[] }) {
  const next = new URLSearchParams(search);
  for (const key of remove) next.delete(key);
  for (const [key, value] of Object.entries(set)) next.set(key, value);
  return next;
}
export function pinTimeline(search: URLSearchParams, from: string | null) {
  return from && !search.has('from') && search.get('timeline') !== 'all' ? withParams(search, { set: { from } }) : search;
}
