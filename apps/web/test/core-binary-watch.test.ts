import { afterEach, expect, test, vi } from 'vitest';
import { createBinaryWatch } from '../server/core/binary-watch.ts';

afterEach(() => vi.useRealTimers());
function fixture() {
  vi.useFakeTimers();
  const exists = vi.fn(() => false);
  const onGone = vi.fn();
  const log = vi.fn();
  const watch = createBinaryWatch({ path: '/kernel', exists, onGone, log });
  return { exists, onGone, log, watch };
}
test('一次缺失不触发', () => {
  const f = fixture(); vi.advanceTimersByTime(3000);
  expect(f.exists).toHaveBeenCalledWith('/kernel'); expect(f.onGone).not.toHaveBeenCalled();
  f.watch.stop();
});
test('连续两次触发且只触发一次', () => {
  const f = fixture(); vi.advanceTimersByTime(5999); expect(f.onGone).not.toHaveBeenCalled();
  vi.advanceTimersByTime(1); expect(f.onGone).toHaveBeenCalledTimes(1);
  vi.advanceTimersByTime(9000); expect(f.onGone).toHaveBeenCalledTimes(1);
});
test('中间恢复则计数清零', () => {
  const f = fixture(); vi.advanceTimersByTime(3000);
  f.exists.mockReturnValueOnce(true); vi.advanceTimersByTime(6000);
  expect(f.onGone).not.toHaveBeenCalled(); vi.advanceTimersByTime(3000);
  expect(f.onGone).toHaveBeenCalledTimes(1);
});
test('stop 后不再检查', () => {
  const f = fixture(); vi.advanceTimersByTime(3000); f.watch.stop(); f.watch.stop();
  vi.advanceTimersByTime(9000); expect(f.exists).toHaveBeenCalledTimes(1); expect(f.onGone).not.toHaveBeenCalled();
});
test('无法读取路径时记录错误，不把未知状态当作连续缺失', () => {
  const f = fixture(); vi.advanceTimersByTime(3000);
  f.exists.mockImplementationOnce(() => { throw Object.assign(new Error('denied'), { code: 'EACCES' }); });
  vi.advanceTimersByTime(6000); expect(f.log).toHaveBeenCalledTimes(1); expect(f.onGone).not.toHaveBeenCalled();
  vi.advanceTimersByTime(3000); expect(f.onGone).toHaveBeenCalledTimes(1);
});
