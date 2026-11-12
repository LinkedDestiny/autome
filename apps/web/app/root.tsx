import { servicesContext } from './load-context.ts';
import { byKind, liveShouldRevalidate } from './lib/live-routes.ts';
import { Shell } from './components/shell.tsx';
import appCss from './styles/app.css?url';
import { LiveBanner, useLiveEvents, writeGate, WriteGateProvider, useKernelRecovery } from './lib/live.tsx';
import { Links, Link, Meta, Outlet, isRouteErrorResponse, useRouteError, Scripts, ScrollRestoration, useLoaderData, type LoaderFunctionArgs } from "react-router";
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
export const links = () => [{ rel: 'stylesheet', href: appCss }];
export default function App() {
  const { projects, kernel } = useLoaderData<typeof loader>();
  const live = useLiveEvents();
  useKernelRecovery({ core: live.core, kernelOk: kernel.ok });
  return <WriteGateProvider value={writeGate(live)}><Shell projects={projects}
    banner={<LiveBanner {...live} kernelError={kernel.ok ? undefined : kernel.error} />}><Outlet /></Shell></WriteGateProvider>;
}
export function ErrorBoundary() {
  const error = useRouteError();
  const missing = isRouteErrorResponse(error) && error.status === 404;
  const message = error instanceof Error ? error.message : isRouteErrorResponse(error) ? error.statusText : '未知错误';
  return <main className="error-card"><h1>{missing ? '这个页面还没开放' : `页面出错了：${message}`}</h1><Link to="/">回到总览</Link></main>;
}
