import { existsSync } from 'node:fs';
import { homedir } from 'node:os';
import { resolve } from 'node:path';

export function resolveAutomedBinary({
  env = process.env, home = homedir(),
  webRoot = resolve(import.meta.dirname, '../..'), exists = existsSync,
}: {
  env?: NodeJS.ProcessEnv; home?: string; webRoot?: string;
  exists?: (path: string) => boolean;
} = {}): { path: string; source: 'env' | 'bundle' | 'local-bin' | 'cargo' } {
  if (env.AUTOMED_BIN) return { path: env.AUTOMED_BIN, source: 'env' };
  const bundle = resolve(webRoot, '../automed');
  if (exists(bundle)) return { path: bundle, source: 'bundle' };
  const local = resolve(home, '.local/bin/automed');
  if (exists(local)) return { path: local, source: 'local-bin' };
  return { path: resolve(webRoot, '../../target', env.AUTOMED_CARGO_PROFILE || 'debug', 'automed'), source: 'cargo' };
}
