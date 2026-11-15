import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: './e2e', workers: 1, fullyParallel: false, forbidOnly: true, retries: 0,
  // Includes both fixture servers, 29 serial cases and teardown. The old 6 min
  // cap expired after 20 passing cases; keep per-case and assertion limits.
  timeout: 90_000, globalTimeout: 720_000, expect: { timeout: 10_000 },
  globalSetup: './e2e/harness/setup.ts', globalTeardown: './e2e/harness/teardown.ts',
  outputDir: '../../.autome/output/playwright', reporter: 'list',
  use: { viewport: { width: 1512, height: 944 }, permissions: ['clipboard-read', 'clipboard-write'], trace: 'retain-on-failure' },
});
