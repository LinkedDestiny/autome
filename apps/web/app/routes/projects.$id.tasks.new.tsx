import { DependencyChoices } from '../components/task-dependencies.tsx';
import { dependencyCandidates } from '../components/queue-deps.ts';
import { TEST_IDS } from '../lib/testids.ts';
import { useRef, useState } from 'react';
import { Form, Link, useActionData, useNavigation, useParams, useRouteLoaderData, type ActionFunctionArgs } from 'react-router';
import { TASK_TEMPLATES, switchKind, type TaskKind } from '../components/task-templates.ts';
import { createNewTask } from '../components/task-new-action.ts';
import { uploadFile } from '../components/upload-client.ts';
import { projectHref } from '../components/nav-links.ts';
import type { ShellData } from '../components/shell-data.ts';
import type { UploadInfo } from '../../server/uploads.ts';
import { UPLOAD_MAX_BYTES, UPLOAD_MAX_FILES } from '../../server/uploads-limits.ts';
import { servicesContext } from '../load-context.ts';
import { KIND_LABELS, COLUMN_LABELS } from '../lib/labels.ts';
import { useWriteGate } from '../lib/live.tsx';

export function action({ request, params, context }: ActionFunctionArgs) {
  return request.formData().then(form => createNewTask(form, params.id ?? '', context.get(servicesContext)));
}
type Attachment = { key: number; name: string; size: number; info?: UploadInfo; error?: string };
export default function NewTask() {
  const { id = '' } = useParams();
  const shell = useRouteLoaderData<ShellData>('root');
  const project = shell?.projects.find(project => project.id === id);
  const [kind, setKind] = useState<TaskKind>('feature');
  const [body, setBody] = useState(TASK_TEMPLATES.feature);
  const [kept, setKept] = useState(false);
  const [attachments, setAttachments] = useState<Attachment[]>([]);
  const [uploading, setUploading] = useState(false);
  const uploadingRef = useRef(false);
  const nextKey = useRef(0);
  const gate = useWriteGate();
  const navigation = useNavigation();
  const result = useActionData<typeof action>();
  const busy = navigation.state !== 'idle' || uploading;
  const disabled = !gate.allowed || busy || attachments.some(item => item.error);
  async function addFiles(files: File[]) {
    if (uploadingRef.current) return;
    uploadingRef.current = true; setUploading(true);
    try {
      for (const file of files) {
        const key = nextKey.current++;
        setAttachments(items => [...items, { key, name: file.name, size: file.size }]);
        const result = await uploadFile(fetch, file);
        setAttachments(items => items.map(item => item.key !== key ? item : result.ok
          ? { ...item, name: result.payload.name, info: result.payload }
          : { ...item, error: result.error }));
      }
    } finally { uploadingRef.current = false; setUploading(false); }
  }
  if (!project) return <p data-testid={TEST_IDS.pageTaskNew} role="alert">项目不存在</p>;
  return <div data-testid={TEST_IDS.pageTaskNew} className="task-new-page"><Form method="post" className="task-new" data-testid={TEST_IDS.taskNewForm}>
    <header><h1>新建任务卡</h1><Link to={projectHref(id)}>{project.name}</Link></header>
    <fieldset disabled={busy}><legend>任务类型</legend>{(['feature', 'bug'] as const).map(value => <label key={value}>
      <input type="radio" name="kind" value={value} checked={kind === value} onChange={() => {
        const changed = switchKind(body, kind, value); setKind(value); setBody(changed.body); setKept(changed.kept);
      }} />{KIND_LABELS[value]}</label>)}</fieldset>
    {kept && <p role="status">正文已修改，未替换模板 <button type="button" disabled={busy} onClick={() => { setBody(TASK_TEMPLATES[kind]); setKept(false); }}>换成{KIND_LABELS[kind]}模板</button></p>}
    <label htmlFor="task-brief">任务正文（Markdown）</label>
    <textarea id="task-brief" name="brief" value={body} readOnly={busy} onChange={event => setBody(event.target.value)} />
    <section className="task-new-attachments" aria-label="附件">
      <label>附件（单个最多 {UPLOAD_MAX_BYTES / 1024 / 1024} MB，最多 {UPLOAD_MAX_FILES} 个）<input type="file" multiple disabled={!gate.allowed || busy} onChange={event => {
        const files = Array.from(event.target.files ?? []); event.target.value = ''; void addFiles(files);
      }} /></label>
      <ul>{attachments.map(item => <li key={item.key}>{item.name} · {item.size} 字节
        {item.error ? <span role="alert">{item.error}</span> : !item.info ? <span>上传中</span> : <input type="hidden" name="attachments" value={item.info.id} />}
        <button type="button" disabled={navigation.state !== 'idle'} onClick={() => setAttachments(items => items.filter(other => other.key !== item.key))}>移除</button>
      </li>)}</ul>
    </section>
    <DependencyChoices tasks={dependencyCandidates(shell?.tasks ?? [], id)} disabled={!gate.allowed || busy} />
    {result && 'error' in result && <p role="alert">{result.error}</p>}
    {!gate.allowed && <p role="status">{gate.reason}</p>}
    <footer><button className="btn-primary" name="intent" value="start" disabled={Boolean(disabled)}>创建并开始</button>
      <button data-testid={TEST_IDS.taskNewDraft} name="intent" value="draft" disabled={Boolean(disabled)}>存为{COLUMN_LABELS.todo}</button></footer>
  </Form></div>;
}

export const handle = { crumb: '新建任务' };
