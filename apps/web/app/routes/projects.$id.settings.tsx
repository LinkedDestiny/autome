import { configWarning } from '../lib/config-warnings.ts';
import { useState } from 'react';
import { Form, redirect, useLoaderData, useActionData, useNavigation, type LoaderFunctionArgs, type ActionFunctionArgs } from 'react-router';
import { servicesContext } from '../load-context.ts';
import { loadSettings, artefactPaths, type SettingsData, type Artefact, type OnboardingResult } from '../components/settings-data.ts';
import { ADMIN, ONBOARDING, SETTINGS } from '../lib/admin-labels.ts';
import { MAX_DOCUMENT_LENGTH } from '../../server/gates/write-gate.ts';
import { useWriteGate } from '../lib/live.tsx';
import { byKind, liveShouldRevalidate } from '../lib/live-routes.ts';
export const shouldRevalidate = liveShouldRevalidate(byKind('project.', 'session.'));
export function loader({ context, params }: LoaderFunctionArgs) { return loadSettings(context.get(servicesContext), params.id ?? ''); }
export async function action({ request, context, params }: ActionFunctionArgs): Promise<OnboardingResult | Response> {
  const form = await request.formData();
  const intent = form.get('intent');
  const { gateway, admin } = context.get(servicesContext);
  if (!gateway) return { error: ADMIN.offline };
  const project_id = params.id ?? '';
  if (intent === 'remove') {
    const listed = await gateway.read({ method: 'project.list' });
    if (!listed.body.ok) return { error: listed.body.error };
    const project = (listed.body.payload as { projects: { project: { id: string; display_name: string } }[] }).projects.find(p => p.project.id === project_id)?.project;
    if (!project) return { error: ONBOARDING.missing };
    if (form.get('name') !== project.display_name) return { error: SETTINGS.mismatch };
    const removed = await gateway.write({ op: 'project.remove', params: { project_id } });
    return removed.body.ok ? redirect('/') : { error: removed.body.error };
  }
  if (intent === 'save') {
    const path = form.get('path');
    if (typeof path !== 'string' || !artefactPaths.includes(path)) return { error: ONBOARDING.invalidPath };
    const result = await gateway.write({ op: 'project.onboarding.save', params: { project_id, path, content: form.get('content') } });
    return result.body.ok ? { saved: true } : { error: result.body.error };
  }
  if (intent === 'run') {
    const loaded = await loadSettings({ gateway, admin }, project_id);
    if (!loaded.project) return { error: loaded.error };
    const result = await gateway.write({ op: 'project.onboarding.run', params: { project_id } });
    if (!result.body.ok) return { error: result.body.error };
    admin?.onboarding.watch(project_id, loaded.project.path, (result.body.payload as { session_id: string }).session_id);
    return {};
  }
  if (intent !== 'finish' && intent !== 'skip') return { error: ADMIN.unknown };
  for (let i = 0; i < 3; i++) {
    const result = await gateway.write({ op: `project.onboarding.${intent === 'finish' ? 'advance' : 'skip'}`, params: { project_id } });
    if (!result.body.ok) return { error: result.body.error };
    const payload = result.body.payload as { onboarding: { onboarding: string }; init_commit: string | null };
    if (payload.onboarding.onboarding !== 'in_progress') return { init_commit: payload.init_commit };
  }
  return { error: ONBOARDING.unfinished };
}
function Editor({ file, disabled, reason }: { file: Artefact; disabled: boolean; reason?: string }) {
  const [content, setContent] = useState(file.content ?? '');
  return <Form method="post" className="onboarding-editor"><input type="hidden" name="intent" value="save" /><input type="hidden" name="path" value={file.path} />
    <label htmlFor={file.path}>{file.path}</label><textarea id={file.path} name="content" value={content} onChange={event => setContent(event.target.value)} />
    <span>{ONBOARDING.count(content.length, MAX_DOCUMENT_LENGTH)}</span><button disabled={disabled} title={reason}>{ONBOARDING.save}</button></Form>;
}
export function SettingsView({ data, result, busy = false }: { data: SettingsData; result?: OnboardingResult; busy?: boolean }) {
  const gate = useWriteGate();
  const [name, setName] = useState('');
  if (!data.project) return <p role="alert">{data.error}</p>;
  const state = data.project.onboarding;
  const active = state.onboarding === 'in_progress';
  const disabled = !gate.allowed || busy;
  const session = data.session;
  const status = session.state === 'running' ? ONBOARDING.running : session.state === 'ended' ? ONBOARDING.exit(session.exit_code) : session.state === 'interrupted' ? ONBOARDING.interrupted : ONBOARDING.none;
  return <div className="project-settings"><h1>{data.project.display_name} · {ADMIN.settings}</h1><section className="settings-card-body"><h2>{ONBOARDING.title}</h2>
    <p>{active ? ONBOARDING.step(state.step!) : state.onboarding === 'skipped' ? ONBOARDING.skipped : ONBOARDING.ended}</p>
    {active && <p>{ONBOARDING.descriptions[state.step!]}</p>}<p role="status">{status} {session.session_id}</p>{session.error && <p role="alert">{session.error}</p>}
    {result?.error && <p role="alert">{result.error}</p>}{result?.saved && <p role="status">{ONBOARDING.saved}</p>}
    {result && 'init_commit' in result && <p role="status">{result.init_commit ? ONBOARDING.commit(result.init_commit) : ONBOARDING.noCommit}</p>}
    {active && <><Form method="post"><button name="intent" value="run" disabled={disabled || session.state === 'running'} title={gate.reason}>{ONBOARDING.run}</button></Form>
      {data.artefactsError && <p role="alert">{data.artefactsError}</p>}
      {data.files.map(file => <Editor key={file.path + file.content} file={file} disabled={disabled} reason={gate.reason} />)}
      <Form method="post"><button name="intent" value="finish" disabled={disabled} title={gate.reason}>{ONBOARDING.finish}</button><button name="intent" value="skip" disabled={disabled} title={gate.reason}>{ONBOARDING.skip}</button></Form></>}
  </section>
  <section className="settings-card-body"><h2>{SETTINGS.config}</h2>
    {data.config?.error ? <p role="alert">{data.config.error}</p> : <>
      {data.config?.paths.map(path => <p key={path}><code>{path}</code></p>)}
      <dl className="config-rows">{data.config?.rows.map(row => <div key={row.key} data-config-key={row.key}><dt>{row.label}</dt><dd>{row.value}</dd></div>)}</dl>
      {data.config?.warnings.map((warning, i) => <pre key={i}>{configWarning(warning)}</pre>)}
    </>}
  </section>
  <section className="settings-card-body"><h2>{SETTINGS.remove}</h2><p>{SETTINGS.warning}</p><p><code>{data.project.path}</code></p>
    <Form method="post"><input type="hidden" name="intent" value="remove" /><label htmlFor="remove-name">{SETTINGS.name}（{data.project.display_name}）</label>
      <input id="remove-name" name="name" value={name} onChange={event => setName(event.target.value)} autoComplete="off" />
      <button disabled={disabled || name !== data.project.display_name} title={gate.reason}>{SETTINGS.remove}</button>
    </Form>
  </section></div>;
}
export default function Settings() { return <SettingsView data={useLoaderData<typeof loader>()} result={useActionData<OnboardingResult>()} busy={useNavigation().state !== 'idle'} />; }
