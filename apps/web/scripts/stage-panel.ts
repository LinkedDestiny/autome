import { cpSync, existsSync, mkdirSync, readFileSync, realpathSync } from 'node:fs';
import { basename, dirname, resolve } from 'node:path';

const root = resolve(import.meta.dirname, '..');
const output = process.argv[2];
if (!output) throw new Error('stage-panel: 需要输出目录');
if (!existsSync(resolve(root, 'build/server/index.js'))) throw new Error('先 npm run build');
function canonical(path: string): string {
  return existsSync(path) ? realpathSync(path) : resolve(canonical(dirname(path)), basename(path));
}
const out = canonical(resolve(output));
if (out === root || out.startsWith(`${root}/`)) throw new Error('stage-panel: 输出目录必须在 apps/web 之外');
if (existsSync(out)) throw new Error('stage-panel: 输出目录必须不存在，避免残留开发依赖');
mkdirSync(out, { recursive: true });
for (const name of ['build', 'server', 'bin', 'app/load-context.ts', 'package.json']) {
  cpSync(resolve(root, name), resolve(out, name), { recursive: true });
}
const lock = JSON.parse(readFileSync(resolve(root, 'package-lock.json'), 'utf8')) as {
  packages: Record<string, { dev?: boolean; devOptional?: boolean; optional?: boolean }>;
};
for (const [name, info] of Object.entries(lock.packages)) {
  if (!name.startsWith('node_modules/') || info.dev || info.devOptional) continue;
  const source = resolve(root, name);
  if (info.optional && !existsSync(source)) continue;
  cpSync(source, resolve(out, name), { recursive: true, filter: path => path === source || !path.slice(source.length + 1).split('/').includes('node_modules') });
}
