import { configRows, type ConfigRow } from './config-view.ts';
import type { WebServices } from '../load-context.ts';
import type { OnboardingSession } from '../../server/onboarding-watch.ts';
import { ADMIN, ONBOARDING } from '../lib/admin-labels.ts';
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
  const session = admin?.onboarding.status(project.path) ?? empty.session;
  if (session.state === 'running' && session.session_id) admin?.onboarding.watch(id, project.path, session.session_id);
  const values = await gateway.read({ method: 'config.get', params: { project_id: id } });
  const validation = values.body.ok ? await gateway.read({ method: 'config.validate', params: { project_id: id } }) : null;
  const config = { rows: values.body.ok ? configRows((values.body.payload as { resolved: unknown }).resolved) : [],
    paths: admin?.configPaths(project.path) ?? [], warnings: validation?.body.ok ? (validation.body.payload as { violations: unknown[] }).violations : [],
    error: values.body.error ?? validation?.body.error };
  if (project.onboarding.onboarding !== 'in_progress') return { ...empty, project, session, config };
  const artefacts = await gateway.read({ method: 'project.onboarding.artefacts', params: { project_id: id } });
  return { project, session, config, files: artefacts.body.ok ? (artefacts.body.payload as { files: Artefact[] }).files : [], artefactsError: artefacts.body.error };
}
