import { servicesContext } from './load-context.ts';
import { byKind, liveShouldRevalidate } from './lib/live-routes.ts';
import { NotificationBell } from './components/bell.tsx';
import { LiveBanner, useLiveEvents } from './lib/live.tsx';
import { Links, Meta, Outlet, Scripts, ScrollRestoration, useLoaderData, type LoaderFunctionArgs } from "react-router";
export async function loader({ context }: LoaderFunctionArgs) {
  const { gateway } = context.get(servicesContext);
  if (!gateway) return { kernel: { ok: false, error: '内核未连接：服务未接入网关' }, projects: [], tasks: [] };
  const result = await gateway.read({ method: 'project.list', params: {} });
  if (!result.body.ok) return { kernel: { ok: false, error: result.body.error }, projects: [], tasks: [] };
  const payload = result.body.payload as { projects: { project: { id: string; display_name: string } }[] };
  return { kernel: { ok: true }, projects: payload.projects.map(({ project }) => ({ id: project.id, name: project.display_name })), tasks: [] };
}
export const shouldRevalidate = liveShouldRevalidate(byKind('task.', 'project.', 'bug.', 'merge.'));

export function Layout({ children }: { children: React.ReactNode }) {
  return (<html lang="zh-CN"><head><meta charSet="utf-8" /><title>Autome 面板</title><Meta /><Links /></head><body>{children}<ScrollRestoration /><Scripts /></body></html>);
}
export default function App() {
  const { projects } = useLoaderData<typeof loader>();
  const { connection } = useLiveEvents();
  return <><LiveBanner connection={connection} /><NotificationBell /><nav aria-label="项目">{projects.map(project => <div key={project.id}>{project.name}</div>)}</nav><Outlet /></>;
}
