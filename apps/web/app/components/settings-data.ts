import { configRows, type ConfigRow } from './config-view.ts';
import type { WebServices } from '../load-context.ts';
import type { OnboardingSession } from '../../server/onboarding-disk.ts';
import { ADMIN, ONBOARDING } from '../lib/admin-labels.ts';
export type LedgerOnboarding = { id: string; started_at: string; lifecycle: { state: string; exit_code?: number } };
export function onboardingState(session: LedgerOnboarding | null | undefined): OnboardingSession {
  if (!session) return { state: 'none' };
  const life = session.lifecycle;
  const label = life.state === 'running' ? '会话运行中' : life.state === 'killed' ? '被停止' : life.state === 'timed_out' ? '超时' : life.state === 'vanished' ? '崩溃（进程消失）' : life.exit_code === 0 ? '正常结束' : '崩溃（退出码 ' + life.exit_code + '）';
  return { state: life.state === 'running' ? 'running' : 'ended', session_id: session.id, exit_code: life.exit_code, label, started_at: session.started_at };
}
export type SettingsProject = { id: string; path: string; display_name: string; onboarding: { onboarding: string; step?: number } };
export type Artefact = { path: string; exists: boolean; content: string };
export type SettingsData = { project?: SettingsProject; files: Artefact[]; session: OnboardingSession; config?: { rows: ConfigRow[]; paths: string[]; warnings: unknown[]; error?: string }; error?: string; artefactsError?: string };
export type OnboardingResult = { error?: string; init_commit?: string | null; saved?: boolean };
export const artefactPaths = ['docs/agent-project-profile.md', 'AGENTS.md'];
export async function loadSettings({ gateway, admin }: WebServices, id: string): Promise<SettingsData> {
  const empty: SettingsData = { files: [], session: { state: 'none' } };
  if (!gateway) return { ...empty, error: ADMIN.offline };
  const result = await gateway.read({ method: 'project.list' });
  if (!result.body.ok) return { ...empty, error: result.body.error };
  const project = (result.body.payload as { projects: { project: SettingsProject }[] }).projects.find(p => p.project.id === id)?.project;
  if (!project) return { ...empty, error: ONBOARDING.missing };
  const detail = await gateway.read({ method: 'project.get', params: { project_id: id } });
  if (!detail.body.ok) return { ...empty, project, error: detail.body.error };
  let session = onboardingState((detail.body.payload as { onboarding_session?: LedgerOnboarding | null }).onboarding_session);
  if (session.state === 'running') {
    const disk = admin?.onboarding.status(project.path);
    if (disk?.error) session = { ...session, error: disk.error };
    if (disk && disk.session_id === session.session_id && (disk.state === 'ended' || disk.state === 'interrupted')) session = { ...session, ...disk, label: disk.state === 'ended' ? '已结束（据磁盘标记，账本尚未记录）' : '进程已消失' };
  }
  const values = await gateway.read({ method: 'config.get', params: { project_id: id } });
  const validation = values.body.ok ? await gateway.read({ method: 'config.validate', params: { project_id: id } }) : null;
  const config = { rows: values.body.ok ? configRows((values.body.payload as { resolved: unknown }).resolved, (values.body.payload as { defaults?: string[] }).defaults) : [],
    paths: admin?.configPaths(project.path) ?? [], warnings: validation?.body.ok ? (validation.body.payload as { violations: unknown[] }).violations : [],
    error: values.body.error ?? validation?.body.error };
  if (project.onboarding.onboarding !== 'in_progress') return { ...empty, project, session, config };
  const artefacts = await gateway.read({ method: 'project.onboarding.artefacts', params: { project_id: id } });
  return { project, session, config, files: artefacts.body.ok ? (artefacts.body.payload as { files: Artefact[] }).files : [], artefactsError: artefacts.body.error };
}
