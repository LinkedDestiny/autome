import { roleNames, roleOrder, roleRules, type Role, type Roles, type Skill } from './role-rules.ts';
export type SkillEntry = Skill & { sources: { scope: 'global' | 'project'; runtime: 'claude' | 'codex'; path: string }[]; project_scoped: boolean; bound_roles: Role[]; conflicts: Role[] };
export type BindingScope = 'global' | 'project';
export type SkillFilter = 'all' | 'global' | 'project' | 'bound';
export function filterSkills(skills: SkillEntry[], filter: SkillFilter, query = '') {
  const text = query.trim().toLowerCase();
  return skills.filter(s => (filter === 'all' || (filter === 'global' && s.sources.some(p => p.scope === 'global')) || (filter === 'project' && s.project_scoped) || (filter === 'bound' && s.bound_roles.length > 0)) && [s.name, ...s.sources.map(p => p.path)].some(v => v.toLowerCase().includes(text)));
}
export function skillCounts(skills: SkillEntry[]) {
  return Object.fromEntries((['all', 'global', 'project', 'bound'] as const).map(f => [f, filterSkills(skills, f).length])) as Record<SkillFilter, number>;
}
export function scopedSkill(skill: SkillEntry, scope: BindingScope): Skill | undefined {
  const sources = skill.sources.filter(s => scope === 'project' || s.scope === 'global');
  return sources.length ? { name: skill.name, visible_to: [...new Set(sources.map(s => s.runtime))] } : undefined;
}
export function bindingReason(skill: SkillEntry, scope: BindingScope, roles: Roles, role: Role): string | undefined {
  const selected = scopedSkill(skill, scope);
  const config = { ...roles, [role]: { ...roles[role], skills: [skill.name] } };
  const invalid = roleRules(config, selected ? [selected] : []).some(v => v.roles.includes(role) && v.kind.startsWith('skill_'));
  return invalid ? `${roleNames[role]} 用 ${roles[role].runtime === 'claude' ? 'Claude Code' : 'Codex'}，看不到这个技能（${roles[role].runtime === 'claude' ? '.claude/skills' : '.agents/skills'} 下没有它）` : undefined;
}
export function bindingChanges(name: string, roles: Roles, selected: Role[], project?: string) {
  return roleOrder.filter(role => roles[role].skills.includes(name) !== selected.includes(role)).map(role => ({ role, skills: selected.includes(role) ? [...roles[role].skills, name] : roles[role].skills.filter(s => s !== name), ...(project ? { project_id: project } : {}) }));
}
export function bindingExceptions(name: string, role: Role, bound: boolean, projects: { display_name: string; roles: Roles }[]) {
  return projects.filter(p => p.roles[role].skills.includes(name) !== bound).map(p => p.display_name);
}
