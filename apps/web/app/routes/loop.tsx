import { Drawer } from '../components/ui/drawer.tsx';
import type { OnboardingResult } from '../components/settings-data.ts';
import { LoopDefaultsView } from '../components/loop-defaults.tsx';
import { saveDefault } from '../components/loop-defaults-data.ts';
import { loader as settingsLoader, action as settingsAction, SettingsView } from '../components/settings-page.tsx';
export { settingsLoader, settingsAction, SettingsView };
import { LOOP, effortOptions } from '../lib/loop-labels.ts';
import { useState, useEffect } from 'react';
import { Link, useLoaderData, useFetcher, useActionData, useNavigation, type LoaderFunctionArgs, type ActionFunctionArgs } from 'react-router';
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
export const shouldRevalidate = liveShouldRevalidate(byKind('config.', 'project.', 'session.'));
export async function loader(args: LoaderFunctionArgs): Promise<LoopData> {
  const query = new URL(args.request.url).searchParams;
  const data = await loadLoop(args.context.get(servicesContext), query.get('project') || undefined);
  if (data.project && query.get('tab') === 'project') data.settings = await settingsLoader(args);
  return data;
}
export async function action(args: ActionFunctionArgs): Promise<SaveResult | OnboardingResult | Response> {
  const { context, request } = args;
  const query = new URL(request.url).searchParams;
  if (query.get('project') && query.get('tab') === 'project') return settingsAction(args);
  const form = await request.formData();
  if (form.get('intent') === 'default') return saveDefault(context.get(servicesContext), query.get('project') || undefined, String(form.get('key')), String(form.get('value')));
  try { return await saveLoop(context.get(servicesContext), new URL(request.url).searchParams.get('project') || undefined, JSON.parse(String(form.get('changes'))), form.get('intent') === 'reset'); }
  catch { return { saved: [], error: '配置请求无效' }; }
}
export const colors: Record<Role, CardColor> = { plan: 'purple', review: 'blue', adjudicate: 'yellow', impl: 'teal', audit: 'red', retro: 'green' };
export const resetMessage = '这个角色的工具、模型、Effort、技能、启用的项目覆盖会一起清掉';
export function LoopView({ data }: { data: LoopData }) {
  const fetcher = useFetcher<SaveResult>(), gate = useWriteGate();
  const result = useActionData<OnboardingResult>();
  const navigation = useNavigation();
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
      <div className="loop-role-head"><button type="button" aria-label={LOOP.edit + roleNames[role]} onClick={() => setEditing(role)}><span className="loop-role-name">{roleNames[role]}</span><span className="loop-role-edit">{LOOP.edit}</span></button>
        <Switch label={<span className="visually-hidden">{roleNames[role]}启用</span>} checked={cfg.enabled} disabled={disabled} onChange={e => edit(role, { enabled: e.target.checked })} />
      </div>
      <p>{cfg.runtime === 'claude' ? 'Claude Code' : 'Codex'}</p>
      <div className="loop-role-model"><span title={cfg.model || 'CLI 默认'}>{cfg.model || 'CLI 默认'}</span><span className="pill">{cfg.effort ?? 'Effort 默认'}</span></div>
      <div className="loop-role-badges"><span className="pill">{cfg.skills.length} 个技能</span>{data.project && <span className="pill">{data.config!.resolved.roles.find(r => r.role === role)?.provenance === 'project' ? '项目覆盖' : '跟随全局'}</span>}</div>
      {!cfg.enabled && <small className="loop-role-disabled" title={LOOP.disabled}>{LOOP.disabled}</small>}
    </Card>;
  };
  return <div className="loop-page" data-testid={TEST_IDS.pageLoop}><header><Ribbon color="blue">Loop 配置</Ribbon><nav className="seg" aria-label="配置范围"><Link aria-current={!data.project ? 'page' : undefined} to="/loop">全局默认</Link>{data.projects.map(p => <Link key={p.id} aria-current={data.project === p.id ? 'page' : undefined} to={`/loop?project=${encodeURIComponent(p.id)}`}>{p.display_name}</Link>)}</nav>{Object.keys(patches).length > 0 && <button disabled={disabled || violations.length > 0 || !order} onClick={() => submit(order!)}>保存 {Object.keys(patches).length} 项改动</button>}</header>
      {data.project && <nav className="seg" aria-label="项目配置页签"><Link to={`/loop?project=${encodeURIComponent(data.project)}`} aria-current={!data.settings ? 'page' : undefined}>Loop 配置</Link><Link to={`/loop?project=${encodeURIComponent(data.project)}&tab=project`} aria-current={data.settings ? 'page' : undefined}>项目信息</Link></nav>}
      {data.settings ? <SettingsView data={data.settings} result={result} busy={navigation.state !== 'idle'} /> : <>
      <p>{data.project ? '仅覆盖当前项目；未改过的字段继续跟随全局。' : '全局默认，所有未覆盖的项目都会使用。'}</p>
      {data.error && <p role="alert">{data.error}</p>}{fetcher.data?.error && <p role="alert">{fetcher.data.error}</p>}
      {violations.map((v, i) => <p role="alert" key={i}>{v.message}</p>)}{!order && Object.keys(patches).length > 0 && <p role="alert">这几项必须分开保存：先把冲突角色改成别的模型。</p>}
      {merged && <div className="loop-body" data-scroll-region><h2>{LOOP.design} <small>最多 {data.config!.resolved.loop_defaults.design_rounds} 轮</small></h2><div className="loop-row"><Card dashed><h3>{LOOP.intake}</h3><p>固定</p></Card>{roleOrder.slice(0, 3).map(roleCard)}<Card color="yellow" dashed><h3>{LOOP.approve}</h3><p>人工停顿</p></Card></div><h2>{LOOP.implementation} <small>预算 = {data.config!.resolved.loop_defaults.budget_factor} × 里程碑数</small></h2><div className="loop-row">{roleCard('impl')}<Card dashed><h3>宿主快检</h3><p>固定</p></Card>{roleCard('audit')}{roleCard('retro')}<Card dashed><h3>{LOOP.mergeGate}</h3><p>固定</p></Card></div><LoopDefaultsView config={data.config!} project={data.project} disabled={disabled} /></div>}
      {editing && merged && <RoleDrawer role={editing} config={merged[editing]} dirty={!!patches[editing]} overridden={data.config!.resolved.roles.find(r => r.role === editing)?.provenance === 'project'} project={data.project} disabled={disabled} saveDisabled={disabled || violations.length > 0 || !patches[editing] || !saveOrder(base!, { [editing]: patches[editing] }, data.skills)} onEdit={fields => edit(editing, fields)} onSave={() => submit([editing])} onReset={() => submit([editing], true)} onClose={() => setEditing(null)} result={fetcher.data} />}
    </>}</div>;
}
export default function Page() { const data = useLoaderData<typeof loader>(); return <LoopView key={data?.project ?? 'global'} data={data ?? { projects: [], skills: [] }} />; }

export function RoleDrawer({ role, config, dirty = false, overridden = true, project, disabled, saveDisabled, onEdit, onSave, onReset, onClose, result }: {
 role: Role; config: RoleConfig; dirty?: boolean; overridden?: boolean; project?: string; disabled: boolean; saveDisabled: boolean;
 onEdit: (fields: Partial<RoleConfig>) => void; onSave: () => void; onReset: () => void; onClose: () => void; result?: SaveResult;
}) {
 return <Drawer className="loop-drawer" label={`编辑${roleNames[role]}`} title={`编辑${roleNames[role]}`} onClose={onClose} footer={<><button disabled={saveDisabled} onClick={onSave}>保存</button>{project && <button disabled={disabled || !overridden} onClick={() => { if (window.confirm(resetMessage)) onReset(); }}>恢复跟随全局</button>}{result?.error && <p role="alert">{result.error}</p>}{result?.saved.includes(role) && <p role="status">已保存</p>}</>}><Switch label="启用" checked={config.enabled} disabled={disabled} onChange={e => onEdit( { enabled: e.target.checked })} /><Segmented name="runtime" label="工具" options={[{ value: 'claude', label: 'Claude Code' }, { value: 'codex', label: 'Codex' }]} value={config.runtime} onChange={runtime => onEdit( { runtime: runtime as RoleConfig['runtime'] })} disabled={disabled} /><label className="field"><span>模型</span><input value={config.model} disabled={disabled} onChange={e => onEdit( { model: e.target.value })} /></label><Segmented name="effort" label="Effort" options={effortOptions(config.effort)} value={config.effort ?? ''} onChange={effort => onEdit( { effort: effort || null })} disabled={disabled} /><div className="field"><span>绑定的技能</span>{config.skills.length === 0 && <p>{LOOP.noSkills}</p>}<div className="drawer-chips">{config.skills.map(name => <button className="pill" key={name} disabled={disabled} onClick={() => onEdit( { skills: config.skills.filter(s => s !== name) })}>{name} ×</button>)}</div></div><Link className="btn btn-sm" onClick={event => { if (dirty && !window.confirm('当前角色有未保存的改动，离开并添加技能？')) event.preventDefault(); }} to={project ? `/skills?project=${encodeURIComponent(project)}` : '/skills'}>{LOOP.addSkill}</Link></Drawer>;
}
