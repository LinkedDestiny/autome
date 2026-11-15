import { ADMIN } from '../lib/admin-labels.ts';
export interface Probe { rejection?: { kind: string; message: string; project_id?: string } | null; path: string; exists: boolean; is_repo_root: boolean; members: { name: string; default_branch: string }[]; suggest_workspace: boolean; suggested_docs_repo: string | null }
export interface CheckedProject { display: string; probe?: Probe; error?: string }
export interface AddResult { error?: string; project?: { id: string }; skip_error?: string }
export function probeView(probe: Probe) {
  return [!probe.exists ? ADMIN.missing : probe.is_repo_root ? ADMIN.repo : probe.members.length ? '' : ADMIN.plain,
    ...(probe.members.length ? [`包含 ${probe.members.length} 个仓库：${probe.members.map(m => m.name).join('、')}`] : []),
    ...(probe.suggest_workspace ? [ADMIN.suggested] : [])].filter(Boolean);
}
export function canConfirm(input: string, checked: CheckedProject) { return Boolean(!checked.probe?.rejection && checked.probe && input === checked.display); }
export function confirmDefaults(probe: Probe) { return { workspace: probe.suggest_workspace, docs_repo: probe.suggested_docs_repo ?? '', members: probe.members.map(m => m.name) }; }
