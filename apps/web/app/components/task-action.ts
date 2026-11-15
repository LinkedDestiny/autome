import { afterInput } from './queue-deps.ts';
import type { WebServices } from '../load-context.ts';
import type { TaskPayload } from './task-detail-data.ts';
import { availableOps, opsInput, OP_LABELS, type Intent } from './task-ops.ts';
import { MAX_PROSE_LENGTH } from '../../server/gates/write-gate.ts';
export const WRITE_OPS: Record<Intent, string> = { note: 'task.note', set_after: 'task.set_after', start: 'task.start', approve: 'task.approve', reject: 'task.reject', reply: 'task.reply', merge: 'task.merge', merge_cancel: 'task.merge', convert: 'task.convert', pause: 'task.pause', resume: 'task.resume', stop: 'task.stop', cancel: 'task.cancel', rerun: 'task.rerun_from', extend: 'task.extend_budget', decide: 'task.decide' };
export async function handleTaskAction(form: FormData, id: string, services: WebServices) {
  const intent = form.get('intent');
  if (typeof intent !== 'string' || !Object.hasOwn(WRITE_OPS, intent)) return { error: '未知操作' };
  const op = intent as Intent;
  const value = (name: string) => typeof form.get(name) === 'string' ? form.get(name) as string : '';
  const params: Record<string, unknown> = { task_id: id };
  if (op === 'cancel' && value('confirm') !== 'yes') return { error: '请先确认取消这个任务' };
  if (op === 'reject' || op === 'reply') {
    const key = op === 'reject' ? 'feedback' : 'info';
    if (!value(key).trim()) return { error: op === 'reject' ? '驳回意见不能为空' : '补充信息不能为空' };
    if (value(key).length > MAX_PROSE_LENGTH) return { error: `输入不能超过 ${MAX_PROSE_LENGTH} 字符` };
    params[key] = value(key);
  }
  if (op === 'note') {
    if (!value('text').trim()) return { error: '备注不能为空' };
    if (value('text').length > MAX_PROSE_LENGTH) return { error: `输入不能超过 ${MAX_PROSE_LENGTH} 字符` };
    params.text = value('text');
    params.reopen = value('reopen') === 'yes';
  }
  if (op === 'set_after') {
    const deps = afterInput(form);
    if (deps.error) return { error: deps.error };
    params.after = deps.after;
  }
  if (op === 'extend') {
    const extra = Number(value('extra_rounds'));
    if (!/^\d+$/.test(value('extra_rounds')) || !Number.isSafeInteger(extra) || extra < 1 || extra > 4294967295) return { error: '追加轮次必须是正整数且不超过 4294967295' };
    params.extra_rounds = extra;
  }
  if (op === 'convert') { params.kind = 'feature'; params.start = value('start') === 'yes'; }
  if (op === 'merge_cancel') params.cancel = true;
  if (op === 'decide') {
    const kind = value('kind'), disposition = value('disposition');
    if (!(kind === 'backlog' ? ['include', 'ignore', 'none'] : kind === 'dispute' ? ['ruled', 'none'] : []).includes(disposition) || !value('item_id')) return { error: '表态参数无效' };
    Object.assign(params, { kind, item_id: value('item_id'), disposition });
    if (disposition === 'ruled') {
      if (!value('ruling').trim() || value('ruling').length > MAX_PROSE_LENGTH) return { error: `裁定文字不能为空且不能超过 ${MAX_PROSE_LENGTH} 字符` };
      params.ruling = value('ruling');
    }
  }
  const gateway = services.gateway;
  if (!gateway) return { error: '内核未连接：服务未接入网关' };
  // Rerun options and merge cancellation depend on current kernel capabilities.
  if (op === 'rerun' || op === 'merge_cancel') {
    const result = await gateway.read({ method: 'task.get', params: { task_id: id } });
    if (!result.body.ok) return { error: result.body.error };
    const data = result.body.payload as TaskPayload;
    if (op === 'merge_cancel' && !Object.hasOwn(data.task, 'merge')) return { error: '当前内核不支持撤销合并授权' };
    if (op === 'rerun') {
      const config = data.project ? await gateway.read({ method: 'config.get', params: { project_id: data.project.id } }) : null;
      const entry = availableOps(opsInput(data, config?.body.ok ? config.body.payload : null)).find(o => o.op === 'rerun');
      if (!entry?.nodes?.includes(value('node'))) return { error: '只能选择当前允许的重跑节点' };
      params.node = value('node');
    }
  }
  const result = await gateway.write({ op: WRITE_OPS[op], params });
  if (result.body.ok && op === 'convert') {
    const converted = result.body.payload as { task: { id: string } };
    return { ok: true, notice: `已改建为需求 ${converted.task.id}`, convertedTaskId: converted.task.id };
  }
  if (result.body.ok && op === 'note') {
    const payload = result.body.payload as { warning?: string };
    return { ok: true, notice: '已提交备注', warning: payload.warning };
  }
  return result.body.ok ? { ok: true, notice: op === 'merge' ? '已提交合并' : `已提交${OP_LABELS[op]}` } : { error: result.body.error };
}
