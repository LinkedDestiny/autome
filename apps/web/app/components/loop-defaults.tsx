import { useState } from 'react';
import { useFetcher } from 'react-router';
import { Stepper } from './ui/stepper.tsx';
import { Card } from './ui/card.tsx';
import { Ribbon } from './ui/ribbon.tsx';
import { defaultFields, validDefault, type DefaultKey } from './loop-defaults-data.ts';
import type { ConfigReply } from './loop-data.ts';
function DefaultField({ field, config, project, disabled }: { field: typeof defaultFields[number]; config: ConfigReply; project?: string; disabled: boolean }) {
  const base = config.resolved.loop_defaults[field.key];
  const [value, setValue] = useState(base);
  const fetcher = useFetcher<{ saved?: boolean; error?: string }>();
  const busy = disabled || fetcher.state !== 'idle';
  const submit = (key: DefaultKey, value: number | null) => { void fetcher.submit({ intent: 'default', key, value: String(value) }, { method: 'post' }); };
  return <Card color="blue" className="loop-default-field">
    <div className="loop-default-head"><span>{field.label}</span>{project && <span className="pill">{base === config.global.loop_defaults[field.key] ? '与全局相同' : '项目值'}</span>}</div>
    <div className="loop-default-controls"><Stepper labelHidden label={field.label} min={field.min} max={field.max} name={field.key} value={value} onChange={setValue} disabled={busy} />
    <button aria-label={`保存${field.label}`} disabled={busy || value === base || !validDefault(field.key, value)} className="btn btn-sm" onClick={() => submit(field.key, value)}>保存</button>
    {fetcher.data?.saved && <span role="status">已保存</span>}</div>
    {project && <button className="loop-default-reset btn btn-sm" aria-label={`恢复跟随全局 · ${field.label}`} disabled={busy} onClick={() => submit(field.key, null)}>恢复跟随全局</button>}
    {fetcher.data?.error && <p role="alert">{fetcher.data.error}</p>}
  </Card>;
}
export function LoopDefaultsView({ config, project, disabled }: { config: ConfigReply; project?: string; disabled: boolean }) {
  return <section aria-label="Loop 默认值"><Ribbon color="blue" small>Loop 默认值</Ribbon><div className="loop-defaults">{defaultFields.map(field => <DefaultField key={field.key + ':' + config.resolved.loop_defaults[field.key]} field={field} config={config} project={project} disabled={disabled} />)}</div></section>;
}
