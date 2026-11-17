import { idleProject, projectColor } from '../lib/shell-labels.ts';
import { ALERTS } from '../lib/admin-labels.ts';
import { Link } from 'react-router';
import { COLUMN_LABELS } from '../lib/labels.ts';
import type { ShellProject } from './shell-data.ts';
import { projectHref } from './nav-links.ts';
export function ProjectList({ projects }: { projects: ShellProject[] }) {
  return <div className="project-list">{projects.length === 0 ? <p>还没有项目。可以用命令行 <code>autome project add &lt;路径&gt;</code> 添加</p> : projects.map(project => {
    const running = project.counts?.running ?? '—', needsYou = project.counts?.needs_you ?? '—';
    return <Link className="project-card" key={project.id} to={projectHref(project.id)} title={project.error}
      aria-label={`${project.name}：${COLUMN_LABELS.running} ${running}，${COLUMN_LABELS.needs_you} ${needsYou}${project.alerts ? `，${ALERTS.count(project.alerts)}` : ''}`}>
      <span className={'project-square sq-' + projectColor(project.id)} /><strong>{project.name}</strong>{project.configError && <span className="count-pill count-alert" title={project.configError}>配置有误</span>}<span className="project-counts">
        {running === 0 && needsYou === 0 ? <small>{idleProject(project.done)}</small> : <>
        <span className={`count-pill count-running${running === 0 ? ' count-zero' : ''}`}>{`${COLUMN_LABELS.running} ${running}`}</span>
        <span className={`count-pill count-needs-you${needsYou === 0 ? ' count-zero' : ''}`}>{`${COLUMN_LABELS.needs_you} ${needsYou}`}</span>
      </>}
      {!!project.alerts && <span className="count-pill count-alert">{ALERTS.count(project.alerts)}</span>}
      </span></Link>;
  })}</div>;
}
