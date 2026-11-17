import type { DriverStatus } from '../../server/drivers.ts';
import { formatDuration } from '../lib/clock.tsx';
export function selectedDriver(status?: DriverStatus) {
  return status?.available ? status.list.filter(d => d.driving).sort((a, b) => Date.parse(b.last_tick_at) - Date.parse(a.last_tick_at))[0] : undefined;
}
export function driverCardModel(status: DriverStatus | undefined, now: string) {
  if (!status?.available) return { title: '驱动状态未知', detail: status?.other_driver?.skipped_last_minute ? '另一个驱动可能在推进' : status?.error ?? '', active: false };
  const driver = selectedDriver(status);
  if (!driver) return { title: status.list.length ? '驱动空闲' : '没有驱动在运行', detail: '', active: false };
  const span = (at: string) => Number.isFinite(Date.parse(at)) ? formatDuration(Date.parse(now) - Date.parse(at)) : '时间未知';
  return { title: '驱动在推进', detail: span(driver.last_tick_at) + '前检查过 · 已运行 ' + span(driver.started_at), active: true, pid: driver.pid };
}
