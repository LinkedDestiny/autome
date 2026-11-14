import { createContext, useContext, useEffect, useState, type ReactNode } from 'react';

export type Clock = { now: string; timeZone: string };
export const DEFAULT_CLOCK: Clock = { now: '1970-01-01T00:00:00Z', timeZone: 'UTC' };
export function currentClock(): Clock {
  return { now: new Date().toISOString(), timeZone: Intl.DateTimeFormat().resolvedOptions().timeZone };
}
const Context = createContext<Clock>(DEFAULT_CLOCK);
export function ClockProvider({ initial, children }: { initial: Clock; children?: ReactNode }) {
  const [clock, setClock] = useState(initial);
  useEffect(() => {
    setClock(currentClock());
    const timer = setInterval(() => setClock(currentClock()), 30_000);
    return () => clearInterval(timer);
  }, []);
  return <Context.Provider value={clock}>{children}</Context.Provider>;
}
export const useClock = () => useContext(Context);
function parts(iso: string, clock: Clock) {
  const value = Date.parse(iso);
  if (!Number.isFinite(value)) return null;
  return Object.fromEntries(new Intl.DateTimeFormat('en-GB', { timeZone: clock.timeZone, year: 'numeric', month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit', hourCycle: 'h23' }).formatToParts(value).map(p => [p.type, p.value]));
}
export function formatClock(iso: string, clock: Clock): string {
  const date = parts(iso, clock), now = parts(clock.now, clock);
  if (!date || !now) return iso;
  const time = `${date.hour}:${date.minute}`;
  if (date.year === now.year && date.month === now.month && date.day === now.day) return time;
  return `${date.year === now.year ? '' : `${date.year}年`}${Number(date.month)}月${Number(date.day)}日 ${time}`;
}
export function formatRange(start: string, end: string | null | undefined, clock: Clock): string {
  const a = parts(start, clock), b = end ? parts(end, clock) : null;
  const sameDay = a && b && ['year', 'month', 'day'].every(k => a[k] === b[k]);
  return `${formatClock(start, clock)} → ${end ? sameDay ? `${b!.hour}:${b!.minute}` : formatClock(end, clock) : '尚未结束'}`;
}
export function formatDuration(milliseconds: number, precision: 'second' | 'minute' = 'second'): string {
  const seconds = Number.isFinite(milliseconds) ? Math.max(0, Math.floor(milliseconds / 1000)) : 0;
  const minutes = Math.floor(seconds / 60), hours = Math.floor(minutes / 60), days = Math.floor(hours / 24);
  if (days) return `${days} 天 ${hours % 24} 小时`;
  if (hours) return `${hours} 小时${minutes % 60 ? ` ${minutes % 60} 分` : ''}`;
  if (precision === 'minute') return `${minutes} 分`;
  if (minutes) return `${minutes} 分${seconds % 60 ? ` ${seconds % 60} 秒` : ''}`;
  return `${seconds} 秒`;
}
