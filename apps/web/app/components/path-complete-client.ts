import { ADMIN } from '../lib/admin-labels.ts';
export interface Suggestions { entries: { name: string; path: string }[]; truncated: boolean; error?: string }
// Each input invalidates the previous timer and response, even if fetch ignores abort.
export function pathCompleteClient(fetcher: typeof fetch, update: (value: Suggestions) => void) {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let controller: AbortController | undefined;
  let generation = 0;
  const cancel = () => { generation++; clearTimeout(timer); controller?.abort(); };
  return {
    input(input: string) {
      cancel(); const current = generation; update({ entries: [], truncated: false });
      timer = setTimeout(async () => {
        controller = new AbortController();
        try {
          const response = await fetcher('/api/paths', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ input }), signal: controller.signal });
          const result = await response.json();
          if (current === generation) update(result.ok ? result.payload : { entries: [], truncated: false, error: result.error });
        } catch (error) {
          if (current === generation) update({ entries: [], truncated: false, error: ADMIN.completionError + String(error) });
        }
      }, 150);
    }, dispose: cancel,
  };
}
