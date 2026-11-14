import { execFileSync } from 'node:child_process';
import { appendFileSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { resolve, join } from 'node:path';

export const checkout = resolve(import.meta.dirname, '../../../..');
export type Fixture = { root: string; home: string; repo: string; projectId: string; url: string; pid: number; tickIntervalMs: number };
export function state(root = process.env.AUTOME_E2E_ROOT): Fixture {
  if (!root) throw Error('Missing private e2e root');
  return JSON.parse(readFileSync(join(root, 'state.json'), 'utf8')) as Fixture;
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

// Reuse the real protocol steps, but give every scenario its own task/worktree.
export function flowSteps(root: string, slug: string, kind: 'feature' | 'bug' = 'feature') {
  const source = kind === 'feature' ? 'feat' : 'bugx';
  const from = join(root, 'repo/.autome/fake', source);
  const to = join(root, 'repo/.autome/fake', slug);
  mkdirSync(to, { recursive: true });
  for (const file of readdirSync(from).filter(name => /^\d+\.sh$/.test(name))) {
    const step = file.slice(0, -3);
    const hold = kind === 'bug' ? `i=0
while [ ! -f '${root}/release-${slug}-${step}' ]; do
  i=$((i+1)); [ "$i" -lt 600 ] || exit 93
  '${root}/bin/w7-sleep' 0.1
done
` : '';
    const script = readFileSync(join(from, file), 'utf8').replaceAll(source, slug)
      .replace('- B-01 以后再做的事', '').replace('- D-01 有争议的事', '');
    writeFileSync(join(to, file), hold + script);
  }
  writeFileSync(join(to, 'next'), '1');
  writeFileSync(join(root, 'release-main-check'), 'release');
}
export function newTask(root: string, slug: string, args: string[] = []) {
  const result = JSON.parse(cli(root, ['task', 'new', slug, '--brief', '-', '--json', ...args], '# ' + slug)) as { task: { id: string } };
  appendFileSync(join(root, 'task-ids'), result.task.id + '\n');
  return result.task.id;
}
export function liveState() {
  if (!process.env.AUTOME_E2E_LIVE_ROOT) throw Error('Missing live root');
  return state(process.env.AUTOME_E2E_LIVE_ROOT);
}
