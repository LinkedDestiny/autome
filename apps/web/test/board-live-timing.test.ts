import { afterEach, expect, test, vi } from 'vitest';
import { revalidateForEvents } from '../app/lib/live-routes.ts';
import { waitForTransition } from './fixtures/wait-for-transition.ts';

afterEach(() => vi.useRealTimers());
function scenario(eventAfter?: number, commandFails = false) {
  vi.useFakeTimers();
  const unsubscribe = vi.fn();
  const result = waitForTransition(received => {
    if (eventAfter !== undefined) setTimeout(() => { void revalidateForEvents([], received); }, eventAfter);
    return unsubscribe;
  }, () => new Promise<void>((resolve, reject) => setTimeout(() => {
    if (commandFails) reject(Error('stop failed')); else resolve();
  }, 3000)));
  return { result, unsubscribe };
}
test('stop grace period does not consume the next tick budget', async () => {
  const { result, unsubscribe } = scenario(5900);
  const checked = expect(result).resolves.toBe(3900);
  await vi.advanceTimersByTimeAsync(6900);
  await checked;
  expect(unsubscribe).toHaveBeenCalledOnce();
});
test('an event before the command reply is retained', async () => {
  const { result, unsubscribe } = scenario(2900);
  const checked = expect(result).resolves.toBe(900);
  await vi.advanceTimersByTimeAsync(3900);
  await checked;
  expect(unsubscribe).toHaveBeenCalledOnce();
});
test('missing event still fails at 4500 ms after the reply', async () => {
  const { result, unsubscribe } = scenario();
  const checked = expect(result).rejects.toThrow('task.updated timeout 4500 ms');
  await vi.advanceTimersByTimeAsync(7499);
  expect(unsubscribe).not.toHaveBeenCalled();
  await vi.advanceTimersByTimeAsync(1);
  await checked;
  expect(unsubscribe).toHaveBeenCalledOnce();
});
test('command failure rejects and removes the listener', async () => {
  const { result, unsubscribe } = scenario(undefined, true);
  const checked = expect(result).rejects.toThrow('stop failed');
  await vi.advanceTimersByTimeAsync(3000);
  await checked;
  expect(unsubscribe).toHaveBeenCalledOnce();
  expect(vi.getTimerCount()).toBe(0);
});
