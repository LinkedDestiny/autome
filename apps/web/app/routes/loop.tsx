import { LOOP, effortOptions } from '../lib/loop-labels.ts';
import { useState, useEffect } from 'react';
import { Link, useLoaderData, useFetcher, type LoaderFunctionArgs, type ActionFunctionArgs } from 'react-router';
import { Card, type CardColor } from '../components/ui/card.tsx';
import { Ribbon } from '../components/ui/ribbon.tsx';
import { Switch } from '../components/ui/switch.tsx';
import { Segmented } from '../components/ui/segmented.tsx';
import { TEST_IDS } from '../lib/testids.ts';
import { byKind, liveShouldRevalidate } from '../lib/live-routes.ts';
import { servicesContext } from '../load-context.ts';
import { useWriteGate } from '../lib/live.tsx';
import { loadLoop, resolvedRoles, saveLoop, type LoopData, type SaveResult } from '../components/loop-data.ts';
import { roleOrder, roleNames, roleRules, saveOrder, changedFields, type Role, type RoleConfig } from '../lib/role-rules.ts';
export const handle = { crumb: 'Loop 配置' };
export const shouldRevalidate = liveShouldRevalidate(byKind('config.', 'project.'));
export const loader = ({ context, request }: LoaderFunctionArgs) => loadLoop(context.get(servicesContext), new URL(request.url).searchParams.get('project') || undefined);
export async function action({ context, request }: ActionFunctionArgs): Promise<SaveResult> {
  const form = await request.formData();
  try { return await saveLoop(context.get(servicesContext), new URL(request.url).searchParams.get('project') || undefined, JSON.parse(String(form.get('changes'))), form.get('intent') === 'reset'); }
  catch { return { saved: [], error: '配置请求无效' }; }
}
export const colors: Record<Role, CardColor> = { plan: 'purple', review: 'blue', adjudicate: 'yellow', impl: 'teal', audit: 'red', retro: 'green' };
export const resetMessage = '这个角色的工具、模型、Effort、技能、启用的项目覆盖会一起清掉';
export function LoopView({ data }: { data: LoopData }) {
  const fetcher = useFetcher<SaveResult>(), gate = useWriteGate();
  const [drafts, setDrafts] = useState<Partial<Record<Role, RoleConfig>>>({});
  const [editing, setEditing] = useState<Role | null>(null);
  useEffect(() => { if (fetcher.state === 'idle' && fetcher.data) setDrafts(old => Object.fromEntries(Object.entries(old).filter(([role]) => !fetcher.data!.saved.includes(role as Role)))); }, [fetcher.state, fetcher.data]);
  const base = data.config && resolvedRoles(data.config);
  const merged = base && { ...base, ...drafts };
  const violations = merged ? roleRules(merged, data.skills) : [];
  const patches = base ? Object.fromEntries(roleOrder.filter(r => drafts[r] && Object.keys(changedFields(base[r], drafts[r]!)).length).map(r => [r, data.project ? changedFields(base[r], drafts[r]!) : drafts[r]!])) : {};
  const order = base ? saveOrder(base, patches, data.skills) : null;
  const disabled = !gate.allowed || fetcher.state !== 'idle' || !!data.error;
  const edit = (role: Role, fields: Partial<RoleConfig>) => setDrafts(old => ({ ...old, [role]: { ...base![role], ...old[role], ...fields } }));
  const submit = (roles: Role[], reset = false) => { void fetcher.submit({ changes: JSON.stringify(roles.map(role => ({ role, fields: reset ? {} : patches[role] }))), intent: reset ? 'reset' : 'save' }, { method: 'post' }); };
  const roleCard = (role: Role) => {
    const cfg = merged![role], error = violations.some(v => v.roles.includes(role));
    return <Card key={role} color={colors[role]} patterned className={error ? 'loop-role loop-conflict' : 'loop-role'} data-role={role} data-testid={TEST_IDS.loopRole}>
      <button type="button" onClick={() => setEditing(role)}><h3>{roleNames[role]}</h3></button><p>{cfg.runtime === 'claude' ? 'Claude Code' : 'Codex'}</p><strong>{cfg.model || 'CLI 默认'}</strong><p className="pill">{cfg.effort ?? 'Effort 默认'}</p><p>{cfg.skills.length} 个绑定技能</p>
      {data.project && <p>{data.config!.resolved.roles.find(r => r.role === role)?.provenance === 'project' ? '项目覆盖' : '跟随全局'}</p>}
      <Switch label={`${roleNames[role]}启用`} checked={cfg.enabled} disabled={disabled} onChange={e => edit(role, { enabled: e.target.checked })} />
      {!cfg.enabled && <small>已停用：Autome 不再为这个角色启动会话，双模型检查对这一对免检</small>}
    </Card>;
  };
  return <div className="loop-page" data-testid={TEST_IDS.pageLoop}><header><Ribbon color="blue">Loop 配置</Ribbon><nav className="seg" aria-label="配置范围"><Link aria-current={!data.project ? 'page' : undefined} to="/loop">全局默认</Link>{data.projects.map(p => <Link key={p.id} aria-current={data.project === p.id ? 'page' : undefined} to={`/loop?project=${encodeURIComponent(p.id)}`}>{p.display_name}</Link>)}</nav>{Object.keys(patches).length > 0 && <button disabled={disabled || violations.length > 0 || !order} onClick={() => submit(order!)}>保存 {Object.keys(patches).length} 项改动</button>}</header>
      <p>{data.project ? '仅覆盖当前项目；未改过的字段继续跟随全局。' : '全局默认，所有未覆盖的项目都会使用。'}</p>
      {data.error && <p role="alert">{data.error}</p>}{fetcher.data?.error && <p role="alert">{fetcher.data.error}</p>}
      {violations.map((v, i) => <p role="alert" key={i}>{v.message}</p>)}{!order && Object.keys(patches).length > 0 && <p role="alert">这几项必须分开保存：先把冲突角色改成别的模型。</p>}
      {merged && <div className="loop-body" data-scroll-region><h2>{LOOP.design} <small>最多 {data.config!.resolved.loop_defaults.design_rounds} 轮</small></h2><div className="loop-row"><Card dashed><h3>{LOOP.intake}</h3><p>固定</p></Card>{roleOrder.slice(0, 3).map(roleCard)}<Card color="yellow" dashed><h3>{LOOP.approve}</h3><p>人工停顿</p></Card></div><h2>{LOOP.implementation} <small>预算 = {data.config!.resolved.loop_defaults.budget_factor} × 里程碑数</small></h2><div className="loop-row">{roleCard('impl')}<Card dashed><h3>宿主快检</h3><p>固定</p></Card>{roleCard('audit')}{roleCard('retro')}<Card dashed><h3>{LOOP.mergeGate}</h3><p>固定</p></Card></div></div>}
      {editing && merged && <aside className="loop-drawer" role="dialog" aria-modal="false" aria-label={`编辑${roleNames[editing]}`}><button onClick={() => setEditing(null)}>关闭</button><h2>编辑{roleNames[editing]}</h2>{fetcher.data?.error && <p role="alert">{fetcher.data.error}</p>}<Switch label="启用" checked={merged[editing].enabled} disabled={disabled} onChange={e => edit(editing, { enabled: e.target.checked })} /><Segmented name="runtime" label="工具" options={[{ value: 'claude', label: 'Claude Code' }, { value: 'codex', label: 'Codex' }]} value={merged[editing].runtime} onChange={runtime => edit(editing, { runtime: runtime as RoleConfig['runtime'] })} disabled={disabled} /><label>模型<input value={merged[editing].model} disabled={disabled} onChange={e => edit(editing, { model: e.target.value })} /></label><Segmented name="effort" label="Effort" options={effortOptions(merged[editing].effort)} value={merged[editing].effort ?? ''} onChange={effort => edit(editing, { effort: effort || null })} disabled={disabled} /><p>绑定的技能</p>{merged[editing].skills.map(name => <button key={name} disabled={disabled} onClick={() => edit(editing, { skills: merged[editing].skills.filter(s => s !== name) })}>{name} ×</button>)}<Link to={data.project ? `/skills?project=${encodeURIComponent(data.project)}` : '/skills'}>添加技能</Link><button disabled={disabled || violations.length > 0 || !patches[editing] || !saveOrder(base!, { [editing]: patches[editing] }, data.skills)} onClick={() => submit([editing])}>保存</button>{data.project && <button disabled={disabled} onClick={() => { if (window.confirm(resetMessage)) submit([editing], true); }}>恢复跟随全局</button>}</aside>}
    </div>;
}
export default function Page() { const data = useLoaderData<typeof loader>(); return <LoopView key={data?.project ?? 'global'} data={data ?? { projects: [], skills: [] }} />; }
