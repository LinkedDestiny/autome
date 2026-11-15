// Resolve the same dependency graph as `vitest related`, without running tests.
// File-sized candidates let the shell runner enforce its admission budget.
import { writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { relative } from 'node:path';
import { createVitest } from 'vitest/node';

/** @param {string} value */
const quote = value => `'${value.replaceAll("'", "'\\''")}'`;
/** @param {string[]} command */
const display = command => command.map(quote).join(' ');

const [output, ...related] = process.argv.slice(2);
const root = fileURLToPath(new URL('../', import.meta.url));
const ctx = await createVitest({ root, related, watch: false });
try {
  const specs = await ctx.getRelevantTestSpecifications([]);
  /** @param {import('vitest/node').TestSpecification} spec */
  const rank = spec => related.includes(relative(root, spec.moduleId)) ? -1
    : ['unit', 'kernel', 'exclusive'].indexOf(spec.project.name);
  specs.sort((a, b) => rank(a) - rank(b) || a.moduleId.localeCompare(b.moduleId));
  const lines = specs.map(spec => {
    const command = ['sh', '-c', `cd apps/web && npx vitest run --project ${quote(spec.project.name)} ${quote(relative(root, spec.moduleId))}`];
    const name = quote(display(command));
    return `if ci_quick_admit ${name}; then ci_partial ${name} --cases vitest ${display(command)}; fi`;
  });
  if (!lines.length) lines.push(`ci_not_run ${quote('vitest related：没有匹配的测试文件')}`);
  writeFileSync(output, lines.join('\n') + '\n');
} finally {
  await ctx.close();
}
