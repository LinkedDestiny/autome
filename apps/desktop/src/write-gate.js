'use strict';

// Pure validation for the `autome:write` IPC channel. Deliberately
// independent from src/ipc-gate.js: a separate allowlist that is never merged
// with, or relaxed by, the read gate's. `isTrustedSenderUrl` is the one thing
// genuinely shared — identifying the packaged renderer origin is not a
// method-surface decision — so main.js imports it from ipc-gate.js for both
// channels rather than this module re-exporting a duplicate.
//
// Two rules shape the list below:
//
// 1. **The renderer never sends a filesystem path.** `project.pick` is the
//    only way a directory enters the system, and Main owns the dialog: the
//    renderer asks for a picker and gets back whatever Main chose. A renderer
//    that could name a path could name `~/.ssh`.
// 2. **Free text is allowed only where the product needs it**, and then it is
//    bounded. A task request and a rejection reason are prose the user typed;
//    everything else is an identifier from a list the renderer was given.

// Renderer-facing op names. `project.add` is deliberately absent — it takes a
// path, and only Main may supply one (see `project.pick` below).
const ALLOWED_WRITE_OPS = Object.freeze([
  'project.pick',
  'project.remove',
  'project.onboarding.advance',
  'project.onboarding.skip',
  'task.create',
  'task.approve',
  'task.reject',
  'task.merge',
  'task.pause',
  'task.resume',
  'task.stop',
  'task.cancel',
  'task.decide',
  'task.extend_budget',
  'task.rerun_from',
  'task.archive',
  'task.restore',
  'config.set_role',
  'config.reset_role',
  'config.set_loop',
  'env.detect',
  'env.install',
  'env.login',
  'scheduler.tick',
  'open.path',
  'open.terminal',
]);

// Ops whose params may contain prose the user typed, and the cap on it. A
// one-line request and a rejection reason are the only two.
const PROSE_FIELDS = Object.freeze({
  'task.create': ['request'],
  'task.reject': ['feedback'],
  'task.decide': ['ruling'],
});
const MAX_PROSE_LENGTH = 8000;

// Everything else is identifiers and small numbers.
const MAX_PAYLOAD_JSON_LENGTH = 32 * 1024;

// A value that eventually reaches the filesystem or a shell must never carry a
// path or a null byte, whatever the core would separately catch.
function looksLikeAPath(value) {
  return (
    value.includes('/') ||
    value.includes('\\') ||
    value.startsWith('~') ||
    value.includes('..') ||
    value.includes('\u0000')
  );
}

function isAllowedOp(op) {
  return ALLOWED_WRITE_OPS.includes(op);
}

function validateShape(op, params) {
  if (params === null || typeof params !== 'object' || Array.isArray(params)) {
    return { ok: false, message: 'params must be a JSON object' };
  }
  let serialized;
  try {
    serialized = JSON.stringify(params);
  } catch {
    return { ok: false, message: 'params must be JSON-serializable' };
  }
  if (serialized.length > MAX_PAYLOAD_JSON_LENGTH) {
    return { ok: false, message: `params exceed the ${MAX_PAYLOAD_JSON_LENGTH}-byte limit` };
  }

  const proseFields = PROSE_FIELDS[op] || [];
  for (const [key, value] of Object.entries(params)) {
    if (value === null || value === undefined) continue;
    if (Array.isArray(value)) {
      // Only `task.create` carries arrays, and only of short strings.
      if (op !== 'task.create') {
        return { ok: false, message: `${op} params must not contain arrays` };
      }
      for (const item of value) {
        if (typeof item !== 'string' || item.length > 1024) {
          return { ok: false, message: `${key} entries must be short strings` };
        }
      }
      continue;
    }
    if (typeof value === 'string') {
      const limit = proseFields.includes(key) ? MAX_PROSE_LENGTH : 512;
      if (value.length > limit) {
        return { ok: false, message: `${key} exceeds ${limit} characters` };
      }
      if (value.includes('\u0000')) {
        return { ok: false, message: `${key} must not contain a null byte` };
      }
      continue;
    }
    if (typeof value !== 'number' && typeof value !== 'boolean') {
      return { ok: false, message: `${key} must be a string, number or boolean` };
    }
  }
  return { ok: true };
}

// Per-op checks for the values that leave the sandbox: an opened path, a
// terminal command, an install target.
function validateOpSpecific(op, params) {
  if (op === 'open.path') {
    // The renderer names *which* thing to open by id, never by path — Main
    // resolves the path from the core's own answer.
    if (typeof params.kind !== 'string') {
      return { ok: false, message: 'open.path needs a kind' };
    }
    if (!['project', 'worktree', 'document', 'skill', 'log'].includes(params.kind)) {
      return { ok: false, message: `unknown open kind: ${params.kind}` };
    }
    for (const key of ['project_id', 'task_id', 'session_id', 'name']) {
      const v = params[key];
      if (typeof v === 'string' && looksLikeAPath(v)) {
        return { ok: false, message: `${key} must not contain a path` };
      }
    }
  }
  if (op === 'env.install' || op === 'env.login') {
    if (!['git', 'claude', 'codex', 'iterm2'].includes(params.component)) {
      return { ok: false, message: 'component must be one of git, claude, codex, iterm2' };
    }
  }
  if (op === 'task.decide') {
    if (!['backlog', 'dispute'].includes(params.kind)) {
      return { ok: false, message: 'kind must be backlog or dispute' };
    }
    if (!['none', 'include', 'ignore', 'ruled'].includes(params.disposition)) {
      return { ok: false, message: 'unknown disposition' };
    }
  }
  if (op === 'config.set_role' || op === 'config.reset_role') {
    if (!['plan', 'review', 'adjudicate', 'impl', 'audit'].includes(params.role)) {
      return { ok: false, message: 'unknown role' };
    }
    if (params.runtime !== undefined && !['claude', 'codex'].includes(params.runtime)) {
      return { ok: false, message: 'runtime must be claude or codex' };
    }
  }
  return { ok: true };
}

// Returns `{ ok: true, op, params }` or `{ ok: false, message }`.
function validateWriteRequest(request) {
  if (request === null || typeof request !== 'object' || Array.isArray(request)) {
    return { ok: false, message: 'request must be a JSON object' };
  }
  const { op, params } = request;
  if (typeof op !== 'string' || !isAllowedOp(op)) {
    return { ok: false, message: `op must be one of: ${ALLOWED_WRITE_OPS.join(', ')}` };
  }
  const effectiveParams = params === undefined ? {} : params;
  const shape = validateShape(op, effectiveParams);
  if (!shape.ok) return shape;
  const specific = validateOpSpecific(op, effectiveParams);
  if (!specific.ok) return specific;
  return { ok: true, op, params: effectiveParams };
}

module.exports = {
  ALLOWED_WRITE_OPS,
  MAX_PAYLOAD_JSON_LENGTH,
  MAX_PROSE_LENGTH,
  looksLikeAPath,
  isAllowedOp,
  validateWriteRequest,
};
