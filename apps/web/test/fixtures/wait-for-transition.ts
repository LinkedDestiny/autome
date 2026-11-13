// Subscribe before the command so events arriving before its reply are retained.
export async function waitForTransition(
  subscribe: (received: () => void) => () => void,
  command: () => Promise<void>,
  timeoutMs = 4500,
): Promise<number> {
  let receivedAt: number | undefined;
  let notify = () => {};
  const unsubscribe = subscribe(() => { receivedAt ??= performance.now(); notify(); });
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    // task.stop waits for session termination before committing the state.
    // That command latency is separate from the web ticker's delivery budget.
    await command();
    const started = performance.now();
    if (receivedAt === undefined) {
      await new Promise<void>((resolve, reject) => {
        notify = resolve;
        timer = setTimeout(() => reject(Error(`task.updated timeout ${timeoutMs} ms`)), timeoutMs);
      });
    }
    return Math.max(0, receivedAt! - started);
  } finally {
    clearTimeout(timer);
    unsubscribe();
  }
}
