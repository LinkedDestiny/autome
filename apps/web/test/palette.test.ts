import { createElement } from 'react';
import { renderToString } from 'react-dom/server';
import { MemoryRouter } from 'react-router';
import { expect, it, vi } from 'vitest';
import { attachPaletteShortcut, buildCandidates, filterCandidates, initialPalette, isPaletteShortcut, paletteReducer } from '../app/components/palette-model.ts';
import { CommandPalette } from '../app/components/command-palette.tsx';
import { Shell } from '../app/components/shell.tsx';
import { WriteGateProvider } from '../app/lib/live.tsx';
import type { ShellData } from '../app/components/shell-data.ts';
const shell: ShellData = { kernel: { ok: true }, projects: [{ id: 'a/b ?中', name: 'Alpha 项目', counts: { running: 1, needs_you: 1 } }], tasks: [
  { id: 'T-12', title: '查找标题', kind: 'feature', project_id: 'a/b ?中', state: { state: 'active', node: 'implement' } },
  ...['done', 'cancelled', 'draft', 'stopped'].map((state, index) => ({ id: `T-${index + 1}`, title: state, kind: 'feature', project_id: 'a/b ?中', state: { state } })),
] };
const list = buildCandidates(shell, { allowed: true });
it('快捷键按平台区分，忽略组合输入、Shift、Alt 与双修饰键', () => {
  const base = { key: 'K', metaKey: false, ctrlKey: false, shiftKey: false, altKey: false, isComposing: false };
  for (const platform of ['MacIntel', 'iPhone', 'iPad', 'Win32', 'Linux']) {
    const mac = /Mac|iPhone|iPad/.test(platform);
    expect(isPaletteShortcut({ ...base, metaKey: true }, platform)).toBe(mac);
    expect(isPaletteShortcut({ ...base, ctrlKey: true }, platform)).toBe(!mac);
    const valid = { ...base, metaKey: mac, ctrlKey: !mac };
    for (const flag of ['shiftKey', 'altKey', 'isComposing'] as const) expect(isPaletteShortcut({ ...valid, [flag]: true }, platform)).toBe(false);
    expect(isPaletteShortcut({ ...valid, key: 'x' }, platform)).toBe(false);
    expect(isPaletteShortcut({ ...base, metaKey: true, ctrlKey: true }, platform)).toBe(false);
  }
});
it('快捷键命中取消默认行为、切换一次，卸载后不响应', () => {
  const target = new EventTarget(), toggle = vi.fn();
  const detach = attachPaletteShortcut(target, 'MacIntel', toggle);
  const key = () => Object.assign(new Event('keydown', { cancelable: true }), { key: 'k', metaKey: true });
  const event = key(); target.dispatchEvent(event); expect(event.defaultPrevented).toBe(true); expect(toggle).toHaveBeenCalledTimes(1);
  detach(); const after = key(); target.dispatchEvent(after); expect(after.defaultPrevented).toBe(false); expect(toggle).toHaveBeenCalledTimes(1);
});
it('候选包含全部任务、项目和新建项，排序和链接编码符合契约', () => {
  expect(list.map(item => item.id)).toEqual(['T-4', 'T-12', 'T-3', 'T-1', 'T-2', 'a/b ?中', 'a/b ?中', 'loop', 'skills', 'env', 'a/b ?中', 'add-project']);
  expect(list.map(item => item.href)).toEqual(['/tasks/T-4', '/tasks/T-12', '/tasks/T-3', '/tasks/T-1', '/tasks/T-2', '/projects/a%2Fb%20%3F%E4%B8%AD', '/projects/a%2Fb%20%3F%E4%B8%AD/tasks/new', '/loop', '/skills', '/env', '/loop?project=a%2Fb%20%3F%E4%B8%AD', '/projects/new']);
  expect(list.find(c => c.key.startsWith('new:'))?.title).toBe('在「Alpha 项目」新建任务卡'); expect(list[1].subtitle).toBe('Alpha 项目 · 进行中');
});
it('按 ID、标题、项目名忽略大小写过滤；空查询及 0/1/50/51 边界', () => {
  expect(filterCandidates(list, ' t-12 ').items.map(item => item.id)).toEqual(['T-12']);
  expect(filterCandidates(list, '查找').items.map(item => item.id)).toEqual(['T-12']);
  expect(filterCandidates(list, 'aLPHa').items).toHaveLength(8);
  expect(filterCandidates(list, '   ').items.map(item => item.id)).toEqual(['T-4', 'T-12', 'a/b ?中', 'a/b ?中', 'loop', 'skills', 'env', 'a/b ?中', 'add-project']);
  expect(filterCandidates(list, 'cancelled').items[0].id).toBe('T-2');
  expect(filterCandidates(list, '不存在').items).toEqual([]);
  for (const count of [0, 1, 50, 51]) {
    const result = filterCandidates(Array.from({ length: count }, () => list[0]), '');
    expect(result.items).toHaveLength(Math.min(count, 50)); expect(result.remaining).toBe(Math.max(count - 50, 0));
  }
});
it('open/input/down/up/enter/close 全部迁移，循环跳过禁用项及空列表', () => {
  const items = [list[0], { ...list[1], disabled: true }, list[2]];
  let state = paletteReducer(initialPalette, { type: 'open', items }); expect(state).toEqual({ open: true, query: '', active: 0 });
  state = paletteReducer(state, { type: 'down', items }); expect(state.active).toBe(2);
  state = paletteReducer(state, { type: 'down', items }); expect(state.active).toBe(0);
  state = paletteReducer(state, { type: 'up', items }); expect(state.active).toBe(2);
  state = paletteReducer(state, { type: 'up', items }); expect(state.active).toBe(0);
  state = paletteReducer(state, { type: 'input', query: '新建', items }); expect(state.active).toBe(0); expect(state.query).toBe('新建');
  expect(paletteReducer(state, { type: 'enter', items })).toMatchObject({ open: false, navigate: items[0].href });
  expect(paletteReducer(state, { type: 'close' }).open).toBe(false);
  for (const type of ['down', 'up', 'enter'] as const) {
    expect(paletteReducer(initialPalette, { type, items })).toEqual(initialPalette);
    expect(paletteReducer({ ...state, active: -1 }, { type, items: [] }).navigate).toBeUndefined();
  }
  expect(paletteReducer({ ...state, active: -1 }, { type: 'up', items: [list[0]] }).active).toBe(0);
});
it('写门禁禁用新建项，回车不导航，顶栏 SSR 禁用并显示理由', () => {
  const gate = { allowed: false, reason: '内核离线：测试' };
  const items = filterCandidates(buildCandidates(shell, gate), '新建').items;
  expect(items).toHaveLength(1); expect(items[0].disabled).toBe(true); expect(items[0].reason).toBe(gate.reason);
  const state = paletteReducer(initialPalette, { type: 'open', items }); expect(state.active).toBe(-1);
  for (const type of ['down', 'up', 'enter'] as const) expect(paletteReducer(state, { type, items })).toEqual(state);
  expect(paletteReducer({ ...state, active: 0 }, { type: 'enter', items }).navigate).toBeUndefined();
  const html = renderToString(createElement(MemoryRouter, null, createElement(WriteGateProvider, { value: gate }, createElement(Shell, { projects: shell.projects, banner: null, children: null }))));
  expect(html).toMatch(/<button[^>]*disabled=""[^>]*title="内核离线：测试"[^>]*>新建任务卡/);
});
it('SSR 语义、活动后代、禁用项、截断提示与关闭状态', () => {
  const candidates = buildCandidates(shell, { allowed: false, reason: '正在连接服务' });
  const render = (open: boolean, candidatesOverride = candidates) => renderToString(createElement(MemoryRouter, null, createElement(CommandPalette, { candidates: candidatesOverride, state: { open, query: '', active: 0 }, onChange: () => {} })));
  const html = render(true);
  for (const role of ['dialog', 'combobox', 'listbox', 'option']) expect(html).toContain(`role="${role}"`);
  expect(html).toContain('aria-activedescendant="palette-option-0"'); expect(html).toContain('id="palette-option-0"'); expect(html).toContain('aria-disabled="true"');
  expect(render(false)).toBe(''); expect(render(true, [])).toContain('没有匹配的结果');
  expect(render(true, Array.from({ length: 51 }, (_, index) => ({ ...list[0], key: String(index) })))).toMatch(/还有 <!-- -->1<!-- --> 条/);
});

it('新增 Loop、技能、环境与项目 Loop 入口', () => {
  const list = buildCandidates(shell, { allowed: true });
  for (const href of ['/loop', '/skills', '/env', ...shell.projects.map(p => '/loop?project=' + encodeURIComponent(p.id))]) expect(list.some(c => c.href === href && !c.disabled)).toBe(true);
});
