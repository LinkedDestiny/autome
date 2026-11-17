import type { WebServices } from '../load-context.ts';
import { loadLoop, resolvedRoles, type ConfigReply } from './loop-data.ts';
import { roleOrder, roleNames, type Role } from '../lib/role-rules.ts';
import { bindingChanges, bindingExceptions, type SkillEntry, type BindingScope } from '../lib/skills-model.ts';
export type SkillsData = { project?: string; projects: { id: string; display_name: string }[]; skills: SkillEntry[]; config?: ConfigReply; globalConfig?: ConfigReply; error?: string };
export async function loadSkills(services: WebServices, project?: string): Promise<SkillsData> {
  const scoped = await loadLoop(services, project);
  const global = project ? await loadLoop(services) : scoped;
  return { ...scoped, skills: scoped.skills as SkillEntry[], globalConfig: global.config, error: scoped.error ?? global.error };
}
export type BindingResult = { saved: Role[]; messages: string[]; error?: string };
export async function saveBinding(services: WebServices, project: string | undefined, scope: BindingScope, name: string, selected: Role[]): Promise<BindingResult> {
  const result: BindingResult = { saved: [], messages: [] };
  if (!['global', 'project'].includes(scope) || (scope === 'project' && !project) || !name || !Array.isArray(selected) || selected.some(r => !roleOrder.includes(r))) return { ...result, error: '绑定请求无效' };
  const target = scope === 'project' ? project : undefined;
  const data = await loadSkills(services, target);
  if (data.error || !data.config || !services.gateway) return { ...result, error: data.error ?? '内核不可用' };
  // Read the target scope again on save; never copy project overrides into global roles.
  const changes = bindingChanges(name, resolvedRoles(data.config), selected, target);
  for (const params of changes) {
    const reply = await services.gateway.write({ op: 'config.set_role', params });
    if (!reply.body.ok) { result.error = reply.body.error; break; }
    result.saved.push(params.role);
  }
  if (scope === 'global' && result.saved.length) {
    const readbacks = [];
    for (const p of data.projects) {
      const reply = await services.gateway.read({ method: 'config.get', params: { project_id: p.id } });
      if (!reply.body.ok) { result.error = result.error ?? reply.body.error; result.messages.push(`${p.display_name}：未能核实绑定结果`); continue; }
      readbacks.push({ display_name: p.display_name, roles: resolvedRoles(reply.body.payload as ConfigReply) });
    }
    for (const role of result.saved) {
      const bound = selected.includes(role), names = bindingExceptions(name, role, bound, readbacks);
      if (names.length) result.messages.push(`以下项目的 ${roleNames[role]} ${bound ? '没有拿到这条绑定' : '仍保留这条绑定'}：${names.join('、')}（它们在项目里有自己的技能列表）`);
    }
  }
  return result;
}
