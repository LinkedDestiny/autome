import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { createGateway, LocalOpError } from './gateway.ts';
import type { SkillEntry } from '../app/lib/skills-model.ts';
export type PathOpener = (path: string) => Promise<void>;
export const macPathOpener: PathOpener = async path => { await promisify(execFile)('/usr/bin/open', [path]); };
export function registerOpenPath(gateway: ReturnType<typeof createGateway>, opener: PathOpener = macPathOpener) {
  gateway.registerLocalOp('open.path', async params => {
    if (params.kind !== 'skill') throw new LocalOpError(501, '网页端还不支持此打开类型');
    if (Object.keys(params).some(k => !['kind', 'name', 'project_id', 'source'].includes(k)) || typeof params.name !== 'string' || !params.name || (params.project_id !== undefined && (typeof params.project_id !== 'string' || !params.project_id))) throw new LocalOpError(400, '技能参数无效');
    const source = params.source ?? 0;
    if (typeof source !== 'number' || !Number.isSafeInteger(source) || source < 0) throw new LocalOpError(400, '来源序号无效');
    const reply = await gateway.core.call('skills.list', params.project_id ? { project_id: params.project_id } : {}) as { skills: SkillEntry[] };
    const skill = reply.skills.find(s => s.name === params.name);
    if (!skill) throw new LocalOpError(404, '没有这个技能');
    const path = skill.sources[source]?.path;
    if (!path) throw new LocalOpError(404, '没有这个来源');
    try { await opener(path); } catch (error) { throw new LocalOpError(503, `打开目录失败：${error instanceof Error ? error.message : String(error)}`); }
    return { opened: true };
  });
}
