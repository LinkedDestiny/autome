import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

export type OnboardingSession = { state: 'none' | 'running' | 'ended' | 'interrupted'; session_id?: string; exit_code?: number; error?: string };
const sessionId = /^ses-[a-f0-9]{32}$/;
const directory = (path: string) => join(path, '.autome/output/sessions/onboarding');
function optionalRead(path: string) {
  try { return readFileSync(path, 'utf8'); } catch (error) { if ((error as NodeJS.ErrnoException).code === 'ENOENT') return undefined; throw error; }
}
export function createOnboardingWatch(publish: (message: { type: string; [key: string]: unknown }) => void = () => {}) {
  const watched = new Map<string, { path: string; id: string }>();
  function inspect(path: string, id: string): OnboardingSession {
    const base = join(directory(path), id);
    const exit = optionalRead(base + '.exit');
    if (exit !== undefined) return { state: 'ended', session_id: id, exit_code: Number(exit.match(/^exit_code=(-?\d+)$/m)?.[1] ?? NaN) };
    const pid = Number(optionalRead(base + '.pid')?.trim());
    let alive = false;
    if (Number.isSafeInteger(pid) && pid > 0) {
      try { process.kill(pid, 0); alive = true; } catch (error) { alive = (error as NodeJS.ErrnoException).code === 'EPERM'; }
    }
    return { state: alive || Date.now() - statSync(base + '.prompt').mtimeMs < 10000 ? 'running' : 'interrupted', session_id: id };
  }
  function status(path: string): OnboardingSession {
    try {
      let names: string[];
      try { names = readdirSync(directory(path)); } catch (error) { if ((error as NodeJS.ErrnoException).code === 'ENOENT') return { state: 'none' }; throw error; }
      const latest = names.filter(name => name.endsWith('.prompt') && sessionId.test(name.slice(0, -7)))
        .map(name => ({ id: name.slice(0, -7), time: statSync(join(directory(path), name)).mtimeMs })).sort((a, b) => b.time - a.time)[0];
      return latest ? inspect(path, latest.id) : { state: 'none' };
    } catch (error) { return { state: 'interrupted', error: (error as Error).message }; }
  }
  return {
    status,
    watch(projectId: string, path: string, id: string) { if (sessionId.test(id)) watched.set(projectId + ':' + id, { path, id }); },
    onTick() {
      for (const [key, { path, id }] of watched) {
        let state: OnboardingSession;
        try { state = inspect(path, id); } catch { continue; }
        if (state.state === 'running') continue;
        publish({ type: 'onboarding.ended', project_id: key.slice(0, -(id.length + 1)), session_id: id });
        watched.delete(key);
      }
    },
  };
}
