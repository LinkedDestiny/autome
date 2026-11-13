import { writeFileSync } from 'node:fs';
import { join } from 'node:path';
import type { CoreSupervisor } from '../../server/core/supervisor.ts';
import type { ProjectAlert } from '../../app/components/alerts-data.ts';

export function fakeBudgetCli(root: string) {
  const path = join(root, 'budget-cli');
  writeFileSync(path, `#!/bin/sh
cat >/dev/null
cat <<'USAGE'
{"type":"assistant","message":{"id":"msg_1","usage":{"input_tokens":10,"cache_creation_input_tokens":100,"cache_read_input_tokens":0,"output_tokens":5}}}
{"type":"result","subtype":"success","total_cost_usd":0.25,"num_turns":3,"duration_api_ms":4000,"usage":{"input_tokens":10,"cache_creation_input_tokens":100,"cache_read_input_tokens":40,"output_tokens":5}}
USAGE
`, { mode: 0o755 });
  return path;
}
export async function withBudgetCli<T>(root: string, start: () => Promise<T>): Promise<T> {
  const fake = fakeBudgetCli(root);
  const keys = ['AUTOMED_CLAUDE_BINARY', 'AUTOMED_CODEX_BINARY'] as const;
  const previous = keys.map(key => process.env[key]);
  keys.forEach(key => { process.env[key] = fake; });
  try { return await start(); } finally { keys.forEach((key, i) => { if (previous[i] === undefined) delete process.env[key]; else process.env[key] = previous[i]; }); }
}
export async function budgetTask(core: Pick<CoreSupervisor, 'call'>, path: string, id: string) {
  writeFileSync(join(path, '.autome/config.toml'), '[budget]\nper_task_usd = 0.01\n');
  await core.call('project.onboarding.skip', { project_id: id });
  await core.call('task.create', { project_id: id, kind: 'feature', brief: '# Budget alert', draft: false });
}
export async function tickAlerts(core: Pick<CoreSupervisor, 'call'>): Promise<ProjectAlert[]> {
  await core.call('scheduler.tick', {});
  return (await core.call('dashboard.get', {}) as { alerts: ProjectAlert[] }).alerts;
}
