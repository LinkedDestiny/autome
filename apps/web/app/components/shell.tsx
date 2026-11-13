import { SidebarFooter } from './sidebar-footer.tsx';
import type { ReactNode } from 'react';
import { Link } from 'react-router';
import { ProjectList } from './project-list.tsx';
import type { ShellProject } from './shell-data.ts';
import { NotificationBell } from './bell.tsx';
import { useWriteGate } from '../lib/live.tsx';

export function Shell({ projects, banner, children, shortcut = 'Ctrl K', onOpenPalette }: {
  shortcut?: string; onOpenPalette?: (query?: string) => void; projects: ShellProject[]; banner: ReactNode; children: ReactNode;
}) {
  const gate = useWriteGate();
  return <div className="shell">
    <nav aria-label="项目" className="shell-sidebar">
      <span className="shell-brand">Autome</span><Link to="/">总览</Link><h2>项目</h2>
      <ProjectList projects={projects} /><SidebarFooter />
    </nav>
    <header className="shell-topbar"><button type="button" className="shell-topbar-title" onClick={() => onOpenPalette?.()}>搜索或跳转 <kbd>{shortcut}</kbd></button>
      <button type="button" className="btn-primary" disabled={!gate.allowed} title={gate.reason} onClick={() => onOpenPalette?.('新建')}>新建任务卡</button>
      <NotificationBell />
    </header>
    <div className="shell-banner">{banner}</div><main className="shell-main">{children}</main>
  </div>;
}
