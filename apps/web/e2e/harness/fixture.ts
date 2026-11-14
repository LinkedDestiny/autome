import { execFileSync } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve, join } from 'node:path';

export const checkout = resolve(import.meta.dirname, '../../../..');
export type Fixture = { root: string; home: string; repo: string; projectId: string; url: string; pid: number };
export function state(): Fixture {
  if (!process.env.AUTOME_E2E_ROOT) throw Error('Missing private e2e root');
  return JSON.parse(readFileSync(join(process.env.AUTOME_E2E_ROOT, 'state.json'), 'utf8')) as Fixture;
}
export function cli(root: string, args: string[], input?: string) {
  return execFileSync(join(checkout, 'target/debug/autome'), args, {
    cwd: join(root, 'repo'), env: { ...process.env, HOME: root, AUTOME_HOME: join(root, 'autome-home'),
      AUTOMED_CLAUDE_BINARY: join(root, 'bin/fake-cli'), AUTOMED_CODEX_BINARY: join(root, 'bin/fake-cli') },
    encoding: 'utf8', input, timeout: 30_000, stdio: ['pipe', 'pipe', 'pipe'],
  });
}
export function prepare(root: string) {
  execFileSync('bash', [join(checkout, 'apps/web/test/fixtures/w7-flow.sh'), root, checkout], {
    env: { ...process.env, HOME: root, AUTOME_HOME: join(root, 'autome-home'),
      AUTOMED_BIN: join(checkout, 'target/debug/automed'), AUTOME_BIN: join(checkout, 'target/debug/autome') },
    timeout: 60_000, stdio: ['ignore', 'pipe', 'pipe'],
  });
  // Enough draft cards to force the board's intended inner scroll container.
  for (let i = 0; i < 30; i++) cli(root, ['task', 'new', `draft-${i}`, '--draft', '--brief', '-'], '# Draft');
  mkdirSync(join(root, 'completion-directory'));
  const list = JSON.parse(cli(root, ['rpc', 'project.list', '{}'])) as { projects: { project: { id: string } }[] };
  return list.projects[0].project.id;
}
export function startHeldTask(root: string) {
  const fake = join(root, 'repo/.autome/fake/held');
  mkdirSync(fake, { recursive: true });
  writeFileSync(join(fake, 'next'), '1');
  writeFileSync(join(fake, '1.sh'), `i=0\nwhile [ "$i" -lt 400 ]; do echo "scroll evidence line $i"; i=$((i+1)); done\nwhile [ ! -f '${root}/release-held' ]; do '${root}/bin/w7-sleep' 0.1; done\n`);
  const result = JSON.parse(cli(root, ['task', 'new', 'held', '--brief', '-', '--json'], '# Held log session')) as { task: { id: string } };
  return result.task.id;
}
