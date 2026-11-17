import type { WebServices } from '../load-context.ts';
export const defaultFields = [
  { key: 'parallel', label: '并行任务数', min: 1, max: 5 },
  { key: 'design_rounds', label: '设计轮上限', min: 1, max: 4294967295 },
  { key: 'budget_factor', label: '实现预算系数', min: 1, max: 4294967295 },
  { key: 'session_minutes', label: '单个会话时长上限（分钟）', min: 5, max: 4294967295 },
] as const;
export type DefaultKey = typeof defaultFields[number]['key'];
export type LoopDefaults = Record<DefaultKey, number>;
export function validDefault(key: string, value: number) {
  const field = defaultFields.find(f => f.key === key);
  return !!field && Number.isInteger(value) && value >= field.min && value <= field.max;
}
export async function saveDefault({ gateway }: WebServices, project: string | undefined, key: string, raw: string) {
  if (!gateway) return { error: '内核不可用' };
  const reset = raw === 'null';
  if (!defaultFields.some(f => f.key === key) || (reset ? !project : !raw.trim() || !validDefault(key, Number(raw)))) return { error: 'Loop 默认值无效' };
  const result = await gateway.write({ op: 'config.set_loop', params: { [key]: reset ? null : Number(raw), ...(project ? { project_id: project } : {}) } });
  return result.body.ok ? { saved: true } : { error: result.body.error };
}
