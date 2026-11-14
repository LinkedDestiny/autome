import { existsSync } from 'node:fs';
import { homedir } from 'node:os';
import { basename, delimiter, resolve } from 'node:path';

export type KernelBinary = { path: string; args: string[]; source: 'env' | 'bundle' | 'cli' | 'cargo' };

export function resolveAutomedBinary({
  env = process.env, home = homedir(),
  webRoot = resolve(import.meta.dirname, '../..'), exists = existsSync,
}: {
  env?: NodeJS.ProcessEnv; home?: string; webRoot?: string;
  exists?: (path: string) => boolean;
} = {}): KernelBinary {
  if (env.AUTOMED_BIN) return { path: env.AUTOMED_BIN, args: basename(env.AUTOMED_BIN) === 'autome' ? ['core'] : [], source: 'env' };
  const bundle = env.AUTOME_BUNDLED_CORE;
  if (bundle) return { path: bundle, args: ['core'], source: 'bundle' };
  const candidates = [...(env.PATH === undefined ? [] : env.PATH.split(delimiter).map(dir => resolve(dir, 'autome'))), resolve(home, '.local/bin/autome')];
  const cli = candidates.find(exists);
  if (cli) return { path: cli, args: ['core'], source: 'cli' };
  return { path: resolve(webRoot, '../../target', env.AUTOMED_CARGO_PROFILE || 'debug', 'automed'), args: [], source: 'cargo' };
}
