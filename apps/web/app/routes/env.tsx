import { useState } from 'react';
import { Form, useLoaderData, useActionData, useNavigation, type LoaderFunctionArgs, type ActionFunctionArgs } from 'react-router';
import { servicesContext } from '../load-context.ts';
import { ADMIN, ENV, COMPONENT_NAMES } from '../lib/admin-labels.ts';
import { useWriteGate } from '../lib/live.tsx';
import { byKind, liveShouldRevalidate } from '../lib/live-routes.ts';
import { loginLabel, type EnvData, type EnvResult, type EnvSnapshot, type InstallRecipe } from '../components/env-model.ts';
export const shouldRevalidate = liveShouldRevalidate(byKind('env.'));
export async function loader({ context }: LoaderFunctionArgs): Promise<EnvData> {
  const data: EnvData = { recipes: {}, errors: {} };
  const { gateway } = context.get(servicesContext);
  if (!gateway) return { ...data, error: ADMIN.offline };
  const result = await gateway.read({ method: 'env.get', params: {} });
  if (!result.body.ok) return { ...data, error: result.body.error };
  data.snapshot = result.body.payload as EnvSnapshot;
  if (data.snapshot.probed) await Promise.all((data.snapshot.environment?.components ?? []).map(async ({ component }) => {
    const recipe = await gateway.read({ method: 'env.install_recipe', params: { component } });
    if (recipe.body.ok) data.recipes[component] = recipe.body.payload as InstallRecipe;
    else data.errors[component] = recipe.body.error ?? ADMIN.offline;
  }));
  return data;
}
export async function action({ request, context }: ActionFunctionArgs): Promise<EnvResult> {
  const form = await request.formData(), intent = form.get('intent');
  const { gateway } = context.get(servicesContext);
  if (!gateway) return { error: ADMIN.offline };
  if (!['detect', 'install', 'login'].includes(String(intent))) return { error: ADMIN.unknown };
  const result = await gateway.write({ op: `env.${intent}`, params: intent === 'detect' ? { force: true } : { component: form.get('component') } });
  return result.body.ok ? result.body.payload as EnvResult : { error: result.body.error };
}
function CopyCommand({ command }: { command: string }) {
  const [status, setStatus] = useState('');
  return <><pre>{command}</pre><button type="button" onClick={async () => {
    try { await navigator.clipboard.writeText(command); setStatus(ENV.copied); } catch { setStatus(ENV.copyError); }
  }}>{ENV.copy}</button><span role="status">{status}</span></>;
}
export function EnvView({ data, result, busy = false }: { data: EnvData; result?: EnvResult; busy?: boolean }) {
  const gate = useWriteGate(), snapshot = data.snapshot;
  const disabled = !gate.allowed || busy;
  return <div className="env-page"><header><h1>{ENV.title}</h1><Form method="post"><button name="intent" value="detect" disabled={disabled || snapshot?.probing} title={gate.reason}>{ENV.detect}</button></Form></header>
    <div className="env-components">
      {data.error && <p role="alert">{data.error}</p>}
      {(snapshot && (!snapshot.probed || snapshot.probing)) && <p role="status">{ENV.probing}</p>}
      {result?.error && <p role="alert">{result.error}</p>}
      {result?.launched === true && <p role="status">{ENV.launched}</p>}
      {result?.launched === false && <section role="status"><p>{ENV.terminalError}{result.reason}。{ENV.fallback}</p><CopyCommand command={result.command ?? ''} /></section>}
      {snapshot?.probed && snapshot.environment?.components.map(component => {
        const recipe = data.recipes[component.component];
        return <section className="env-component" key={component.component}><h2>{Object.hasOwn(COMPONENT_NAMES, component.component) ? COMPONENT_NAMES[component.component] : component.component}</h2>
          <p>{component.present ? ENV.installed : ENV.missing} · {loginLabel(component.login)}{snapshot.problems.some(p => p.component === component.component) && ` · ${ENV.attention}`}</p>
          <dl><dt>{ENV.version}</dt><dd>{component.version ?? ENV.none}</dd><dt>{ENV.path}</dt><dd>{component.path ?? ENV.none}</dd></dl>
          {data.errors[component.component] && <p role="alert">{data.errors[component.component]}</p>}
          {recipe && <><p>{recipe.recipe.note}</p><p>{ENV.prerequisite}：{recipe.recipe.prerequisite ?? ENV.none} · {recipe.prerequisite_ok ? ENV.met : ENV.unmet}</p><CopyCommand command={recipe.recipe.command} />
            <Form method="post"><input type="hidden" name="component" value={component.component} /><button name="intent" value="install" disabled={disabled || !recipe.prerequisite_ok} title={gate.reason}>{ENV.install}</button></Form>
            {recipe.login_command && <><CopyCommand command={recipe.login_command} /><Form method="post"><input type="hidden" name="component" value={component.component} /><button name="intent" value="login" disabled={disabled} title={gate.reason}>{ENV.login}</button></Form></>}
          </>}
        </section>;
      })}
    </div></div>;
}
export default function Environment() { return <EnvView data={useLoaderData<typeof loader>()} result={useActionData<typeof action>()} busy={useNavigation().state !== 'idle'} />; }
