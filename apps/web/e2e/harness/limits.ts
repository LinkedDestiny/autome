export const DECLARED_GLOBAL_TIMEOUT_MS = 720_000;
export function effectiveTimeout(now = Date.now()) {
  const deadline = Number(process.env.CI_E2E_DEADLINE);
  return Number.isFinite(deadline) && deadline > 0
    ? Math.max(1, Math.min(DECLARED_GLOBAL_TIMEOUT_MS, deadline - now)) : DECLARED_GLOBAL_TIMEOUT_MS;
}
