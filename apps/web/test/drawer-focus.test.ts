import { afterEach, expect, test, vi } from 'vitest';
import { createElement as h } from 'react';
import { renderToString } from 'react-dom/server';
import { createMemoryRouter, RouterProvider } from 'react-router';
import { Drawer } from '../app/components/ui/drawer.tsx';
import { manageDrawerFocus } from '../app/components/ui/drawer-focus.ts';
import { SkillDrawer } from '../app/routes/skills.tsx';
import { RoleDrawer } from '../app/routes/loop.tsx';
import config from './fixtures/m07-global.json' with { type: 'json' };
import captured from './fixtures/m09-skills.json' with { type: 'json' };
import { resolvedRoles, type ConfigReply } from '../app/components/loop-data.ts';
import type { SkillEntry } from '../app/lib/skills-model.ts';

const hooks = vi.hoisted(() => ({ node: null as unknown, effects: [] as (() => void | (() => void))[] }));
vi.mock('react', async original => ({ ...await original<typeof import('react')>(),
  useRef: () => ({ current: hooks.node }),
  useEffect: (effect: () => void | (() => void)) => { hooks.effects.push(effect); },
}));
class Element {
  tabIndex = 0;
  isConnected = true;
  disabled = false;
  hidden = false;
  visible = true;
  children: Element[] = [];
  listeners = new Set<(event: KeyboardEvent) => void>();
  focus = vi.fn(() => { documentState.activeElement = this; });
  querySelectorAll = vi.fn(() => this.children);
  matches(selector: string) { expect(selector).toBe(':disabled'); return this.disabled; }
  closest(selector: string) { expect(selector).toBe('[hidden], [inert]'); return this.hidden ? this : null; }
  getClientRects() { return this.visible ? [{}] : []; }
  addEventListener(_type: string, listener: (event: KeyboardEvent) => void) { this.listeners.add(listener); }
  removeEventListener(_type: string, listener: (event: KeyboardEvent) => void) { this.listeners.delete(listener); }
}
const documentState = { activeElement: null as Element | null };
function setup(count = 3) {
  const trigger = new Element(), container = new Element();
  container.children = Array.from({ length: count }, () => new Element());
  documentState.activeElement = trigger;
  vi.stubGlobal('document', documentState); vi.stubGlobal('HTMLElement', Element);
  hooks.node = container; hooks.effects = [];
  return { trigger, container, items: container.children };
}
function mount(container: Element) { return manageDrawerFocus(container as unknown as HTMLElement); }
function key(container: Element, shiftKey = false, key = 'Tab') {
  const event = { key, shiftKey, preventDefault: vi.fn() };
  for (const listener of container.listeners) listener(event as unknown as KeyboardEvent);
  return event;
}
afterEach(() => { vi.unstubAllGlobals(); hooks.node = null; hooks.effects = []; });
const noop = () => {};
for (const kind of ['角色', '技能']) {
  test(`${kind}抽屉真实组件挂载移入、卸载恢复触发按钮及清理`, () => {
    const { container, trigger, items } = setup();
    const roles = resolvedRoles(config as ConfigReply);
    const child = kind === '角色'
      ? h(RoleDrawer, { role: 'plan', config: roles.plan, disabled: false, saveDisabled: true, onEdit: noop, onSave: noop, onReset: noop, onClose: noop })
      : h(SkillDrawer, { skill: captured.skills[0] as SkillEntry, scope: 'global', roles, selected: [], disabled: false, onScope: noop, onRoles: noop, onSave: noop, onOpen: noop, onClose: noop });
    const router = createMemoryRouter([{ path: '/', element: child }]);
    try {
      expect(renderToString(h(RouterProvider, { router }))).toContain('tabindex="-1"');
      const cleanups = hooks.effects.map(effect => effect());
      expect(items[0].focus).toHaveBeenCalledOnce();
      expect(container.listeners.size).toBe(1);
      cleanups.forEach(cleanup => cleanup?.());
      expect(trigger.focus).toHaveBeenCalledOnce();
      expect(container.listeners.size).toBe(0);
      expect(key(container).preventDefault).not.toHaveBeenCalled();
    } finally { router.dispose(); }
  });
}
test('Tab 从末控件循环至首控件', () => {
  const { container, items } = setup(); const cleanup = mount(container);
  items[2].focus(); expect(key(container).preventDefault).toHaveBeenCalledOnce();
  expect(documentState.activeElement).toBe(items[0]); cleanup();
});
test('Shift+Tab 从首控件循环至末控件', () => {
  const { container, items } = setup(); const cleanup = mount(container);
  expect(key(container, true).preventDefault).toHaveBeenCalledOnce();
  expect(documentState.activeElement).toBe(items[2]); cleanup();
});
test('中间控件双向 Tab 和其它按键交给原生行为', () => {
  const { container, items } = setup(); const cleanup = mount(container); items[1].focus();
  for (const shift of [false, true]) expect(key(container, shift).preventDefault).not.toHaveBeenCalled();
  expect(key(container, false, 'Enter').preventDefault).not.toHaveBeenCalled(); cleanup();
});
test('零控件回退容器且双向 Tab 不外泄', () => {
  const { container } = setup(0); const cleanup = mount(container);
  expect(documentState.activeElement).toBe(container);
  for (const shift of [false, true]) { expect(key(container, shift).preventDefault).toHaveBeenCalledOnce(); expect(documentState.activeElement).toBe(container); }
  cleanup();
});
test('单控件双向循环', () => {
  const { container, items } = setup(1); const cleanup = mount(container);
  for (const shift of [false, true]) { expect(key(container, shift).preventDefault).toHaveBeenCalledOnce(); expect(documentState.activeElement).toBe(items[0]); }
  cleanup();
});
test('排除禁用、隐藏、不可见及负 tabindex，动态更新候选', () => {
  const { container, items } = setup(5);
  items[0].disabled = true; items[1].hidden = true; items[2].visible = false; items[3].tabIndex = -1;
  const cleanup = mount(container); expect(documentState.activeElement).toBe(items[4]);
  expect(container.querySelectorAll).toHaveBeenCalledWith('button, input, select, textarea, a[href], [tabindex]');
  items[0].disabled = false;
  key(container); expect(documentState.activeElement).toBe(items[0]);
  container.children = []; key(container); expect(documentState.activeElement).toBe(container); cleanup();
});
test('焦点在容器或候选之外时双向回收', () => {
  const { container, items, trigger } = setup(); const cleanup = mount(container);
  container.focus(); key(container); expect(documentState.activeElement).toBe(items[0]);
  trigger.focus(); key(container, true); expect(documentState.activeElement).toBe(items[2]); cleanup();
});
test('触发按钮被移除时跳过恢复，仍清理监听', () => {
  const { container, trigger } = setup(); const cleanup = mount(container);
  trigger.isConnected = false; cleanup(); expect(trigger.focus).not.toHaveBeenCalled(); expect(container.listeners.size).toBe(0);
});
test('Drawer Esc 和关闭按钮调用 onClose，其它键不关闭', () => {
  let calls = 0;
  const element = Drawer({ label: '编辑', title: '标题', onClose: () => calls++, footer: null, children: null });
  element.props.onKeyDown({ key: 'Enter' }); expect(calls).toBe(0);
  element.props.onKeyDown({ key: 'Escape' }); expect(calls).toBe(1);
  element.props.children[0].props.children[1].props.onClick(); expect(calls).toBe(2);
});
