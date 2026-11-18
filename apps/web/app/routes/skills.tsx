import { Drawer } from '../components/ui/drawer.tsx';
import { useState } from 'react';
import { useFetcher, useLoaderData, Link, type LoaderFunctionArgs, type ActionFunctionArgs } from 'react-router';
import { Ribbon } from '../components/ui/ribbon.tsx';
import { Check } from '../components/ui/check.tsx';
import { Segmented, SegmentedTabs } from '../components/ui/segmented.tsx';
import { TEST_IDS } from '../lib/testids.ts';
import { byKind, liveShouldRevalidate } from '../lib/live-routes.ts';
import { servicesContext } from '../load-context.ts';
import { useWriteGate } from '../lib/live.tsx';
import { loadSkills, saveBinding, type SkillsData, type BindingResult } from '../components/skills-data.ts';
import { resolvedRoles } from '../components/loop-data.ts';
import { roleNames, roleOrder, type Role, type Roles } from '../lib/role-rules.ts';
import { boundRolesLabel, bindingReason, filterSkills, skillCounts, scopedSkill, type SkillEntry, type SkillFilter, type BindingScope } from '../lib/skills-model.ts';
export const handle = { crumb: '技能' };
export const shouldRevalidate = liveShouldRevalidate(byKind('config.', 'project.', 'skills.'));
export function loader({ request, context }: LoaderFunctionArgs) { return loadSkills(context.get(servicesContext), new URL(request.url).searchParams.get('project') || undefined); }
export async function action({ request, context }: ActionFunctionArgs): Promise<BindingResult> {
  const form = await request.formData(), services = context.get(servicesContext);
  const project = new URL(request.url).searchParams.get('project') || undefined;
  if (form.get('intent') === 'open') {
    const reply = await services.gateway?.write({ op: 'open.path', params: { kind: 'skill', name: String(form.get('name')), source: Number(form.get('source')), ...(project ? { project_id: project } : {}) } });
    return { saved: [], messages: reply?.body.ok ? ['已打开目录'] : [], error: reply?.body.ok ? undefined : reply?.body.error ?? '内核不可用' };
  }
  return saveBinding(services, project, String(form.get('scope')) as BindingScope, String(form.get('name') ?? ''), form.getAll('roles').map(String) as Role[]);
}
export function SkillCard({ skill, roles, onSelect }: { skill: SkillEntry; roles?: Roles; onSelect: () => void }) {
  return <button type="button" className={`card card-pat skill-card card-${skill.conflicts.length ? 'red' : skill.bound_roles.length ? 'green' : 'blue'}`} data-testid={TEST_IDS.skillCard} onClick={onSelect}>
    <strong title={skill.name}>{skill.name}</strong><span className="skill-description">{skill.description?.trim() || '暂无描述'}</span><span className="skill-path"><span title={skill.sources[0]?.path}>{skill.sources[0]?.path}</span>{skill.sources.length > 1 && <small>另 {skill.sources.length - 1} 处</small>}</span>
    <span className="skill-badges">{[...new Set(skill.sources.map(s => s.scope))].map(s => <span key={s} className="pill">{s === 'global' ? '全局' : '项目'}</span>)}{skill.visible_to.map(r => <span className="pill" key={r}>{r === 'claude' ? 'Claude Code' : 'Codex'}</span>)}</span>
    <span className={`pill skill-binding${skill.bound_roles.length ? ' pill-green pill-fill' : ''}`} title={boundRolesLabel(skill.bound_roles)}>{boundRolesLabel(skill.bound_roles)}</span>
    {skill.conflicts.map(r => <span key={r}>{roleNames[r]} 用 {roles?.[r].runtime === 'claude' ? 'Claude Code' : 'Codex'}，看不到它</span>)}
  </button>;
}
function SkillDescription({ text }: { text: string }) {
  const [open, setOpen] = useState(false);
  return <><p className={open ? 'skill-drawer-description' : 'skill-drawer-description skill-drawer-clamp'}>{text}</p>{(text.length > 120 || text.includes('\n')) && <button type="button" className="btn btn-sm btn-text skill-expand" aria-expanded={open} onClick={() => setOpen(v => !v)}>{open ? '收起' : '展开'}</button>}</>;
}
export function SkillDrawer({ skill, scope, roles, selected, project, disabled, onScope, onRoles, onSave, onOpen, onClose, result }: {
  skill: SkillEntry; scope: BindingScope; roles: Roles; selected: Role[]; project?: string; disabled: boolean;
  onScope: (s: BindingScope) => void; onRoles: (r: Role[]) => void; onSave: () => void; onOpen: (source: number) => void; onClose: () => void; result?: BindingResult;
}) {
  return <Drawer className="skills-drawer" label={`绑定 ${skill.name}`} title={skill.name} onClose={onClose} footer={<>    <button className="btn btn-primary" disabled={disabled} onClick={onSave}>保存绑定</button>
    {result?.error && <p role="alert">{result.error}</p>}{result && !result.error && <p role="status">{result.saved.length ? '绑定已保存' : '无需修改绑定'}</p>}{result?.messages.map(m => <p key={m} role="status">{m}</p>)}
</>}><h3>绑定范围</h3>
    <Segmented name="binding-scope" label="绑定范围" value={scope} disabled={disabled} onChange={v => onScope(v as BindingScope)} options={[{ value: 'global', label: '全局 · 所有项目', disabled: !scopedSkill(skill, 'global') }, { value: 'project', label: '仅当前项目', disabled: !project }]} />
    {!scopedSkill(skill, 'global') && <p>项目技能只能绑定在自己的项目里</p>}
    <p>{scope === 'project' ? '此后这个角色在本项目里不再跟随全局技能列表的变化；要恢复，去 Loop 配置点「恢复跟随全局」。' : '某个项目如果给这个角色写了自己的技能列表，就不会跟随这条全局绑定；保存后会列出没有拿到的项目。'}</p>
    <fieldset disabled={disabled} className="skill-roles"><legend>哪些角色必须使用</legend>{roleOrder.map(role => { const reason = bindingReason(skill, scope, roles, role); return <Check key={role} label={roleNames[role]} checked={selected.includes(role)} {...(reason ? { disabled: true, reason } : { disabled: false })} onChange={e => onRoles(e.target.checked ? [...selected, role] : selected.filter(r => r !== role))} />; })}</fieldset>
    <h3>描述</h3><SkillDescription text={skill.description?.trim() || '暂无描述'} />
    <h3>来源</h3>{skill.sources.map((s, i) => <div className="skill-source" key={i}><span>{s.scope === 'global' ? '全局' : '项目'} · {s.runtime === 'claude' ? 'Claude Code' : 'Codex'}</span><div className="skill-source-path"><code title={s.path}>{s.path}</code><button className="btn btn-sm" disabled={disabled} onClick={() => onOpen(i)}>打开目录</button></div></div>)}
  </Drawer>;
}
function BindingEditor({ skill, data, onClose }: { skill: SkillEntry; data: SkillsData; onClose: () => void }) {
  const fetcher = useFetcher<BindingResult>(), gate = useWriteGate();
  const [scope, setScope] = useState<BindingScope>(data.project ? 'project' : 'global');
  const roles = resolvedRoles((scope === 'global' ? data.globalConfig : data.config)!);
  const [selection, setSelection] = useState<Role[] | null>(null);
  const selected = selection ?? roleOrder.filter(r => roles[r].skills.includes(skill.name));
  const disabled = !gate.allowed || fetcher.state !== 'idle' || !!data.error;
  const save = () => { const form = new FormData(); form.set('name', skill.name); form.set('scope', scope); selected.forEach(r => form.append('roles', r)); void fetcher.submit(form, { method: 'post' }); };
  return <SkillDrawer skill={skill} scope={scope} roles={roles} selected={selected} project={data.project} disabled={disabled} onScope={s => { setScope(s); setSelection(null); }} onRoles={setSelection} onSave={save} onOpen={source => { void fetcher.submit({ intent: 'open', name: skill.name, source }, { method: 'post' }); }} onClose={onClose} result={fetcher.data} />;
}
export function SkillsView({ data }: { data: SkillsData }) {
  const [filter, setFilter] = useState<SkillFilter>('all'), [query, setQuery] = useState(''), [editing, setEditing] = useState<string | null>(null);
  const counts = skillCounts(data.skills), roles = data.config && resolvedRoles(data.config);
  const skill = data.skills.find(s => s.name === editing);
  return <div className="skills-page" data-testid={TEST_IDS.pageSkills}><header><Ribbon color="green">技能</Ribbon><SegmentedTabs label="技能范围"><Link to="/skills" aria-current={!data.project ? 'page' : undefined}>全局</Link>{data.projects.map(p => <Link key={p.id} to={`/skills?project=${encodeURIComponent(p.id)}`} aria-current={data.project === p.id ? 'page' : undefined}>{p.display_name}</Link>)}</SegmentedTabs><label>搜索技能<input type="search" value={query} onChange={e => setQuery(e.target.value)} placeholder="名称或路径" /></label></header>
    <div className="skills-filter"><span className="muted">筛选</span><Segmented name="skills-filter" label="技能筛选" value={filter} onChange={v => setFilter(v as SkillFilter)} options={(['all', 'global', ...(data.project ? ['project'] : []), 'bound'] as SkillFilter[]).map(value => ({ value, label: `${({ all: '全部', global: '全局', project: '本项目', bound: '已绑定' })[value]} ${counts[value]}` }))} /></div>
    {data.error && <p role="alert">{data.error}</p>}<div className="skills-grid" data-scroll-region>{filterSkills(data.skills, filter, query).map(s => <SkillCard key={s.name} skill={s} roles={roles} onSelect={() => setEditing(s.name)} />)}{!filterSkills(data.skills, filter, query).length && <p>没有符合条件的技能</p>}</div>
    {skill && data.config && data.globalConfig && <BindingEditor key={skill.name} skill={skill} data={data} onClose={() => setEditing(null)} />}</div>;
}
export default function Page() { const data = useLoaderData<typeof loader>(); return <SkillsView key={data?.project ?? 'global'} data={data ?? { projects: [], skills: [] }} />; }
