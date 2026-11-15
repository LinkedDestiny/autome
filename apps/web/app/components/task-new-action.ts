import { afterInput } from './queue-deps.ts';
import { redirect } from 'react-router';
import type { WebServices } from '../load-context.ts';
import { MAX_BRIEF_BYTES } from '../../server/gates/write-gate.ts';
import { UPLOAD_MAX_FILES } from '../../server/uploads-limits.ts';
import { projectHref } from './nav-links.ts';

export function validateNewTask(brief: string, attachments: string[], uploads: WebServices['uploads']) {
  if (brief.split(/\r?\n/)[0].trim() === '# <标题>') return '请把第一行的 <标题> 换成任务标题';
  if (new TextEncoder().encode(brief).length > MAX_BRIEF_BYTES) return '任务卡正文超过 64KB';
  if (attachments.length > UPLOAD_MAX_FILES) return `附件最多 ${UPLOAD_MAX_FILES} 个`;
  const names = new Set<string>();
  for (const id of attachments) {
    const info = uploads?.describe(id);
    if (!info) return `附件 ${id} 不存在或已过期，请重新上传`;
    if (names.has(info.name)) return `附件重名：${info.name}，内核会按文件名复制，后一个会覆盖前一个，请改名后重新上传`;
    names.add(info.name);
  }
  return null;
}
export async function createNewTask(form: FormData, projectId: string, services: WebServices) {
  const kind = form.get('kind'), intent = form.get('intent'), brief = form.get('brief');
  if (kind !== 'feature' && kind !== 'bug') return { error: '未知任务类型' };
  if (intent !== 'start' && intent !== 'draft') return { error: '未知操作' };
  const attachments = form.getAll('attachments');
  if (typeof brief !== 'string' || attachments.some(id => typeof id !== 'string')) return { error: '任务卡表单无效' };
  const deps = afterInput(form);
  if (deps.error) return { error: deps.error };
  const ids = attachments as string[];
  const error = validateNewTask(brief, ids, services.uploads);
  if (error) return { error };
  if (!services.gateway) return { error: '内核未连接：服务未接入网关' };
  const result = await services.gateway.write({ op: 'task.create', params: { project_id: projectId, kind, brief, draft: intent === 'draft', attachments: ids, after: deps.after } });
  if (!result.body.ok) return { error: result.body.error };
  return redirect(projectHref(projectId));
}
