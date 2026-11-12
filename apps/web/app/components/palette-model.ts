import { STATE_LABELS } from '../lib/labels.ts';
import { columnOf } from './task-columns.ts';
import { newTaskHref, projectHref, taskHref } from './nav-links.ts';
import type { ShellData } from './shell-data.ts';

type Gate = { allowed: boolean; reason?: string };
type Shortcut = Pick<KeyboardEvent, 'key' | 'metaKey' | 'ctrlKey' | 'shiftKey' | 'altKey' | 'isComposing'>;
export const isMac = (platform: string) => /Mac|iPhone|iPad/.test(platform);
export function isPaletteShortcut(event: Shortcut, platform: string) {
  return event.key.toLowerCase() === 'k' && !event.shiftKey && !event.altKey && !event.isComposing
    && (isMac(platform) ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey);
}
export function attachPaletteShortcut(target: EventTarget, platform: string, onToggle: () => void) {
  const listener = (event: Event) => {
    if (isPaletteShortcut(event as KeyboardEvent, platform)) { event.preventDefault(); onToggle(); }
  };
  target.addEventListener('keydown', listener);
  return () => target.removeEventListener('keydown', listener);
}
export interface Candidate { key: string; id: string; title: string; project: string; subtitle: string; href: string; defaultVisible: boolean; disabled: boolean; reason?: string }
export function buildCandidates(shell: ShellData, gate: Gate): Candidate[] {
  const ranks = { needs_you: 0, running: 1, todo: 2, done: 3 };
  const rank = (state: ShellData['tasks'][number]['state']) => ranks[columnOf(state) as keyof typeof ranks] ?? 4;
  const tasks = [...shell.tasks].sort((a, b) => rank(a.state) - rank(b.state) || Number(b.id.match(/\d+/)?.[0] ?? 0) - Number(a.id.match(/\d+/)?.[0] ?? 0));
  return [
    ...tasks.map(task => {
      const project = shell.projects.find(project => project.id === task.project_id)?.name ?? task.project_id;
      return { key: `task:${task.project_id}:${task.id}`, id: task.id, title: task.title, project,
        subtitle: `${project} · ${STATE_LABELS[task.state.state as keyof typeof STATE_LABELS] ?? task.state.state}`, href: taskHref(task.id),
        defaultVisible: rank(task.state) < 2, disabled: false };
    }),
    ...shell.projects.map(project => ({ key: `project:${project.id}`, id: project.id, title: project.name, project: project.name, subtitle: '项目', href: projectHref(project.id), defaultVisible: true, disabled: false })),
    ...shell.projects.map(project => ({ key: `new:${project.id}`, id: project.id, title: `在「${project.name}」新建任务卡`, project: project.name, subtitle: '新建任务卡', href: newTaskHref(project.id), defaultVisible: true, disabled: !gate.allowed, reason: gate.reason })),
  ];
}
export function filterCandidates(list: Candidate[], query: string) {
  const needle = query.trim().toLowerCase();
  const matches = list.filter(item => needle ? [item.id, item.title, item.project, item.subtitle].some(value => value.toLowerCase().includes(needle)) : item.defaultVisible);
  return { items: matches.slice(0, 50), remaining: Math.max(0, matches.length - 50) };
}
export interface PaletteState { open: boolean; query: string; active: number; navigate?: string }
export type PaletteAction = { type: 'open' | 'input'; query?: string; items: Candidate[] } | { type: 'down' | 'up' | 'enter'; items: Candidate[] } | { type: 'close' };
export const initialPalette: PaletteState = { open: false, query: '', active: -1 };
export function paletteReducer(state: PaletteState, action: PaletteAction): PaletteState {
  if (action.type === 'close') return { ...state, open: false, navigate: undefined };
  if (action.type === 'open' || action.type === 'input') return { open: true, query: action.query ?? '', active: action.items.findIndex(item => !item.disabled) };
  if (!state.open) return state;
  if (action.type === 'enter') {
    const item = action.items[state.active];
    return item && !item.disabled ? { ...state, open: false, navigate: item.href } : state;
  }
  const { items } = action;
  for (let step = 1; step <= items.length; step++) {
    const base: number = state.active < 0 ? (action.type === 'down' ? -1 : 0) : state.active;
    const active = (base + (action.type === 'down' ? step : -step) + items.length) % items.length;
    if (!items[active].disabled) return { ...state, active, navigate: undefined };
  }
  return { ...state, active: -1 };
}
