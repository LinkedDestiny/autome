import { loadOverview } from '../server/overview.ts';
import { ClockProvider, currentClock } from './lib/clock.tsx';
import { useAdminRevalidate } from './components/admin-live.ts';
import { useEffect, useState } from 'react';
import { CommandPalette } from './components/command-palette.tsx';
import { attachPaletteShortcut, buildCandidates, filterCandidates, initialPalette, isMac, paletteReducer } from './components/palette-model.ts';
import { loadShell } from './components/shell-data.ts';
import { servicesContext } from './load-context.ts';
import { byKind, liveShouldRevalidate } from './lib/live-routes.ts';
import { Shell } from './components/shell.tsx';
import appCss from './styles/app.css?url';
import { LiveBanner, useLiveEvents, writeGate, WriteGateProvider, useKernelRecovery } from './lib/live.tsx';
import { Links, Link, Meta, Outlet, isRouteErrorResponse, useRouteError, Scripts, ScrollRestoration, useLoaderData, useMatches, type LoaderFunctionArgs } from "react-router";
export async function loader({ context, request }: LoaderFunctionArgs) {
  const services = context.get(servicesContext);
  const shell = await loadShell(services.gateway, services.drivers), clock = currentClock();
  const overview = new URL(request.url).pathname === '/' ? await loadOverview(services, shell, clock) : undefined;
  return { ...shell, clock, overview };
}
export const shouldRevalidate = liveShouldRevalidate(byKind('task.', 'project.', 'bug.', 'merge.', 'session.', 'gate.'));

export function Layout({ children }: { children: React.ReactNode }) {
  return (<html lang="zh-CN"><head><meta charSet="utf-8" /><title>Autome 面板</title><Meta /><Links /></head><body>{children}<ScrollRestoration /><Scripts /></body></html>);
}
export const links = () => [{ rel: 'stylesheet', href: appCss }];
export default function App() {
  useAdminRevalidate();
  const data = useLoaderData<typeof loader>();
  const { projects, kernel } = data;
  const crumbs = useMatches().map(match => (match.handle as { crumb?: string } | undefined)?.crumb).filter(Boolean).join(' / ');
  const [palette, setPalette] = useState(initialPalette);
  const [shortcut, setShortcut] = useState('Ctrl K');
  const live = useLiveEvents();
  useKernelRecovery({ core: live.core, kernelOk: kernel.ok });
  const gate = writeGate(live);
  const candidates = buildCandidates(data, gate);
  function openPalette(query = '') {
    setPalette(state => paletteReducer(state, { type: 'open', query, items: filterCandidates(candidates, query).items }));
  }
  useEffect(() => {
    const platform = navigator.platform;
    setShortcut(isMac(platform) ? '⌘K' : 'Ctrl K');
    return attachPaletteShortcut(window, platform, () => setPalette(state => state.open
      ? paletteReducer(state, { type: 'close' })
      : paletteReducer(state, { type: 'open', items: filterCandidates(candidates, '').items })));
  }, [candidates]);
  return <ClockProvider initial={data.clock}><WriteGateProvider value={gate}><Shell projects={projects} drivers={data.drivers} skillCount={data.skillCount} crumb={crumbs} shortcut={shortcut} onOpenPalette={openPalette}
    banner={<LiveBanner {...live} kernelError={kernel.ok ? undefined : kernel.error} />}><Outlet /></Shell><CommandPalette candidates={candidates} state={palette} onChange={setPalette} /></WriteGateProvider></ClockProvider>;
}
export function ErrorBoundary() {
  const error = useRouteError();
  const missing = isRouteErrorResponse(error) && error.status === 404;
  const message = error instanceof Error ? error.message : isRouteErrorResponse(error) ? error.statusText : '未知错误';
  return <main className="error-card"><h1>{missing ? '这个页面还没开放' : `页面出错了：${message}`}</h1><Link to="/">回到总览</Link></main>;
}
