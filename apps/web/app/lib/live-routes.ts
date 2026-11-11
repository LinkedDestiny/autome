import type { Params, ShouldRevalidateFunction } from 'react-router';
import type { LedgerEvent } from '../../server/eventbus-protocol.ts';

type Matcher = (event: LedgerEvent, params: Params) => boolean;
let pending: LedgerEvent[] | undefined;
let all = false;
let generation = 0;

async function run(revalidate: () => void | Promise<void>) {
  const current = ++generation;
  try { await revalidate(); }
  finally {
    if (generation === current) { pending = undefined; all = false; }
  }
}
export function revalidateForEvents(events: LedgerEvent[], revalidate: () => void | Promise<void>) {
  pending = [...(pending ?? []), ...events];
  return run(revalidate);
}
export function revalidateAll(revalidate: () => void | Promise<void>) {
  all = true;
  pending ??= [];
  return run(revalidate);
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
