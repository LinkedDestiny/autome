import { NODE_LABELS } from './labels.ts';
export const LOOP = { edit: '编辑', disabled: '已停用：Autome 不再为这个角色启动会话，双模型检查对这一对免检', intake: NODE_LABELS.intake, approve: '你批准设计', mergeGate: NODE_LABELS.merge_gate, design: '设计循环', implementation: '实现循环' };
export function effortOptions(effort: string | null) {
  return [{ value: '', label: '默认' }, ...['low', 'medium', 'high'].map(value => ({ value, label: value })), ...(!['', 'low', 'medium', 'high'].includes(effort ?? '') ? [{ value: effort!, label: '其它：' + effort, disabled: true }] : [])];
}
