import { spawnSync, execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { locateElectron } from './electron-locate.ts';

export function checkRuntime(executable: string, expectedElectron?: string): boolean {
  const child = spawnSync(executable, [resolve(import.meta.dirname, 'sqlite-check.mjs')], {
    encoding: 'utf8', timeout: 30_000,
    env: { ...process.env, ELECTRON_RUN_AS_NODE: '1' },
  });
  try {
    if (child.error) throw child.error;
    if (child.status !== 0) throw new Error(child.stderr || `退出码 ${child.status}`);
    const result = JSON.parse(child.stdout);
    if (result.ok !== true || result.journalMode !== 'wal') throw new Error('SQLite 检查失败');
    if (expectedElectron && result.electron !== expectedElectron) {
      throw new Error(`Electron 版本不符：需要 ${expectedElectron}，实际 ${result.electron}`);
    }
    console.log(JSON.stringify({ ...result, warnings: child.stderr.trim() }));
    return true;
  } catch (error) {
    console.error(`SQLite 验证失败：${String(error)}`);
    return false;
  }
}

function main(): number {
  const root = resolve(import.meta.dirname, '../../..');
  const common = execFileSync('git', ['rev-parse', '--git-common-dir'], { cwd: root, encoding: 'utf8' }).trim();
  const mainRoot = dirname(resolve(root, common));
  const expected = JSON.parse(readFileSync(resolve(root, 'apps/desktop/package.json'), 'utf8')).devDependencies.electron;
  const systemOk = checkRuntime(process.execPath);
  const electron = locateElectron(process.env.AUTOME_ELECTRON, [root, mainRoot]);
  if (!electron) {
    console.error('未取证：找不到 Electron 二进制，请设置 AUTOME_ELECTRON。');
    return systemOk ? 2 : 1;
  }
  const electronOk = checkRuntime(electron, expected);
  return systemOk && electronOk ? 0 : 1;
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  process.exitCode = main();
}
