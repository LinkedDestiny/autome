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
  'project.onboarding.advance',
  'project.onboarding.skip',
  'project.onboarding.run',
  'project.onboarding.save',
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
  // Handled by Main, never forwarded: it is what the offline banner offers
  // after Main has stopped restarting a core that will not start.
  'core.restart',
  'protocol.rollback',
  'protocol.improve',
  'rules.proposals',
  'rules.decide',
  'rules.retire',
  'rules.restore',
]);

// Ops whose params may contain prose the user typed, and the cap on it. A
// one-line request and a rejection reason are the only two.
const PROSE_FIELDS = Object.freeze({
  'task.create': ['request'],
  'task.reject': ['feedback'],
  'task.decide': ['ruling'],
  // A whole project profile or AGENTS.md, edited in the wizard.
  'project.onboarding.save': ['content'],
});
const MAX_PROSE_LENGTH = 8000;
const MAX_DOCUMENT_LENGTH = 256 * 1024;

// Ops whose params may carry a list, and which keys may be one. A table
// rather than a condition, because the previous version of this was a
// condition — `op !== 'task.create'` — and it silently killed two features:
// the routing screen saves a role by sending its whole skill list (S-03: a
// binding *is* a role's skill list), and it sends that list on every save,
// even an empty one, so *every* role save was refused. Nothing caught it
// because the gate's tests agreed with the gate rather than with its callers.
const ARRAY_FIELDS = Object.freeze({
  'task.create': ['attachments', 'doc_refs'],
  'config.set_role': ['skills'],
});
// Attachments on a request, or skills bound to a role. Well above anything a
// person would pick by hand, and the payload cap bounds the total anyway.
const MAX_ARRAY_ENTRIES = 256;
const MAX_ARRAY_ITEM_LENGTH = 1024;
// Keys whose entries name something on disk by *name*, never by path. A skill
// is a directory under one of the runtimes' skill roots, so its name is a
// single segment; `../` in one has no legitimate reading.
const NAME_ONLY_ARRAYS = Object.freeze(['skills']);

// The payload cap, per op. One op legitimately carries a whole document —
// the Onboarding wizard's in-app editor — and giving every op that headroom
// would mean a buggy renderer could wedge half a megabyte of attachment paths
// into the sidecar pipe.
const MAX_PAYLOAD_JSON_LENGTH = 32 * 1024;
const PAYLOAD_LIMITS = Object.freeze({
  'project.onboarding.save': MAX_DOCUMENT_LENGTH + 4096,
});

function payloadLimitFor(op) {
  return PAYLOAD_LIMITS[op] || MAX_PAYLOAD_JSON_LENGTH;
}

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
  const limit = payloadLimitFor(op);
  if (serialized.length > limit) {
    return { ok: false, message: `params exceed the ${limit}-byte limit` };
  }

  const proseFields = PROSE_FIELDS[op] || [];
  for (const [key, value] of Object.entries(params)) {
    if (value === null || value === undefined) continue;
    if (Array.isArray(value)) {
      if (!(ARRAY_FIELDS[op] || []).includes(key)) {
        return { ok: false, message: `${op} params must not contain an array at ${key}` };
      }
      if (value.length > MAX_ARRAY_ENTRIES) {
        return { ok: false, message: `${key} must hold at most ${MAX_ARRAY_ENTRIES} entries` };
      }
      for (const item of value) {
        if (typeof item !== 'string' || item.length > MAX_ARRAY_ITEM_LENGTH) {
          return { ok: false, message: `${key} entries must be short strings` };
        }
        if (item.includes('\u0000')) {
          return { ok: false, message: `${key} entries must not contain a null byte` };
        }
        if (NAME_ONLY_ARRAYS.includes(key) && (!item || looksLikeAPath(item))) {
          return { ok: false, message: `${key} entries must be names, not paths` };
        }
      }
      continue;
    }
    if (typeof value === 'string') {
      const limit = proseFields.includes(key)
        ? op === 'project.onboarding.save'
          ? MAX_DOCUMENT_LENGTH
          : MAX_PROSE_LENGTH
        : 512;
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
  if (op === 'config.set_theme' && !['system', 'light', 'dark'].includes(params.theme)) {
    return { ok: false, message: 'theme must be one of system, light, dark' };
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
  if (op === 'project.onboarding.save') {
    // The renderer names one of two known files, never an arbitrary path;
    // the core enforces the same list, and both are deliberate.
    if (!['docs/agent-project-profile.md', 'AGENTS.md'].includes(params.path)) {
      return { ok: false, message: 'path must be the profile or AGENTS.md' };
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
  ARRAY_FIELDS,
  MAX_ARRAY_ENTRIES,
  MAX_PAYLOAD_JSON_LENGTH,
  MAX_PROSE_LENGTH,
  MAX_DOCUMENT_LENGTH,
  payloadLimitFor,
  looksLikeAPath,
  validateWriteRequest,
};
