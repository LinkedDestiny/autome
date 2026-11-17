import type { WebServices } from '../load-context.ts';
import { roleOrder, type Role, type RoleConfig, type Roles, type Skill } from '../lib/role-rules.ts';
import type { LoopDefaults } from './loop-defaults-data.ts';
import type { SettingsData } from './settings-data.ts';
export type ConfigReply = { global: { loop_defaults: LoopDefaults }; resolved: { roles: { role: Role; config: RoleConfig; provenance: string }[]; loop_defaults: LoopDefaults } };
export type LoopData = { settings?: SettingsData; project?: string; projects: { id: string; display_name: string }[]; config?: ConfigReply; skills: Skill[]; error?: string };
export function resolvedRoles(config: ConfigReply): Roles { return Object.fromEntries(config.resolved.roles.map(r => [r.role, r.config])) as Roles; }
export async function loadLoop({ gateway }: WebServices, project?: string): Promise<LoopData> {
  const data: LoopData = { project, projects: [], skills: [] };
  if (!gateway) return { ...data, error: '内核不可用' };
  const params = project ? { project_id: project } : {};
  const [projects, config, skills] = await Promise.all([gateway.read({ method: 'project.list' }), gateway.read({ method: 'config.get', params }), gateway.read({ method: 'skills.list', params })]);
  if (projects.body.ok) data.projects = (projects.body.payload as { projects: { project: LoopData['projects'][number] }[] }).projects.map(p => p.project);
  if (config.body.ok) data.config = config.body.payload as ConfigReply;
  if (skills.body.ok) data.skills = (skills.body.payload as { skills: Skill[] }).skills;
  data.error = projects.body.error ?? config.body.error ?? skills.body.error;
  return data;
}
export type SaveResult = { saved: Role[]; error?: string };
export async function saveLoop({ gateway }: WebServices, project: string | undefined, changes: { role: Role; fields: Partial<RoleConfig> }[], reset = false): Promise<SaveResult> {
  const saved: Role[] = [];
  if (!gateway) return { saved, error: '内核不可用' };
  for (const change of changes) {
    if (!roleOrder.includes(change.role)) return { saved, error: '未知角色' };
    const result = await gateway.write({ op: reset ? 'config.reset_role' : 'config.set_role', params: { ...change.fields, role: change.role, ...(project ? { project_id: project } : {}) } });
    if (!result.body.ok) return { saved, error: result.body.error };
    saved.push(change.role);
  }
  return { saved };
}
