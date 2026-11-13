const ALLOWED_WRITE_OPS = Object.freeze([
  'project.pick',
  'project.confirm',
  'project.onboarding.advance',
  'project.onboarding.skip',
  'project.onboarding.run',
  'project.onboarding.save',
  'project.alert.dismiss',
  'task.create',
  'task.set_after',
  'task.move',
  'task.start',
  'task.reply',
  'task.approve',
  'task.reject',
  'task.merge',
  'task.pause',
  'task.resume',
  'task.stop',
  'task.cancel',
  'task.convert',
  'task.decide',
  'task.extend_budget',
  'task.rerun_from',
  'task.retro',
  'task.archive',
  'task.restore',
  'config.set_role',
  'config.reset_role',
  'config.set_loop',
  'config.set_theme',
  'env.detect',
  'env.install',
  'env.login',
  'open.path',
  'open.terminal',
  // 由网关本地处理，不转发给内核。
  'core.restart',
  'rules.proposals',
  'rules.decide',
  'rules.retire',
  'rules.restore',
]);

const COMPONENTS = Object.freeze(['git', 'claude', 'codex', 'iterm2']);

const ROLES = Object.freeze(['plan', 'review', 'adjudicate', 'impl', 'audit', 'retro']);

const PROSE_FIELDS: Readonly<Record<string, readonly string[]>> = Object.freeze({
  'task.create': ['request'],
  'task.reply': ['info'],
  'task.reject': ['feedback'],
  'task.decide': ['ruling'],
  // A whole project profile or AGENTS.md, edited in the wizard.
  'project.onboarding.save': ['content'],
});
const MAX_PROSE_LENGTH = 8000;
const MAX_DOCUMENT_LENGTH = 256 * 1024;
const MAX_BRIEF_BYTES = 65536;
const BYTE_LIMITED_FIELDS: Readonly<Record<string, Readonly<Record<string, number>>>> = Object.freeze({
  'task.create': { brief: MAX_BRIEF_BYTES },
});

function utf8ByteLength(value: string) {
  return new TextEncoder().encode(value).length;
}

const ARRAY_FIELDS: Readonly<Record<string, readonly string[]>> = Object.freeze({
  'task.create': ['attachments', 'doc_refs', 'after'],
  'task.set_after': ['after'],
  'config.set_role': ['skills'],
});
const MAX_ARRAY_ENTRIES = 256;
const MAX_AFTER_IDS = 8;
const MAX_ARRAY_ITEM_LENGTH = 1024;
const NAME_ONLY_ARRAYS = Object.freeze(['skills']);

const MAX_PAYLOAD_JSON_LENGTH = 32 * 1024;
const MAX_TASK_CREATE_PAYLOAD_LENGTH = 512 * 1024;
const PAYLOAD_LIMITS: Readonly<Record<string, number>> = Object.freeze({
  'task.create': MAX_TASK_CREATE_PAYLOAD_LENGTH,
  'project.onboarding.save': MAX_DOCUMENT_LENGTH + 4096,
});

function payloadLimitFor(op: string) {
  return PAYLOAD_LIMITS[op] || MAX_PAYLOAD_JSON_LENGTH;
}

function looksLikeAPath(value: string) {
  return (
    value.includes('/') ||
    value.includes('\\') ||
    value.startsWith('~') ||
    value.includes('..') ||
    value.includes('\u0000')
  );
}

function isAllowedOp(op: string) {
  return ALLOWED_WRITE_OPS.includes(op);
}

function validateShape(op: string, params: unknown): { ok: true } | { ok: false; message: string } {
  if (params === null || typeof params !== 'object' || Array.isArray(params)) {
    return { ok: false, message: 'params 必须是 JSON 对象' };
  }
  let serialized;
  try {
    serialized = JSON.stringify(params);
  } catch {
    return { ok: false, message: 'params 必须能序列化为 JSON' };
  }
  const limit = payloadLimitFor(op);
  if (serialized.length > limit) {
    return { ok: false, message: `params 超过 ${limit} 字符上限` };
  }

  const proseFields = PROSE_FIELDS[op] || [];
  for (const [key, value] of Object.entries(params)) {
    if (value === null || value === undefined) continue;
    if (Array.isArray(value)) {
      if (!(ARRAY_FIELDS[op] || []).includes(key)) {
        return { ok: false, message: `${op} 的 ${key} 不允许数组` };
      }
      if (value.length > MAX_ARRAY_ENTRIES) {
        return { ok: false, message: `${key} 最多允许 ${MAX_ARRAY_ENTRIES} 项` };
      }
      for (const item of value) {
        if (typeof item !== 'string' || item.length > MAX_ARRAY_ITEM_LENGTH) {
          return { ok: false, message: `${key} 的元素必须是短字符串` };
        }
        if (item.includes('\u0000')) {
          return { ok: false, message: `${key} 的元素不能含空字符` };
        }
        if (NAME_ONLY_ARRAYS.includes(key) && (!item || looksLikeAPath(item))) {
          return { ok: false, message: `${key} 的元素必须是名称，不能是路径` };
        }
      }
      continue;
    }
    if (typeof value === 'string') {
      const byteLimit = BYTE_LIMITED_FIELDS[op]?.[key];
      if (byteLimit !== undefined) {
        const bytes = utf8ByteLength(value);
        if (bytes > byteLimit) {
          return { ok: false, message: `任务卡正文超过 64KB（${bytes} 字节，上限 ${MAX_BRIEF_BYTES} 字节）` };
        }
      } else {
        const limit = proseFields.includes(key)
          ? op === 'project.onboarding.save'
            ? MAX_DOCUMENT_LENGTH
            : MAX_PROSE_LENGTH
          : 512;
        if (value.length > limit) {
          return { ok: false, message: `${key} 超过 ${limit} 字符上限` };
        }
      }
      if (value.includes('\u0000')) {
        return { ok: false, message: `${key} 不能含空字符` };
      }
      continue;
    }
    if (typeof value !== 'number' && typeof value !== 'boolean') {
      return { ok: false, message: `${key} 必须是字符串、数字或布尔值` };
    }
  }
  return { ok: true };
}

function validateOpSpecific(op: string, params: Record<string, unknown>): { ok: true } | { ok: false; message: string } {
  if (op === 'task.set_after' || (op === 'task.create' && params.after !== undefined)) {
    if (!Array.isArray(params.after) || params.after.length > MAX_AFTER_IDS || !params.after.every(
      (id: unknown) => typeof id === 'string' && /^T-\d+$/.test(id),
    )) {
      return { ok: false, message: `after 必须是任务编号列表，每项格式为 T-N，最多 ${MAX_AFTER_IDS} 项` };
    }
  }
  if (op === 'task.move') {
    for (const key of ['before', 'after']) {
      if (params[key] !== undefined && (typeof params[key] !== 'string' || !/^T-\d+$/.test(params[key]))) {
        return { ok: false, message: `${key} 必须是 T-N 格式的任务编号` };
      }
    }
    if (params.top !== undefined && typeof params.top !== 'boolean') {
      return { ok: false, message: 'top 必须是布尔值' };
    }
    if (Number(params.before !== undefined) + Number(params.after !== undefined) + Number(params.top === true) !== 1) {
      return { ok: false, message: 'before、after、top 必须恰好指定一个位置' };
    }
  }
  if (op === 'task.create') {
    if (params.kind !== undefined && params.kind !== null && params.kind !== 'feature' && params.kind !== 'bug') {
      return { ok: false, message: 'kind 必须是 feature 或 bug' };
    }
    if (params.draft !== undefined && typeof params.draft !== 'boolean') {
      return { ok: false, message: 'draft 必须是布尔值' };
    }
    if (params.brief !== undefined && typeof params.brief !== 'string') {
      return { ok: false, message: '任务卡正文必须是字符串' };
    }
    if (params.doc_refs !== undefined && (!Array.isArray(params.doc_refs) || params.doc_refs.length !== 0)) {
      return { ok: false, message: '网页端暂不支持指定参考文档' };
    }
    if (params.attachments !== undefined && (!Array.isArray(params.attachments) || !params.attachments.every(
      (id: unknown) => typeof id === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(id),
    ))) {
      return { ok: false, message: '附件必须是上传编号（小写 UUID）' };
    }
  }
  if (op === 'task.start' || op === 'task.reply' || op === 'task.set_after' || op === 'task.move') {
    if (typeof params.task_id !== 'string' || params.task_id.length === 0) {
      return { ok: false, message: 'task_id 必须是非空字符串' };
    }
  }
  if (op === 'task.reply' && typeof params.info !== 'string') {
    return { ok: false, message: 'info 必须是字符串' };
  }
  if (op === 'open.path') {
    // The renderer names *which* thing to open by id, never by path — Main
    // resolves the path from the core's own answer.
    if (typeof params.kind !== 'string') {
      return { ok: false, message: 'open.path 必须指定 kind' };
    }
    if ((typeof params.kind !== 'string' || !['project', 'worktree', 'document', 'skill', 'log'].includes(params.kind))) {
      return { ok: false, message: `未知的打开类型： ${params.kind}` };
    }
    for (const key of ['project_id', 'task_id', 'session_id', 'name']) {
      const v = params[key];
      if (typeof v === 'string' && looksLikeAPath(v)) {
        return { ok: false, message: `${key} 不能包含路径` };
      }
    }
  }
  if (op === 'config.set_theme' && (typeof params.theme !== 'string' || !['system', 'light', 'dark'].includes(params.theme))) {
    return { ok: false, message: 'theme 必须是 system、light、dark 之一' };
  }
  if (op === 'env.install' || op === 'env.login') {
    if ((typeof params.component !== 'string' || !COMPONENTS.includes(params.component))) {
      return { ok: false, message: 'component 必须是 git、claude、codex、iterm2 之一' };
    }
  }
  if (op === 'task.decide') {
    if ((typeof params.kind !== 'string' || !['backlog', 'dispute'].includes(params.kind))) {
      return { ok: false, message: 'kind 必须是 backlog 或 dispute' };
    }
    if ((typeof params.disposition !== 'string' || !['none', 'include', 'ignore', 'ruled'].includes(params.disposition))) {
      return { ok: false, message: '未知的 disposition' };
    }
  }
  if (op === 'task.convert') {
    if (typeof params.task_id !== 'string' || !params.task_id) {
      return { ok: false, message: 'task_id 必须是非空字符串' };
    }
    if (params.kind !== 'feature') {
      return { ok: false, message: 'kind 只能是 feature' };
    }
    if (params.start !== undefined && typeof params.start !== 'boolean') {
      return { ok: false, message: 'start 必须是布尔值' };
    }
  }
  if (op === 'project.onboarding.save') {
    // The renderer names one of two known files, never an arbitrary path;
    // the core enforces the same list, and both are deliberate.
    if ((typeof params.path !== 'string' || !['docs/agent-project-profile.md', 'AGENTS.md'].includes(params.path))) {
      return { ok: false, message: 'path 必须是项目档案或 AGENTS.md' };
    }
  }
  if (op === 'project.confirm') {
    // A member repository is a directory name directly under the workspace,
    // so a path-shaped one is either a mistake or an attempt to walk out.
    if (typeof params.docs_repo === 'string' && looksLikeAPath(params.docs_repo)) {
      return { ok: false, message: 'docs_repo 必须是名称，不能是路径' };
    }
  }
  if (op === 'config.set_role' || op === 'config.reset_role') {
    if ((typeof params.role !== 'string' || !ROLES.includes(params.role))) {
      return { ok: false, message: '未知的 role' };
    }
    if (params.runtime !== undefined && (typeof params.runtime !== 'string' || !['claude', 'codex'].includes(params.runtime))) {
      return { ok: false, message: 'runtime 必须是 claude 或 codex' };
    }
  }
  return { ok: true };
}

function validateWriteRequest(request: unknown): { ok: false; message: string; op?: never; params?: never } | { ok: true; message?: never; op: string; params: object } {
  if (request === null || typeof request !== 'object' || Array.isArray(request)) {
    return { ok: false, message: '请求必须是 JSON 对象' };
  }
  const { op, params } = request as Record<string, unknown>;
  if (typeof op !== 'string' || !isAllowedOp(op)) {
    return { ok: false, message: `op 必须是以下之一： ${ALLOWED_WRITE_OPS.join(', ')}` };
  }
  const effectiveParams = params === undefined ? {} : params;
  const shape = validateShape(op, effectiveParams);
  if (!shape.ok) return shape;
  const specific = validateOpSpecific(op, effectiveParams as Record<string, unknown>);
  if (!specific.ok) return specific;
  return { ok: true, op, params: effectiveParams as Record<string, unknown> };
}

export {
  ALLOWED_WRITE_OPS,
  ARRAY_FIELDS,
  ROLES,
  COMPONENTS,
  MAX_ARRAY_ENTRIES,
  MAX_PAYLOAD_JSON_LENGTH,
  MAX_PROSE_LENGTH,
  MAX_DOCUMENT_LENGTH,
  MAX_BRIEF_BYTES,
  MAX_TASK_CREATE_PAYLOAD_LENGTH,
  utf8ByteLength,
  payloadLimitFor,
  looksLikeAPath,
  validateWriteRequest,
};
