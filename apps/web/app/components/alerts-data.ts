export interface ProjectAlert {
  id: string; project_id: string; kind: string; task_id: string | null; detail: string; created_at: string;
}
export interface AlertDashboard { alerts?: ProjectAlert[]; environment?: unknown }
export function projectAlerts(dashboard: AlertDashboard, projectId: string): ProjectAlert[] {
  return (dashboard.alerts ?? []).filter(alert => alert.project_id === projectId)
    .sort((a, b) => Date.parse(b.created_at) - Date.parse(a.created_at));
}
export function alertCounts(dashboard: AlertDashboard): Map<string, number> {
  const counts = new Map<string, number>();
  for (const alert of dashboard.alerts ?? []) counts.set(alert.project_id, (counts.get(alert.project_id) ?? 0) + 1);
  return counts;
}
