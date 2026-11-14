import type { Params, ShouldRevalidateFunction } from 'react-router';
import type { LedgerEvent } from '../../server/eventbus-protocol.ts';

type Matcher = (event: LedgerEvent, params: Params) => boolean;
let pending: LedgerEvent[] | undefined;
let all = false;
let generation = 0;

let queued: LedgerEvent[] = [];
let queuedAll = false;
let windowPromise: Promise<void> | undefined;

function schedule(revalidate: () => void | Promise<void>) {
  if (!windowPromise) {
    windowPromise = new Promise<void>((resolve, reject) => {
      setTimeout(() => {
        windowPromise = undefined;
        // Keep in-flight matchers too: React Router may interrupt that request.
        pending = [...(pending ?? []), ...queued];
        all ||= queuedAll;
        queued = []; queuedAll = false;
        void run(revalidate).then(resolve, reject);
      }, 1000);
    });
  }
  return windowPromise;
}

async function run(revalidate: () => void | Promise<void>) {
  const current = ++generation;
  try { await revalidate(); }
  finally {
    if (generation === current) { pending = undefined; all = false; }
  }
}
export function revalidateForEvents(events: LedgerEvent[], revalidate: () => void | Promise<void>) {
  queued.push(...events);
  return schedule(revalidate);
}
export function revalidateAll(revalidate: () => void | Promise<void>) {
  queuedAll = true;
  return schedule(revalidate);
}
export function liveShouldRevalidate(matcher: Matcher): ShouldRevalidateFunction {
  return args => pending !== undefined && args.currentUrl.href === args.nextUrl.href && !args.formMethod
    ? all || pending.some(event => matcher(event, args.currentParams))
    : args.defaultShouldRevalidate;
}
export function byKind(...prefixes: string[]): Matcher {
  return event => prefixes.some(prefix => event.kind.startsWith(prefix));
}
export function bySubjectParam(prefixes: string[], param: string): Matcher {
  const matches = byKind(...prefixes);
  return (event, params) => matches(event, params) && event.subject === params[param];
}
