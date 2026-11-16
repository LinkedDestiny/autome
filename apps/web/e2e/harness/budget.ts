import { appendFileSync, mkdirSync } from 'node:fs';
import { resolve, join } from 'node:path';
import type { TestInfo } from '@playwright/test';
export type Budget = { label: string; duration: number; budget?: number };
export function reportDirectory() {
  return process.env.CI_E2E_REPORT_DIR || resolve(import.meta.dirname, '../../../../.autome/output/playwright-gate');
}
export function recordTiming(value: Budget) {
  mkdirSync(reportDirectory(), { recursive: true });
  appendFileSync(join(reportDirectory(), 'harness.jsonl'), JSON.stringify(value) + '\n');
}
export async function timedExpect(info: TestInfo, label: string, budget: number, run: (timeout: number) => Promise<unknown>) {
  const start = performance.now();
  try { await run(budget); } finally {
    await info.attach('gate-budget', { body: JSON.stringify({ label, budget, duration: performance.now() - start }), contentType: 'application/json' });
  }
}
