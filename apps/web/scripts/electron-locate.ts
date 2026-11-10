import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

export function locateElectron(override: string | undefined, roots: string[]): string | undefined {
  if (override && existsSync(override)) return override;
  for (const root of roots) {
    const pkg = join(root, 'apps/desktop/node_modules/electron');
    const pathFile = join(pkg, 'path.txt');
    if (!existsSync(pathFile)) continue;
    const executable = join(pkg, 'dist', readFileSync(pathFile, 'utf8').trim());
    if (existsSync(executable)) return executable;
  }
}
