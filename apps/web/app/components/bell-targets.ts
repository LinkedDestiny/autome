export const TARGETS = [
  { pattern: '/tasks/:taskId', available: false },
  { pattern: '/projects/:projectId', available: true },
];
export function resolveTarget(item: { task_id: string | null; project_id: string | null }, targets = TARGETS): { href: string | null; note?: string } {
  const pattern = item.task_id ? '/tasks/:taskId' : item.project_id ? '/projects/:projectId' : null;
  if (!pattern) return { href: null };
  if (!targets.find(target => target.pattern === pattern)?.available) return { href: null, note: '详情页尚未开放' };
  return { href: pattern.replace(/:[^/]+/, encodeURIComponent((item.task_id || item.project_id)!)) };
}
