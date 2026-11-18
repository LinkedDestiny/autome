import { readFileSync } from 'node:fs';
import type { ConfigReply } from '../app/components/loop-data.ts';
import { expect, test, vi } from 'vitest';
import { createElement as h } from 'react';
import { renderToString } from 'react-dom/server';
import { createMemoryRouter, RouterProvider, RouterContextProvider } from 'react-router';
import { LoopDefaultsView } from '../app/components/loop-defaults.tsx';
import { defaultFields, validDefault, saveDefault } from '../app/components/loop-defaults-data.ts';
import { servicesContext } from '../app/load-context.ts';
import { action } from '../app/routes/loop.tsx';
import config from './fixtures/m07-global.json' with { type: 'json' };
function html(project?: string, disabled = false, c = config) {
  const router = createMemoryRouter([{ path: '/', element: h(LoopDefaultsView, { config: c as ConfigReply, project, disabled }) }]);
  try { return renderToString(h(RouterProvider, { router })); } finally { router.dispose(); }
}
const ok = { status: 200, body: { ok: true as const } };
test('四项 Stepper 取真实配置值和内核 u32 范围', () => {
  const text = html();
  for (const f of defaultFields) {
    expect(text).toContain(`name="${f.key}"`);
    expect(text).toContain(`min="${f.min}" max="${f.max}"`);
    expect(text).toContain(`value="${config.resolved.loop_defaults[f.key]}"`);
  }
  expect(text).not.toContain('恢复跟随全局');
});
test('四项最小最大合法，越界、小数、NaN、无穷均拒绝', () => {
  for (const f of defaultFields) {
    for (const n of [f.min, f.max]) expect(validDefault(f.key, n)).toBe(true);
    for (const n of [f.min - 1, f.max + 1, 1.5, NaN, Infinity]) expect(validDefault(f.key, n)).toBe(false);
  }
});
test('Stepper 边界按钮禁用，未修改保存禁用', () => {
  for (const bound of ['min', 'max'] as const) {
    const c = structuredClone(config);
    for (const f of defaultFields) c.resolved.loop_defaults[f.key] = f[bound];
    const text = html(undefined, false, c);
    for (const f of defaultFields) expect(text).toContain(`aria-label="${bound === 'min' ? '减少' : '增加'}${f.label}" disabled=""`);
    expect((text.match(/<button aria-label="保存[^"]+" disabled=""/g) ?? []).length).toBe(4);
  }
});
test('项目来源按值比较，相同不冒充继承，四项都有恢复', () => {
  expect(html('p').match(/与全局相同/g)).toHaveLength(4);
  const c = structuredClone(config); c.resolved.loop_defaults.parallel = 2;
  expect(html('p', false, c).match(/项目值/g)).toHaveLength(1);
  expect(html('p').match(/>恢复跟随全局</g)).toHaveLength(4);
});
test('未授权或忙碌禁用全部输入和操作', () => {
  const text = html('p', true);
  expect((text.match(/<button[^>]*>/g) ?? []).every(tag => tag.includes('disabled'))).toBe(true);
  expect((text.match(/<input[^>]*>/g) ?? []).every(tag => tag.includes('disabled'))).toBe(true);
});
test('每项单独写入，项目逐项恢复只发对应 null', async () => {
  const write = vi.fn(async () => ok), gateway = { read: vi.fn(), write };
  for (const f of defaultFields) {
    expect(await saveDefault({ gateway }, undefined, f.key, String(f.min))).toEqual({ saved: true });
    expect(write).toHaveBeenLastCalledWith({ op: 'config.set_loop', params: { [f.key]: f.min } });
    await saveDefault({ gateway }, 'p', f.key, 'null');
    expect(write).toHaveBeenLastCalledWith({ op: 'config.set_loop', params: { project_id: 'p', [f.key]: null } });
  }
});
test('空输入、未知键、全局 null、越界不发请求，前导零按数值保存', async () => {
  const write = vi.fn(async () => ok), gateway = { read: vi.fn(), write };
  for (const [key, value] of [['parallel', ''], ['parallel', ' '], ['unknown', '2'], ['parallel', '6'], ['parallel', 'null'], ['design_rounds', '4294967296']]) expect(await saveDefault({ gateway }, undefined, key, value)).toHaveProperty('error');
  expect(write).not.toHaveBeenCalled();
  await saveDefault({ gateway }, undefined, 'parallel', '02');
  expect(write).toHaveBeenCalledWith({ op: 'config.set_loop', params: { parallel: 2 } });
});
test('离线、拒绝授权、读盘失败、超时、技能拒绝原样保留', async () => {
  expect(await saveDefault({ gateway: null }, 'p', 'parallel', '2')).toEqual({ error: '内核不可用' });
  for (const error of ['拒绝授权', '读盘失败', '请求超时', '设计 绑定了不存在的技能 alpha']) {
    const gateway = { read: vi.fn(), write: vi.fn(async () => ({ status: 409, body: { ok: false as const, error } })) };
    const context = new RouterContextProvider(); context.set(servicesContext, { gateway });
    const request = new Request('http://localhost/loop?project=p', { method: 'POST', body: new URLSearchParams({ intent: 'default', key: 'parallel', value: '2' }) });
    expect(await action({ context, request, params: {}, url: new URL(request.url), pattern: '/loop' })).toEqual({ error });
  }
});

test('四卡同结构、保存可见名统一且可访问名唯一、成功反馈留在对应卡内', () => {
  const css = readFileSync(new URL('../app/styles/app.css', import.meta.url), 'utf8');
  expect(css).toMatch(/\.loop-default-field \{[^}]*display: grid;[^}]*grid-template-rows: auto auto auto/);
  for (const project of [undefined, 'p']) {
    const text = html(project);
    const cards = text.match(/<section class="card card-blue loop-default-field">[\s\S]*?<\/section>/g)!;
    expect(cards).toHaveLength(4);
    const classes = (card: string) => [...card.matchAll(/class="([^"]+)"/g)].map(m => m[1]);
    for (const [i, card] of cards.entries()) {
      expect(classes(card)).toEqual(classes(cards[0]));
      expect(card).toContain(`aria-label="保存${defaultFields[i].label}" disabled=""`);
      expect(card).toContain('>保存</button>');
      expect(card).not.toContain(`>保存${defaultFields[i].label}</button>`);
      if (project) expect(card).toContain(`aria-label="恢复跟随全局 · ${defaultFields[i].label}"`);
    }
  }
  feedback.data = { saved: true };
  try {
    const cards = html('p').match(/<section class="card card-blue loop-default-field">[\s\S]*?<\/section>/g)!;
    for (const card of cards) expect(card).toMatch(/class="loop-default-controls">[\s\S]*?<span role="status">已保存<\/span><\/div>/);
  } finally { feedback.data = undefined; }
});

const feedback = vi.hoisted(() => ({ data: undefined as { saved?: boolean; error?: string } | undefined }));
vi.mock('react-router', async importOriginal => {
  const actual = await importOriginal<typeof import('react-router')>();
  return { ...actual, useFetcher: () => ({ ...actual.useFetcher(), data: feedback.data }) };
});
