import { join } from 'node:path';

export function resolveAutomeHome(env: NodeJS.ProcessEnv) {
  return env.AUTOME_HOME ?? join(env.HOME ?? '/tmp', '.autome');
}
