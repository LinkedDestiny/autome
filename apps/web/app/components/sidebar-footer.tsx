import { NavLink } from 'react-router';
import { ADMIN } from '../lib/admin-labels.ts';
import type { DriverStatus } from '../../server/drivers.ts';
import { DriverCard } from './driver-card.tsx';
import { Icon } from './icons.tsx';
export function SidebarFooter({ drivers }: { drivers?: DriverStatus }) { return <footer className="sidebar-footer"><NavLink className="shell-nav" to="/projects/new"><Icon name="plus" />{ADMIN.add}</NavLink><div className="sidebar-bottom"><DriverCard initial={drivers} /><NavLink className="shell-nav" to="/env"><Icon name="gear" />{ADMIN.environment}</NavLink></div></footer>; }
