export const roleOrder = ['plan', 'review', 'adjudicate', 'impl', 'audit', 'retro'] as const;
export type Role = typeof roleOrder[number];
export type RoleConfig = { enabled: boolean; runtime: 'claude' | 'codex'; model: string; effort: string | null; skills: string[] };
export type Roles = Record<Role, RoleConfig>;
export type Skill = { name: string; visible_to: string[] };
export type Violation = { kind: string; roles: Role[]; message: string };
export const roleNames: Record<Role, string> = { plan: '设计', review: '评审', adjudicate: '裁决', impl: '实现', audit: '审计', retro: '复盘' };
export function roleRules(roles: Roles, skills: Skill[]): Violation[] {
  const errors: Violation[] = [];
  for (const role of roleOrder) {
    const cfg = roles[role];
    for (const name of cfg.skills) {
      const skill = skills.find(s => s.name === name);
      if (!skill || !skill.visible_to.includes(cfg.runtime)) errors.push({ kind: skill ? 'skill_not_visible' : 'skill_not_found', roles: [role], message: `${roleNames[role]}：技能 ${name}${skill ? '对当前工具不可见' : '在当前范围不存在'}` });
    }
    const generator = role === 'review' ? 'plan' : role === 'audit' || role === 'retro' ? 'impl' : null;
    if (!generator) continue;
    const other = roles[generator];
    if (cfg.enabled && other.enabled && cfg.runtime === other.runtime && cfg.model === other.model) errors.push({ kind: cfg.model.trim() ? 'same_model' : 'both_default_models', roles: [role, generator], message: `${roleNames[role]}与${roleNames[generator]}${cfg.model.trim() ? '使用同一模型' : '都用 CLI 默认'}，请指定不同模型或工具。` });
  }
  return errors;
}
export function saveOrder(base: Roles, patches: Partial<Record<Role, Partial<RoleConfig>>>, skills: Skill[]): Role[] | null {
  const pending = roleOrder.filter(r => patches[r]);
  function search(state: Roles, rest: Role[]): Role[] | null {
    if (!rest.length) return [];
    for (const role of rest) {
      const next = { ...state, [role]: { ...state[role], ...patches[role] } };
      if (roleRules(next, skills).length) continue;
      const tail = search(next, rest.filter(r => r !== role));
      if (tail) return [role, ...tail];
    }
    return null;
  }
  return search(base, pending);
}
export function changedFields(base: RoleConfig, draft: RoleConfig): Partial<RoleConfig> {
  return Object.fromEntries(Object.entries(draft).filter(([key, value]) => JSON.stringify(value) !== JSON.stringify(base[key as keyof RoleConfig])));
}
