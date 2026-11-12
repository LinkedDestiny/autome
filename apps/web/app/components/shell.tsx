import type { ReactNode } from 'react';
import { Link } from 'react-router';
import { ProjectList } from './project-list.tsx';
import type { ShellProject } from './shell-data.ts';
import { NotificationBell } from './bell.tsx';
import { useWriteGate } from '../lib/live.tsx';

export function Shell({ projects, banner, children }: {
  projects: ShellProject[]; banner: ReactNode; children: ReactNode;
}) {
  const gate = useWriteGate();
  return <div className="shell">
    <nav aria-label="项目" className="shell-sidebar">
      <span className="shell-brand">Autome</span><Link to="/">总览</Link><h2>项目</h2>
      <ProjectList projects={projects} />
    </nav>
    <header className="shell-topbar"><span className="shell-topbar-title">工作面板</span>
      <button type="button" className="btn-primary" disabled title={gate.allowed ? '新建入口即将开放' : gate.reason}>新建任务卡</button>
      <NotificationBell />
    </header>
    <div className="shell-banner">{banner}</div><main className="shell-main">{children}</main>
  </div>;
}
