import { Input } from '../components/ui/input.tsx';
import { Check } from '../components/ui/check.tsx';
import { Ribbon } from '../components/ui/ribbon.tsx';
import { TEST_IDS } from '../lib/testids.ts';
import { useEffect, useState } from 'react';
import { Form, Link, redirect, useActionData, useLoaderData, useNavigation, type ActionFunctionArgs, type LoaderFunctionArgs } from 'react-router';
import { servicesContext } from '../load-context.ts';
import { ADMIN } from '../lib/admin-labels.ts';
import { useWriteGate } from '../lib/live.tsx';
import { byKind, liveShouldRevalidate } from '../lib/live-routes.ts';
import { canConfirm, confirmDefaults, probeView, type AddResult, type CheckedProject, type Probe } from '../components/add-project-model.ts';
import { nativePicker, type PickerWindow } from '../components/native-picker.ts';
import { pathCompleteClient, type Suggestions } from '../components/path-complete-client.ts';

const rejectionLabels: Record<string, string> = { already_project: '这个目录已经是项目', nested_in_project: '它在已有项目里面', contains_project: '它包含已有项目', not_a_directory: '这不是目录' };

export const shouldRevalidate = liveShouldRevalidate(byKind('project.'));

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
  return redirect(skip ? `/projects/${encodeURIComponent(added.project.id)}` : `/loop?project=${encodeURIComponent(added.project.id)}&tab=project`);
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
  return <div className="add-project" data-testid={TEST_IDS.pageProjectNew}><Ribbon><h1>{ADMIN.add}</h1></Ribbon><div className="add-project-body card card-pat card-blue">
    <Form method="get"><label htmlFor="project-path">{ADMIN.path}</label>
      <Input data-testid={TEST_IDS.pathInput} id="project-path" name="path" value={input} maxLength={512} placeholder="~/code/project" disabled={busy || Boolean(result?.project)} onChange={event => setInput(event.target.value)} />
      <button disabled={disabled} title={gate.reason}>{busy ? ADMIN.checking : ADMIN.check}</button>
      <DirectoryPicker picker={picker} disabled={disabled} onPick={setInput} onError={setPickerError} />
    </Form>
    <PathSuggestions entries={suggestions.entries} disabled={busy || Boolean(result?.project)} onSelect={setInput} />
    {suggestions.truncated && <p>{ADMIN.more}</p>}{suggestions.error && <p role="alert">{suggestions.error}</p>}{pickerError && <p role="alert">{pickerError}</p>}
    {checked.error && <p role="alert">{checked.error}</p>}
    {checked.probe && <section><p className="probe-path">{checked.probe.path}</p><div className="probe-members">{probeView(checked.probe).map(text => <p key={text}>{text}</p>)}</div>
      {checked.probe.rejection && <section role="alert"><h2>{Object.hasOwn(rejectionLabels, checked.probe.rejection.kind) ? rejectionLabels[checked.probe.rejection.kind] : ''}</h2><p>{checked.probe.rejection.message}</p>{checked.probe.rejection.project_id && <Link to={'/projects/' + encodeURIComponent(checked.probe.rejection.project_id)}>打开这个项目</Link>}</section>}
      {!workspace && checked.probe.members.length > 0 && <p>{ADMIN.nested}</p>}
      {canConfirm(input, checked) && defaults && !result?.project && <Form method="post" className="project-confirm">
        <input type="hidden" name="intent" value="confirm" /><input type="hidden" name="path" value={checked.display} />
        <Check name="workspace" checked={workspace} onChange={event => setWorkspace(event.target.checked)} label={ADMIN.workspace} />
        <label>{ADMIN.docs}<select className="inp" name="docs_repo" disabled={!workspace} defaultValue={defaults.docs_repo}>{defaults.members.map(name => <option key={name} value={name}>{name}</option>)}</select></label>
        <Check name="skip_onboarding" label={ADMIN.skip} /><button className="btn-primary" disabled={disabled} title={gate.reason}>{ADMIN.confirm}</button>
      </Form>}
    </section>}
    {result?.error && <p role="alert">{result.error}</p>}
    {result?.skip_error && result.project && <section role="alert"><p>{ADMIN.partial}{result.skip_error}</p><Link to={`/projects/${encodeURIComponent(result.project.id)}`}>{ADMIN.project}</Link> <Link to={`/loop?project=${encodeURIComponent(result.project.id)}&tab=project`}>{ADMIN.settings}</Link></section>}
    {!gate.allowed && <p role="status">{gate.reason}</p>}
  </div></div>;
}
export default function AddProject() {
  const checked = useLoaderData<typeof loader>();
  const result = useActionData<typeof action>();
  const navigation = useNavigation();
  return <AddProjectView key={JSON.stringify(checked)} checked={checked} result={result} busy={navigation.state !== 'idle'} />;
}

export function PathSuggestions({ entries, disabled, onSelect }: { entries: Suggestions['entries']; disabled: boolean; onSelect: (path: string) => void }) {
  return <ul className="path-suggestions" data-testid={TEST_IDS.pathSuggestions}>{entries.map(entry => <li key={entry.path}><button data-testid={TEST_IDS.pathSuggestion} type="button" disabled={disabled} onClick={() => onSelect(entry.path)}>{entry.name}</button></li>)}</ul>;
}

export function DirectoryPicker({ picker, disabled, onPick, onError }: { picker: ReturnType<typeof nativePicker>; disabled: boolean; onPick: (path: string) => void; onError: (error: string) => void }) {
  return picker ? <button data-testid={TEST_IDS.pickDirectory} type="button" disabled={disabled} onClick={async () => { try { const path = await picker(); if (path) onPick(path); } catch (error) { onError(String(error)); } }}>{ADMIN.picker}</button> : null;
}

export const handle = { crumb: '添加项目' };
