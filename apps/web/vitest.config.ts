import { defineConfig } from 'vitest/config';
import { testGroups } from './test-groups.ts';
export default defineConfig({
  test: {
    globalSetup: ['./test/fixtures/global-setup.ts'],
    projects: Object.entries(testGroups).map(([name, include]) => ({
      test: {
        name, include, testTimeout: 60_000, hookTimeout: 60_000,
        // Vitest requires the same worker limit within a scheduling group.
        // Finish parallel work before suites that inspect processes/shared state.
        sequence: { groupOrder: name === 'exclusive' ? 1 : 0 },
        ...(name === 'exclusive' ? { fileParallelism: false } : { maxWorkers: 4 }),
      },
    })),
  },
});
