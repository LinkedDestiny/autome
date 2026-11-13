import type { TaskPayload } from './task-detail-data.ts';
import { failureText } from '../lib/labels.ts';
export const TASK_PAGE_EVENT_PREFIXES = ['task.', 'session.', 'gate.', 'bug.', 'merge.', 'rebase.', 'guard.', 'runtime.'];
export const DETAIL_TABS = [{ id: 'docs', label: '文档' }, { id: 'changes', label: '改动与合并' }, { id: 'decisions', label: '待你决定' }] as const;
const MERGE_NODES = ['await_merge', 'rebase', 'merge_gate', 'merging', 'main_check', 'cleanup'];
export function inMergeSegment(data: TaskPayload) {
  return Boolean(data.task.merge?.authorized || (data.task.state?.state === 'active' && MERGE_NODES.includes(data.task.state.node ?? '')));
}
export function detailTab(data: TaskPayload, search: URLSearchParams) {
  const selected = search.get('tab');
  if (DETAIL_TABS.some(tab => tab.id === selected)) return selected!;
  if (search.has('doc')) return 'docs';
  return inMergeSegment(data) ? 'changes' : 'docs';
}
export type TaskChanges = {
  available: boolean; commits?: number; files?: { path: string; added: number; deleted: number }[];
  total_added?: number; total_deleted?: number; subjects?: string[]; mergeable?: boolean; rebase_first?: boolean;
  blocked_by?: { kind: string; paths: string[] } | null;
  pending_decisions?: { included: number; ruled: number };
  repos?: { behind?: boolean }[];
};
export function changesView(changes: TaskChanges | undefined, payload: TaskPayload) {
  const files = changes?.files ?? [];
  const suspected = files.filter(file => file.added === 0 && file.deleted > 0).length;
  const state = payload.task.state;
  const node = state?.node ?? state?.at;
  const progress = state?.state === 'done' ? 2 : ['main_check', 'cleanup'].includes(node ?? '') ? 1
    : ['merging', 'rebase', 'merge_gate'].includes(node ?? '') ? 0 : -1;
  return {
    available: changes?.available ?? false, files, commits: changes?.commits ?? 0,
    added: changes?.total_added ?? 0, deleted: changes?.total_deleted ?? 0, subjects: changes?.subjects ?? [],
    suspected, warning: suspected >= 5 || (suspected >= 2 && suspected * 2 >= files.length),
    rebase: changes?.mergeable && (changes.rebase_first || changes.repos?.some(repo => repo.behind))
      ? '按合并后会先自动 rebase、跑合并前门禁再合并' : null,
    blocked: changes?.mergeable === false ? changes.blocked_by?.paths ?? [] : [],
    included: (changes?.pending_decisions?.included ?? 0) > 0 ? '有纳入的 Backlog：按合并会先回到实现轮处理它们，不会直接合并' : null,
    progress: ['合并中', '验证 main', '已完成'].map((label, index) => ({ label, current: index === progress, complete: index < progress })),
    failure: state?.state === 'failed' && MERGE_NODES.includes(node ?? '') ? failureText(state.reason ?? {}) : null,
  };
}
export const CHANGES_COPY = { title: '改动与合并', blocked: '未提交改动阻止合并：' };
