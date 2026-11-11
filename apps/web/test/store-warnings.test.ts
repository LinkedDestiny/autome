import { expect, test } from 'vitest';
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';

function run(filter: boolean) {
  return spawnSync(process.execPath, ['--input-type=module', '-e', `
    ${filter ? "import { filterSqliteWarning } from './server/store-warnings.ts'; filterSqliteWarning(process);" : ''}
    process.emitWarning('SQLite is an experimental feature and might change at any time', 'ExperimentalWarning');
    process.emitWarning('另一实验功能', { type: 'ExperimentalWarning', code: 'OTHER' });
    process.emitWarning('SQLite is an experimental feature but not experimental type', 'Warning');
    const warning = new Error('SQLite is an experimental feature'); warning.name = 'ExperimentalWarning'; process.emitWarning(warning);
    await import('node:sqlite');
  `], { encoding: 'utf8' });
}
test('装过滤后只隐藏 SQLite ExperimentalWarning，其它告警保留', () => {
  const child = run(true); expect(child.status, child.stderr).toBe(0);
  expect(child.stderr).not.toContain('ExperimentalWarning: SQLite'); expect(child.stderr).toContain('另一实验功能');
  expect(child.stderr).toContain('[OTHER]'); expect(child.stderr).toContain('Warning: SQLite is an experimental feature but');
});
test('不装过滤时同一条告警出现', () => {
  const child = run(false); expect(child.status, child.stderr).toBe(0); expect(child.stderr).toContain('ExperimentalWarning: SQLite is an experimental feature');
});
test('start.js 在版本检查后、导入 main.ts 前装过滤', () => {
  const source = readFileSync('server/start.js', 'utf8');
  const filter = source.indexOf("(await import('./store-warnings.ts')).filterSqliteWarning(process)");
  expect(filter).toBeGreaterThan(source.indexOf('process.exit(1)'));
  expect(filter).toBeLessThan(source.indexOf("await import('./main.ts')"));
});
