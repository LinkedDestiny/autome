'use strict';

// Pure validation for the `autome:write` IPC channel — plan §8.2's
// two-phase target-registration write path. Deliberately independent from
// src/ipc-gate.js: a separate allowlist, never merged with or relaxed by
// the read gate's own list. `isTrustedSenderUrl` is the one thing genuinely
// shared with ipc-gate.js (identifying the packaged renderer origin isn't a
// method-surface decision, so main.js imports it straight from ipc-gate.js
// for both channels rather than this module re-exporting a duplicate).

// Renderer-facing op names over the `autome:write` channel. Neither
// `project.register_target` (Core's method, Main-only — it takes a raw
// filesystem path) nor `project.create` (the old raw-locator primitive)
// is ever reachable from here; see write-gate.test.js's negative tests.
const ALLOWED_WRITE_OPS = Object.freeze(['project.pick_target', 'project.create_from_target']);

// snake_case at this layer; main.js translates to Core's PascalCase
// `ProjectKind` variant strings ("NewProduct" / "ExistingRepository") only
// when it forwards to `project.register_target`.
const ALLOWED_PROJECT_KINDS = Object.freeze(['new_product', 'existing_repository']);

// Generous but finite, matching ipc-gate.js's read-side limit — a write
// request here is a handful of short strings and a boolean, never a
// document.
const MAX_PAYLOAD_JSON_LENGTH = 4096;

// §8.2/§8.3: `display_name` and `destination_name` are scalars that
// eventually get written to disk (a project's display name; a new
// directory's name) — they must never carry a path, URL, or shell shape,
// regardless of what Core's own validation would separately catch.
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

function validateScalarParams(params) {
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
  for (const value of Object.values(params)) {
    // `destination_name` may be explicitly absent — null is allowed at
    // this generic layer; the per-op checks below reject null where it
    // isn't meaningful (e.g. `kind`, `target_id`).
    if (value === null) continue;
    if (typeof value !== 'string' && typeof value !== 'number' && typeof value !== 'boolean') {
      return { ok: false, message: 'params values must be strings, numbers, booleans, or null' };
    }
  }
  return { ok: true };
}

// Validates the shape of a renderer-submitted write request. Returns
// `{ ok: true, op, params }` or `{ ok: false, message }` — never throws,
// mirroring ipc-gate.js's validateReadRequest so main.js's handler can
// reuse the same error-mapping shape for both channels.
function validateWriteRequest(request) {
  if (request === null || typeof request !== 'object' || Array.isArray(request)) {
    return { ok: false, message: 'request must be a JSON object' };
  }
  const { op, params } = request;
  if (typeof op !== 'string' || !isAllowedOp(op)) {
    return { ok: false, message: `op must be one of: ${ALLOWED_WRITE_OPS.join(', ')}` };
  }
  const effectiveParams = params === undefined ? {} : params;
  const scalarCheck = validateScalarParams(effectiveParams);
  if (!scalarCheck.ok) {
    return scalarCheck;
  }

  if (op === 'project.pick_target') {
    const { kind, ...rest } = effectiveParams;
    if (Object.keys(rest).length > 0) {
      return { ok: false, message: 'project.pick_target accepts only { kind }' };
    }
    if (!ALLOWED_PROJECT_KINDS.includes(kind)) {
      return { ok: false, message: `kind must be one of: ${ALLOWED_PROJECT_KINDS.join(', ')}` };
    }
  }

  if (op === 'project.create_from_target') {
    const {
      target_id: targetId,
      display_name: displayName,
      trust_confirmed: trustConfirmed,
      destination_name: destinationName,
      ...rest
    } = effectiveParams;
    if (Object.keys(rest).length > 0) {
      return {
        ok: false,
        message: 'project.create_from_target accepts only target_id, display_name, trust_confirmed, destination_name',
      };
    }
    if (typeof targetId !== 'string' || targetId.length === 0) {
      return { ok: false, message: 'target_id must be a non-empty string' };
    }
    if (typeof displayName !== 'string' || displayName.trim().length === 0) {
      return { ok: false, message: 'display_name must be a non-empty string' };
    }
    if (looksLikeAPath(displayName)) {
      return { ok: false, message: 'display_name must not look like a filesystem path' };
    }
    if (typeof trustConfirmed !== 'boolean') {
      return { ok: false, message: 'trust_confirmed must be a boolean' };
    }
    if (destinationName !== undefined && destinationName !== null) {
      if (typeof destinationName !== 'string' || destinationName.length === 0) {
        return { ok: false, message: 'destination_name must be a non-empty string when present' };
      }
      if (looksLikeAPath(destinationName)) {
        return { ok: false, message: 'destination_name must not look like a filesystem path' };
      }
    }
  }

  return { ok: true, op, params: effectiveParams };
}

module.exports = {
  ALLOWED_WRITE_OPS,
  ALLOWED_PROJECT_KINDS,
  MAX_PAYLOAD_JSON_LENGTH,
  isAllowedOp,
  validateWriteRequest,
};
