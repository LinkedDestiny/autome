import { expect, test } from 'vitest';
import { createGateway } from '../server/gateway.ts';
import { registerOpenPath } from '../server/open-path.ts';
import captured from './fixtures/m09-skills.json' with { type: 'json' };
function setup(failure?: Error, openFailure?: Error) { const opened: string[] = [], calls: unknown[] = []; const gateway = createGateway({ home: '/tmp/test', core: { restart: async () => ({ restarted: true }), call: async (method, params) => { calls.push({ method, params }); if (failure) throw failure; return captured; } } }); registerOpenPath(gateway, async path => { if (openFailure) throw openFailure; opened.push(path); }); return { opened, calls, write: (params: object) => gateway.write({ op: 'open.path', params: { kind: 'skill', name: 'alpha', ...params } }) }; }
test('按名解析服务端路径交给注入打开器', async () => { const s = setup(); expect((await s.write({})).status).toBe(200); expect(s.opened).toEqual([captured.skills[0].sources[0].path]); });
test('多来源按序号且 project_id 交给内核', async () => { const s = setup(); await s.write({ source: 1, project_id: 'A' }); expect(s.opened).toEqual([captured.skills[0].sources[1].path]); expect(s.calls).toEqual([{ method: 'skills.list', params: { project_id: 'A' } }]); });
test('未知名返回 404', async () => { const s = setup(); expect(await s.write({ name: 'missing' })).toMatchObject({ status: 404, body: { error: '已拒绝：没有这个技能' } }); expect(s.opened).toEqual([]); });
test('其它 kind 仍 501', async () => { const s = setup(); expect((await s.write({ kind: 'project' })).status).toBe(501); expect(s.calls).toEqual([]); });
test('不接受浏览器路径', async () => { const s = setup(); expect((await s.write({ path: '/etc' })).status).toBe(400); expect((await s.write({ name: '../alpha' })).status).toBe(400); expect(s.opened).toEqual([]); });
test.each([-1, 0.5, '0', Number.MAX_SAFE_INTEGER + 1])('非法序号 %s 拒绝', async source => { const s = setup(); expect((await s.write({ source })).status).toBe(400); expect(s.opened).toEqual([]); });
test('越界最大序号不打开', async () => { const s = setup(); expect((await s.write({ source: Number.MAX_SAFE_INTEGER })).status).toBe(404); expect(s.opened).toEqual([]); });
test('空名称拒绝', async () => { expect((await setup().write({ name: '' })).status).toBe(400); });
test('读盘或内核不可用错误可见', async () => { expect((await setup(new Error('读盘失败')).write({})).body.error).toContain('读盘失败'); });
test('超时回 504 不打开', async () => { const s = setup(Object.assign(new Error('超时'), { timeout: true })); expect((await s.write({})).status).toBe(504); expect(s.opened).toEqual([]); });
test('打开器拒绝可见且不报告成功', async () => { expect(await setup(undefined, new Error('拒绝授权')).write({})).toMatchObject({ status: 503, body: { ok: false, error: '已拒绝：打开目录失败：拒绝授权' } }); });
