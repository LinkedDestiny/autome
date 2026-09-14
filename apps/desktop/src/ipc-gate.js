'use strict';

// Pure validation for the `autome:read` IPC channel — plan §9.4 "每个 IPC
// handler 校验 sender origin、webContents、schema 和 payload 上限" and §5.9
// "Renderer 只能提交 plan/action ID，不能提交 shell、URL 或 argv". Kept out of
// main.js so it can be unit-tested under plain `node --test` without an
// Electron runtime — main.js is the only caller and adds nothing this
// module doesn't already decide.

const ALLOWED_READ_METHODS = Object.freeze([
  'project.list',
  'project.get',
  'task.list',
  'task.get',
  'queue.get',
]);

const TRUSTED_ORIGIN_PREFIX = 'autome://app/';

// Generous but finite — a read method's params are a handful of short
// string ids, never a document. Guards against a compromised/buggy
// renderer wedging an unbounded payload into the sidecar pipe.
const MAX_PAYLOAD_JSON_LENGTH = 4096;

function isTrustedSenderUrl(url) {
  return typeof url === 'string' && url.startsWith(TRUSTED_ORIGIN_PREFIX);
}

function isAllowedMethod(method) {
  return ALLOWED_READ_METHODS.includes(method);
}

// Validates the shape of a renderer-submitted read request before it is
// ever handed to the sidecar. Returns `{ ok: true, method, params }` or
// `{ ok: false, message }` — never throws, so main.js's handler can map
// straight to a Reply-shaped error without its own try/catch.
function validateReadRequest(request) {
  if (request === null || typeof request !== 'object' || Array.isArray(request)) {
    return { ok: false, message: 'request must be a JSON object' };
  }
  const { method, params } = request;
  if (typeof method !== 'string' || !isAllowedMethod(method)) {
    return { ok: false, message: `method must be one of: ${ALLOWED_READ_METHODS.join(', ')}` };
  }
  const effectiveParams = params === undefined ? {} : params;
  if (effectiveParams === null || typeof effectiveParams !== 'object' || Array.isArray(effectiveParams)) {
    return { ok: false, message: 'params must be a JSON object' };
  }
  let serialized;
  try {
    serialized = JSON.stringify(effectiveParams);
  } catch {
    return { ok: false, message: 'params must be JSON-serializable' };
  }
  if (serialized.length > MAX_PAYLOAD_JSON_LENGTH) {
    return { ok: false, message: `params exceed the ${MAX_PAYLOAD_JSON_LENGTH}-byte limit` };
  }
  for (const value of Object.values(effectiveParams)) {
    if (typeof value !== 'string' && typeof value !== 'number' && typeof value !== 'boolean') {
      return { ok: false, message: 'params values must be strings, numbers, or booleans' };
    }
  }
  return { ok: true, method, params: effectiveParams };
}

module.exports = {
  ALLOWED_READ_METHODS,
  TRUSTED_ORIGIN_PREFIX,
  MAX_PAYLOAD_JSON_LENGTH,
  isTrustedSenderUrl,
  isAllowedMethod,
  validateReadRequest,
};
