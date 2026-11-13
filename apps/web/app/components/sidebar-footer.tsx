import { Link } from 'react-router';
import { ADMIN } from '../lib/admin-labels.ts';
export function SidebarFooter() { return <footer className="sidebar-footer"><Link to="/projects/new">{ADMIN.add}</Link><Link to="/env">{ADMIN.environment}</Link></footer>; }
