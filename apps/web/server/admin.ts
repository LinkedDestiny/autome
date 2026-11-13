import { createPathGuard } from './paths.ts';
import { LocalOpError, type createGateway } from './gateway.ts';
import type { CoreSupervisor } from './core/supervisor.ts';

type Project = { id: string; onboarding: unknown; [key: string]: unknown };
export function createAdmin({ core, userHome }: { core: Pick<CoreSupervisor, 'call'>; userHome: string; automeHome: string }) {
  const paths = createPathGuard({ home: userHome });
  return {
    paths,
    register(gateway: ReturnType<typeof createGateway>) {
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
    onTick() {},
  };
}
export type AdminServices = ReturnType<typeof createAdmin>;
