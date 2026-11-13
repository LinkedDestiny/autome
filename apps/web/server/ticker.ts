import type { TickReport } from './core/supervisor.ts';

export function createTicker({ call, isConnected, canTick = isConnected, isSlow = () => false, isBusy, intervalMs = 3000, log, onTick }: {
  call: () => Promise<TickReport>;
  isConnected?: () => boolean;
  canTick?: () => boolean;
  isSlow?: () => boolean;
  isBusy: () => boolean;
  intervalMs?: number;
  log: (line: string) => void;
  onTick?: (report: TickReport) => void;
}) {
  let pending = false;
  let slowIntervals = 0;
  const timer = setInterval(async () => {
    if (!canTick?.() || pending || isBusy()) return;
    if (isSlow()) { if (++slowIntervals < 10) return; }
    slowIntervals = 0;
    pending = true;
    try {
      const report = await call();
      if (!report.skipped) for (const error of report.errors ?? []) log(`[调度] ${error}`);
      onTick?.(report);
    } catch (error) {
      log(`[调度] tick 失败：${error instanceof Error ? error.message : String(error)}`);
    } finally { pending = false; }
  }, intervalMs);
  return { stop() { clearInterval(timer); } };
}
