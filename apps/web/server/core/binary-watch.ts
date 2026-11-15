import { statSync } from 'node:fs';

export function createBinaryWatch({ path, intervalMs = 3000, exists = file => {
  try { statSync(file); return true; }
  catch (error) {
    if (['ENOENT', 'ENOTDIR'].includes((error as NodeJS.ErrnoException).code ?? '')) return false;
    throw error;
  }
}, onGone, log }: {
  path: string; intervalMs?: number; exists?: (path: string) => boolean;
  onGone: () => void; log: (message: string) => void;
}) {
  let missing = 0;
  const timer = setInterval(() => {
    try { missing = exists(path) ? 0 : missing + 1; }
    catch (error) { missing = 0; log(`检查内核二进制失败：${String(error)}`); return; }
    if (missing < 2) return;
    clearInterval(timer);
    onGone();
  }, intervalMs);
  return { stop: () => clearInterval(timer) };
}
