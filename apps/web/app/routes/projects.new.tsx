import { useEffect, useState } from 'react';
import { Form, Link, redirect, useActionData, useLoaderData, useNavigation, type ActionFunctionArgs, type LoaderFunctionArgs } from 'react-router';
import { servicesContext } from '../load-context.ts';
import { ADMIN } from '../lib/admin-labels.ts';
import { useWriteGate } from '../lib/live.tsx';
import { canConfirm, confirmDefaults, probeView, type AddResult, type CheckedProject, type Probe } from '../components/add-project-model.ts';
import { nativePicker, type PickerWindow } from '../components/native-picker.ts';
import { pathCompleteClient, type Suggestions } from '../components/path-complete-client.ts';

export async function loader({ request, context }: LoaderFunctionArgs): Promise<CheckedProject> {
  const input = new URL(request.url).searchParams.get('path');
  if (input === null) return { display: '' };
  const { gateway, admin } = context.get(servicesContext);
  if (!admin) return { display: input, error: ADMIN.unavailable };
  const checked = admin.paths.check(input);
  if (!checked.ok) return { display: input, error: checked.message };
  if (!gateway) return { display: checked.display, error: ADMIN.offline };
  const result = await gateway.read({ method: 'project.probe', params: { path: checked.display } });
  return result.body.ok ? { display: checked.display, probe: result.body.payload as Probe } : { display: checked.display, error: result.body.error };
}
export async function action({ request, context }: ActionFunctionArgs): Promise<AddResult | Response> {
  const form = await request.formData();
  if (form.get('intent') !== 'confirm') return { error: ADMIN.unknown };
  const { gateway } = context.get(servicesContext);
  if (!gateway) return { error: ADMIN.offline };
  const workspace = form.get('workspace') === 'on', skip = form.get('skip_onboarding') === 'on';
  const result = await gateway.write({ op: 'project.confirm', params: { path: form.get('path'), workspace,
    ...(workspace ? { docs_repo: form.get('docs_repo') } : {}), skip_onboarding: skip } });
  if (!result.body.ok) return { error: result.body.error };
  const added = result.body.payload as AddResult & { project: { id: string } };
  if (added.skip_error) return added;
  return redirect(`/projects/${encodeURIComponent(added.project.id)}${skip ? '' : '/settings'}`);
}
export function AddProjectView({ checked, result, busy = false }: { checked: CheckedProject; result?: AddResult; busy?: boolean }) {
  const gate = useWriteGate();
  const [input, setInput] = useState(checked.display);
  const [workspace, setWorkspace] = useState(checked.probe?.suggest_workspace ?? false);
  const [suggestions, setSuggestions] = useState<Suggestions>({ entries: [], truncated: false });
  const [picker, setPicker] = useState<ReturnType<typeof nativePicker>>();
  const [pickerError, setPickerError] = useState('');
  useEffect(() => { setPicker(() => nativePicker(window as PickerWindow)); }, []);
  useEffect(() => { const client = pathCompleteClient(fetch, setSuggestions); client.input(input); return client.dispose; }, [input]);
  const disabled = !gate.allowed || busy || Boolean(result?.project);
  const defaults = checked.probe && confirmDefaults(checked.probe);
  return <div className="add-project"><h1>{ADMIN.add}</h1><div className="add-project-body">
    <Form method="get"><label htmlFor="project-path">{ADMIN.path}</label>
      <input id="project-path" name="path" value={input} maxLength={512} placeholder="~/code/project" disabled={busy || Boolean(result?.project)} onChange={event => setInput(event.target.value)} />
      <button disabled={disabled} title={gate.reason}>{busy ? ADMIN.checking : ADMIN.check}</button>
      {picker && <button type="button" disabled={disabled} onClick={async () => { try { const path = await picker(); if (path) setInput(path); } catch (error) { setPickerError(String(error)); } }}>{ADMIN.picker}</button>}
    </Form>
    <ul className="path-suggestions">{suggestions.entries.map(entry => <li key={entry.path}><button type="button" disabled={busy || Boolean(result?.project)} onClick={() => setInput(entry.path)}>{entry.name}</button></li>)}</ul>
    {suggestions.truncated && <p>{ADMIN.more}</p>}{suggestions.error && <p role="alert">{suggestions.error}</p>}{pickerError && <p role="alert">{pickerError}</p>}
    {checked.error && <p role="alert">{checked.error}</p>}
    {checked.probe && <section><p className="probe-path">{checked.probe.path}</p><div className="probe-members">{probeView(checked.probe).map(text => <p key={text}>{text}</p>)}</div>
      {!workspace && checked.probe.members.length > 0 && <p>{ADMIN.nested}</p>}
      {canConfirm(input, checked) && defaults && !result?.project && <Form method="post" className="project-confirm">
        <input type="hidden" name="intent" value="confirm" /><input type="hidden" name="path" value={checked.display} />
        <label><input type="checkbox" name="workspace" checked={workspace} onChange={event => setWorkspace(event.target.checked)} />{ADMIN.workspace}</label>
        <label>{ADMIN.docs}<select name="docs_repo" disabled={!workspace} defaultValue={defaults.docs_repo}>{defaults.members.map(name => <option key={name} value={name}>{name}</option>)}</select></label>
        <label><input type="checkbox" name="skip_onboarding" />{ADMIN.skip}</label><button className="btn-primary" disabled={disabled} title={gate.reason}>{ADMIN.confirm}</button>
      </Form>}
    </section>}
    {result?.error && <p role="alert">{result.error}</p>}
    {result?.skip_error && result.project && <section role="alert"><p>{ADMIN.partial}{result.skip_error}</p><Link to={`/projects/${encodeURIComponent(result.project.id)}`}>{ADMIN.project}</Link> <Link to={`/projects/${encodeURIComponent(result.project.id)}/settings`}>{ADMIN.settings}</Link></section>}
    {!gate.allowed && <p role="status">{gate.reason}</p>}
  </div></div>;
}
export default function AddProject() {
  const checked = useLoaderData<typeof loader>();
  const result = useActionData<typeof action>();
  const navigation = useNavigation();
  return <AddProjectView key={JSON.stringify(checked)} checked={checked} result={result} busy={navigation.state !== 'idle'} />;
}
