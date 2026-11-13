import { writeFileSync } from 'node:fs';
import { setTimeout as delay } from 'node:timers/promises';
import { w9Kernel } from './w9-replies.ts';
import type { EnvSnapshot } from '../../app/components/env-model.ts';
const s = await w9Kernel();
try {
  const deadline = Date.now() + 20000;
  let environment: EnvSnapshot;
  while (true) {
    environment = await s.core.call('env.get', {}) as EnvSnapshot;
    if (environment.probed && !environment.probing) break;
    if (Date.now() >= deadline) throw new Error('环境检测超时');
    await delay(50);
  }
  const recipes = Object.fromEntries(await Promise.all(environment.environment!.components.map(async ({ component }) => [component, await s.core.call('env.install_recipe', { component })])));
  writeFileSync(new URL('./w9-env-replies.json', import.meta.url), JSON.stringify({ environment, recipes }, null, 2) + '\n');
} finally { await s.close(); }
