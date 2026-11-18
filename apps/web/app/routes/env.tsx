import { RelativeTime } from '../lib/clock.tsx';
import { Ribbon } from '../components/ui/ribbon.tsx';
import { Pill } from '../components/ui/pill.tsx';
import { driverSummary } from '../../server/drivers.ts';
import { TEST_IDS } from '../lib/testids.ts';
import { useState } from 'react';
import { Form, useLoaderData, useActionData, useNavigation, type LoaderFunctionArgs, type ActionFunctionArgs } from 'react-router';
import { servicesContext } from '../load-context.ts';
import { ADMIN, ENV, COMPONENT_NAMES } from '../lib/admin-labels.ts';
import { useWriteGate } from '../lib/live.tsx';
import { byKind, liveShouldRevalidate } from '../lib/live-routes.ts';
import { toolOk, loginLabel, type EnvData, type EnvResult, type EnvSnapshot, type InstallRecipe } from '../components/env-model.ts';
export const shouldRevalidate = liveShouldRevalidate(byKind('env.'));
export async function loader({ context }: LoaderFunctionArgs): Promise<EnvData> {
  const data: EnvData = { recipes: {}, errors: {} };
  const { gateway, drivers, instanceStatus } = context.get(servicesContext);
  if (instanceStatus) {
    try { data.kernel = (await instanceStatus() as { kernel: EnvData['kernel'] }).kernel; }
    catch { data.kernelError = '内核信息暂不可用'; }
  }
  if (drivers) data.drivers = await drivers();
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
  return <><pre>{command}</pre><button className="btn" data-testid={TEST_IDS.envCopy} data-command={command} type="button" onClick={async () => {
    try { await navigator.clipboard.writeText(command); setStatus(ENV.copied); } catch { setStatus(ENV.copyError); }
  }}>{ENV.copy}</button><span role="status">{status}</span></>;
}
export function EnvView({ data, result, busy = false }: { data: EnvData; result?: EnvResult; busy?: boolean }) {
  const gate = useWriteGate(), snapshot = data.snapshot;
  const disabled = !gate.allowed || busy;
  return <div className="env-page" data-testid={TEST_IDS.pageEnv}><header><Ribbon><h1>{ENV.title}</h1></Ribbon>
    <div>{snapshot?.probed && <p>{snapshot.problems.length ? snapshot.problems.length + ' 项有问题' : (snapshot.environment?.components.length ?? 0) + ' 项检查全部正常'}</p>}
    {snapshot?.environment?.checked_at && <RelativeTime at={snapshot.environment.checked_at} suffix="检测过" />}</div>
    <Form method="post"><button className="btn" name="intent" value="detect" disabled={disabled || snapshot?.probing} title={gate.reason}>{ENV.detect}</button></Form></header>
    <div className="env-components" data-scroll-region>
      {data.error && <p role="alert">{data.error}</p>}
      {(snapshot && (!snapshot.probed || snapshot.probing)) && <p role="status">{ENV.probing}</p>}
      {result?.error && <p role="alert">{result.error}</p>}
      {result?.launched === true && <p role="status">{ENV.launched}</p>}
      {result?.launched === false && <section role="status"><p>{ENV.terminalError}{result.reason}。{ENV.fallback}</p><CopyCommand command={result.command ?? ''} /></section>}
      <section className="card env-tools"><h2>工具检查</h2>
      <section className="env-component" data-env-ok={Boolean(data.kernel)}><div className="env-tool-row"><span className={data.kernel ? "env-dot" : "env-dot env-dot-warning"} /><h3>Autome 内核</h3><div>{data.kernel?.path ?? data.kernelError ?? "内核信息暂不可用"}</div><Pill color="brown">{data.kernel?.build_id ?? ENV.none}</Pill></div></section>
      {snapshot?.probed && snapshot.environment?.components.map(component => {
        const recipe = data.recipes[component.component];
        const ok = toolOk(component, snapshot.problems);
        return <section className="env-component" data-env-ok={ok} key={component.component}><div className="env-tool-row"><span className={ok ? "env-dot" : "env-dot env-dot-warning"} /><h3>{Object.hasOwn(COMPONENT_NAMES, component.component) ? COMPONENT_NAMES[component.component] : component.component}</h3><div><p className="env-path" title={component.path ?? undefined}>{component.path ?? ENV.none}</p>
          <p>{component.present ? ENV.installed : ENV.missing} · {loginLabel(component.login)}{snapshot.problems.some(p => p.component === component.component) && ` · ${ENV.attention}`}</p>
          </div><Pill color="blue">{component.version ?? ENV.none}</Pill></div><details className="env-install" open={!ok}><summary>查看安装方法</summary><div className="env-tool-actions">
          {data.errors[component.component] && <p role="alert">{data.errors[component.component]}</p>}
          {recipe && <><p>{recipe.recipe.note}</p><p>{ENV.prerequisite}：{recipe.recipe.prerequisite ?? ENV.none} · {recipe.prerequisite_ok ? ENV.met : ENV.unmet}</p><CopyCommand command={recipe.recipe.command} />
            <Form method="post"><input type="hidden" name="component" value={component.component} /><button className="btn" name="intent" value="install" disabled={disabled || !recipe.prerequisite_ok} title={gate.reason}>{ENV.install}</button></Form>
            {recipe.login_command && <><CopyCommand command={recipe.login_command} /><Form method="post"><input type="hidden" name="component" value={component.component} /><button className="btn" name="intent" value="login" disabled={disabled} title={gate.reason}>{ENV.login}</button></Form></>}
          </>}
        </div></details></section>;
      })}</section>
      <section className="card card-pat card-green env-drivers"><Ribbon color="green" small><h2>{ENV.drivers}</h2></Ribbon>
        <p role="status">{driverSummary(data.drivers)}</p>
        {data.drivers?.list.map(driver => <div className="env-driver-row" key={driver.pid}><div><strong>{driver.command}</strong>{driver.self && <Pill color="green">{ENV.selfDriver}</Pill>}<p>pid {driver.pid} · <RelativeTime at={driver.started_at} suffix="启动" /></p></div><Pill color={driver.driving ? 'green' : 'brown'}>{driver.driving ? ENV.driving : ENV.stoppedDriving}</Pill><Pill><RelativeTime at={driver.last_tick_at} suffix="检查过" /></Pill></div>)}
      </section>
    </div></div>;
}
export default function Environment() { return <EnvView data={useLoaderData<typeof loader>()} result={useActionData<typeof action>()} busy={useNavigation().state !== 'idle'} />; }

export const handle = { crumb: '环境' };
