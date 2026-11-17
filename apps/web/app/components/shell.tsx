import { TEST_IDS } from '../lib/testids.ts';
import { SidebarFooter } from './sidebar-footer.tsx';
import type { ReactNode } from 'react';
import { NavLink } from 'react-router';
import { ProjectList } from './project-list.tsx';
import type { ShellProject } from './shell-data.ts';
import type { DriverStatus } from '../../server/drivers.ts';
import { NotificationBell } from './bell.tsx';
import { Icon } from './icons.tsx';
import { useWriteGate } from '../lib/live.tsx';
export function Shell({ projects, banner, children, shortcut = 'Ctrl K', onOpenPalette, drivers, skillCount, crumb = '总览' }: {
  shortcut?: string; onOpenPalette?: (query?: string) => void; projects: ShellProject[]; banner: ReactNode; children: ReactNode;
  drivers?: DriverStatus; skillCount?: number; crumb?: string;
}) {
  const gate = useWriteGate();
  const needsYou = projects.reduce((n, project) => n + (project.counts?.needs_you ?? 0), 0);
  return <div className="shell">
    <nav aria-label="项目" className="shell-sidebar">
      <span className="shell-brand"><Icon name="leaf" />Autome</span>
      <NavLink className="shell-nav" to="/" end><Icon name="home" />总览{needsYou > 0 && <span className="nav-count">{needsYou}</span>}</NavLink>
      <NavLink className="shell-nav" to="/loop"><Icon name="route" />Loop 配置</NavLink>
      <NavLink className="shell-nav" to="/skills"><Icon name="bag" />技能{skillCount !== undefined && <span className="nav-count">{skillCount}</span>}</NavLink>
      <h2>项目</h2><ProjectList projects={projects} /><SidebarFooter drivers={drivers} />
    </nav>
    <header className="shell-topbar"><span className="shell-crumb" aria-label="当前位置">{crumb}</span><button type="button" className="shell-topbar-title" onClick={() => onOpenPalette?.()}><Icon name="search" />搜索或跳转 <kbd>{shortcut}</kbd></button>
      <NotificationBell /><button type="button" className="btn btn-primary" disabled={!gate.allowed} title={gate.reason} onClick={() => onOpenPalette?.('新建')}>新建任务卡</button>
    </header>
    <div className="shell-banner">{banner}</div><main className="shell-main" data-testid={TEST_IDS.shellMain}>{children}</main>
  </div>;
}
