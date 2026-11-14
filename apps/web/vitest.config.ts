import { defineConfig } from 'vitest/config';
import { testGroups } from './test-groups.ts';
export default defineConfig({
  test: {
    globalSetup: ['./test/fixtures/global-setup.ts'],
    projects: Object.entries(testGroups).map(([name, include]) => ({
      test: {
        name, include, testTimeout: 60_000, hookTimeout: 60_000,
        sequence: { groupOrder: 0 },
        ...(name === 'exclusive' ? { fileParallelism: false } : name === 'kernel' ? { maxWorkers: 4 } : {}),
      },
    })),
  },
});
