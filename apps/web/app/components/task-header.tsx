import { Link } from 'react-router';
import { headerView } from './task-header-model.ts';
export function TaskHeader({ view }: { view: ReturnType<typeof headerView> }) {
  return <header className="task-detail-header">
    <div className="task-detail-meta"><span>{view.kind}</span><span>{view.id}</span>{view.project && <Link to={view.project.href}>{view.project.label}</Link>}<code>{view.branch}</code></div>
    <h1>{view.title}</h1>
    <div className="task-detail-meta"><span>{view.activity}</span><span>{view.cost}</span><span>{view.gate}</span></div>
  </header>;
}
