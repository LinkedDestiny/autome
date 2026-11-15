import { TerminalUnavailable, unavailableTerminal, type TerminalLauncher } from './terminal.ts';
import type { InstallRecipe } from '../app/components/env-model.ts';
import { relative, isAbsolute, join } from 'node:path';
import { realpathSync } from 'node:fs';
import { readOnboardingDisk } from './onboarding-disk.ts';
import { createPathGuard } from './paths.ts';
import { LocalOpError, type createGateway } from './gateway.ts';
import type { CoreSupervisor } from './core/supervisor.ts';

type Project = { id: string; onboarding: unknown; [key: string]: unknown };
export function createAdmin({ core, userHome, automeHome, publish, terminal = unavailableTerminal }: { core: Pick<CoreSupervisor, 'call'>; userHome: string; automeHome: string; terminal?: TerminalLauncher; publish?: (message: { type: string; [key: string]: unknown }) => void }) {
  const paths = createPathGuard({ home: userHome });
  const onboarding = { status: readOnboardingDisk };
  let environment: { generation: number; probing: boolean } | undefined;
  return {
    configPaths(projectPath: string) {
      const display = (path: string) => {
        let home = userHome;
        try { home = realpathSync(userHome); } catch { /* Preserve lexical paths when HOME is unavailable. */ }
        const local = relative(userHome, path), canonical = relative(home, path);
        const inside = (value: string) => value !== '..' && !value.startsWith('../') && !isAbsolute(value);
        const part = inside(local) ? local : inside(canonical) ? canonical : null;
        return part === null ? path : part ? '~/' + part : '~';
      };
      return [display(join(automeHome, 'config.toml')), display(join(projectPath, '.autome/config.toml'))];
    },
    onboarding,
    paths,
    register(gateway: ReturnType<typeof createGateway>) {
      for (const op of ['env.install', 'env.login']) gateway.registerLocalOp(op, async params => {
        const result = await core.call('env.install_recipe', { component: params.component }) as InstallRecipe;
        if (op === 'env.install' && !result.prerequisite_ok) throw new LocalOpError(409, `请先安装 ${result.recipe.prerequisite}`);
        const command = op === 'env.install' ? result.recipe.command : result.login_command;
        if (!command) throw new LocalOpError(400, `${params.component} 不需要登录`);
        try { await terminal(command); return { launched: true, command }; }
        catch (error) {
          if (error instanceof TerminalUnavailable) return { launched: false, command, reason: error.message };
          throw error;
        }
      });
      gateway.registerLocalOp('project.confirm', async params => {
        const path = paths.check(params.path);
        if (!path.ok) throw new LocalOpError(400, path.message);
        if (path.isHome) throw new LocalOpError(400, '不能把整个家目录添加为项目');
        const probe = await core.call('project.probe', { path: path.absolute }) as { members: unknown[] };
        if (params.workspace && !probe.members.length) throw new LocalOpError(400, '这个目录下没有仓库，不能按工作区添加');
        const added = await core.call('project.add', { path: path.absolute, workspace: params.workspace ?? false,
          ...(params.workspace && params.docs_repo !== undefined ? { docs_repo: params.docs_repo } : {}) }) as { project: Project; init: unknown; pending_commit: unknown };
        const result: Record<string, unknown> = { project: added.project, init: added.init, pending_commit: added.pending_commit, probe, onboarding: added.project.onboarding };
        if (params.skip_onboarding) {
          try {
            const skipped = await core.call('project.onboarding.skip', { project_id: added.project.id }) as { onboarding: unknown; init_commit?: string };
            result.onboarding = skipped.onboarding;
            if (skipped.init_commit !== undefined) result.init_commit = skipped.init_commit;
          } catch (error) { result.skip_error = (error as Error).message; }
        }
        return result;
      });
    },
    onTick(report?: { [key: string]: unknown }) {
      if (typeof report?.env_generation !== 'number' || typeof report.env_probing !== 'boolean') return;
      const next = { generation: report.env_generation, probing: report.env_probing };
      if (!environment || next.generation !== environment.generation || next.probing !== environment.probing) publish?.({ type: 'env.changed', ...next });
      environment = next;
    },
  };
}
export type AdminServices = ReturnType<typeof createAdmin>;
