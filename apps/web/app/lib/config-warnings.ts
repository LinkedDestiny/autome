import { roleLabel } from './labels.ts';
type Warning = Record<string, unknown>;
const role = (value: unknown) => roleLabel(String(value ?? ''));
export const CONFIG_WARNINGS: Record<string, (w: Warning) => string> = {
  same_model: w => `评测方与被评测方是同一个模型：${role(w.evaluator)} 与 ${role(w.generator)}（${w.identity}）`,
  skill_not_visible: w => `${role(w.role)} 的技能 ${w.skill} 在 ${w.runtime} 里看不到`,
  skill_not_found: w => `${role(w.role)} 的技能 ${w.skill} 未安装`,
  parallel_out_of_range: w => `并行数 ${w.value} 超出 ${w.min}–${w.max}`,
  design_rounds_out_of_range: w => `设计轮数 ${w.value} 超出允许范围`,
  session_minutes_out_of_range: w => `会话时长 ${w.value} 分钟低于下限 ${w.min} 分钟`,
  budget_factor_out_of_range: w => `预算系数 ${w.value} 超出允许范围`,
  both_default_models: w => `评测方 ${role(w.evaluator)} 与被评测方 ${role(w.generator)} 都使用默认模型，请为其中一方指定模型`,
};
export function configWarning(value: unknown): string {
  const w = value && typeof value === 'object' ? value as Warning : { kind: String(value) };
  const kind = String(w.kind ?? '');
  return Object.hasOwn(CONFIG_WARNINGS, kind) ? CONFIG_WARNINGS[kind](w)
    : [kind, ...Object.entries(w).filter(([key]) => key !== 'kind').map(([key, v]) => `${key}=${typeof v === 'object' ? JSON.stringify(v) : String(v)}`)].join(' · ');
}
