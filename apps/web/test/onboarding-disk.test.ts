import { mkdtempSync, mkdirSync, writeFileSync, utimesSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, expect, test, vi } from 'vitest';
import { readOnboardingDisk } from '../server/onboarding-disk.ts';
const roots: string[] = [];
afterEach(() => roots.splice(0).forEach(path => rmSync(path, { recursive: true, force: true })));
function setup() { const root = mkdtempSync(join(tmpdir(), 'watch-')); roots.push(root); const dir = join(root, '.autome/output/sessions/onboarding'); mkdirSync(dir, { recursive: true }); const id = 'ses-' + 'a'.repeat(32); writeFileSync(join(dir, id + '.prompt'), ''); const publish = vi.fn(); return { root, dir, id, publish, watch: { status: readOnboardingDisk } }; }
test('退出标记解析退出码', () => { const s = setup(); writeFileSync(join(s.dir, s.id + '.exit'), 'exit_code=7\nmarker_end\n'); expect(s.watch.status(s.root)).toEqual({ state: 'ended', session_id: s.id, exit_code: 7 }); });
test('存活 pid 保持运行', () => { const s = setup(); utimesSync(join(s.dir, s.id + '.prompt'), 1, 1); writeFileSync(join(s.dir, s.id + '.pid'), String(process.pid)); expect(s.watch.status(s.root).state).toBe('running'); });
test('死亡 pid 与十秒边界', () => { const s = setup(); writeFileSync(join(s.dir, s.id + '.pid'), '2147483647'); const now = Date.now(); vi.spyOn(Date, 'now').mockReturnValue(now); try { for (const [age, state] of [[9999, 'running'], [10000, 'interrupted'], [10001, 'interrupted']] as const) { utimesSync(join(s.dir, s.id + '.prompt'), new Date(now - age), new Date(now - age)); expect(s.watch.status(s.root).state).toBe(state); } } finally { vi.restoreAllMocks(); } });
test('最新会话、无目录、无效 id、读盘失败', () => { const s = setup(); utimesSync(join(s.dir, s.id + '.prompt'), 1, 1); const next = 'ses-' + 'b'.repeat(32); writeFileSync(join(s.dir, next + '.prompt'), ''); writeFileSync(join(s.dir, 'invalid.prompt'), ''); expect(s.watch.status(s.root).session_id).toBe(next); expect(s.watch.status(join(s.root, 'missing'))).toEqual({ state: 'none' }); mkdirSync(join(s.dir, next + '.exit')); expect(s.watch.status(s.root)).toMatchObject({ state: 'interrupted', error: expect.any(String) }); });
