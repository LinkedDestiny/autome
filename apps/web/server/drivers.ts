export type Driver = { pid: number; command: string; started_at: string; last_tick_at: string; driving: boolean; self: boolean };
export type DriverStatus = { available: boolean; list: Driver[]; error?: string; other_driver?: { skipped_last_minute: number; last_skipped_at: string | null } };
export async function readDrivers(call: (method: string) => Promise<unknown>, pid: () => number | undefined, other: () => NonNullable<DriverStatus['other_driver']>): Promise<DriverStatus> {
  try {
    const result = await call('driver.list') as { drivers: Omit<Driver, 'self'>[] };
    return { available: true, list: result.drivers.map(driver => ({ ...driver, self: driver.pid === pid() })) };
  } catch (error) {
    if ((error as { code?: string }).code === 'unknown_method') return { available: false, list: [], other_driver: other() };
    return { available: false, list: [], error: error instanceof Error ? error.message : String(error) };
  }
}
export function driverSummary(drivers: DriverStatus | undefined, other?: DriverStatus['other_driver']): string {
  if (drivers?.error) return '驱动信息暂不可用：' + drivers.error;
  if (!drivers?.available) {
    const skipped = (drivers?.other_driver ?? other)?.skipped_last_minute ?? 0;
    return '内核较旧，只能推断：' + (skipped ? '有另一个驱动在推进（最近 1 分钟让路 ' + skipped + ' 次）' : '最近 1 分钟没有发现其他驱动');
  }
  const others = drivers.list.filter(driver => !driver.self);
  return others.map(driver => driver.driving
    ? '另一个驱动在推进：pid ' + driver.pid + '（' + driver.command + '），最近一次 tick ' + driver.last_tick_at
    : 'pid ' + driver.pid + ' 已登记但最近没有推进（' + driver.command + '），最近一次 tick ' + driver.last_tick_at).join('；') || '没有其他驱动';
}

// Compare only the card's observable summary, not registry ordering or idle ticks.
export function driverCardKey(status: DriverStatus): string {
  const selected = status.available ? status.list.filter(driver => driver.driving)
    .sort((a, b) => Date.parse(b.last_tick_at) - Date.parse(a.last_tick_at))[0] : undefined;
  return JSON.stringify([status.available, Boolean(status.list.length), selected?.pid,
    selected?.last_tick_at, selected?.started_at, status.error, Boolean(status.other_driver?.skipped_last_minute)]);
}
