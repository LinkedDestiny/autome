'use strict';

// Pure validation for the `autome:read` IPC channel — technical design §15
// and the Electron security baseline it inherits: every IPC handler checks
// the sender origin, the webContents identity, the method against an
// allowlist, and the payload shape and size.
//
// Kept out of main.js so it can be unit-tested under plain `node --test`
// without an Electron runtime. main.js is the only caller and adds nothing
// this module does not already decide.
//
// The read/write split is load-bearing: a method on this list must be
// side-effect free in the core (see `READ_METHODS` in
// crates/automed/src/dispatch.rs, which this mirrors). If the two ever
// disagree, a "read" could mutate, and the separate write gate would be
// decoration.

const ALLOWED_READ_METHODS = Object.freeze([
  'project.list',
  'project.get',
  'project.onboarding.artefacts',
  'task.get',
  'task.changes',
  'session.log',
  'dashboard.get',
  'config.get',
  'config.validate',
  'env.get',
  'env.install_recipe',
  'skills.list',
  'events.since',
  'protocol.get',
  'protocol.versions',
  'protocol.eval',
]);

const TRUSTED_ORIGIN_PREFIX = 'autome://app/';

// Generous but finite. A read method's params are a handful of short string
// ids, never a document. This guards against a buggy or compromised renderer
// wedging an unbounded payload into the sidecar pipe.
const MAX_PAYLOAD_JSON_LENGTH = 4096;

function isTrustedSenderUrl(url) {
  return typeof url === 'string' && url.startsWith(TRUSTED_ORIGIN_PREFIX);
}

function isAllowedMethod(method) {
  return ALLOWED_READ_METHODS.includes(method);
}

// Validates a renderer-submitted read request before it is ever handed to the
// sidecar. Returns `{ ok: true, method, params }` or `{ ok: false, message }`;
// never throws, so main.js's handler maps straight to an error without its own
// try/catch.
function validateReadRequest(request) {
  if (request === null || typeof request !== 'object' || Array.isArray(request)) {
    return { ok: false, message: 'request must be a JSON object' };
  }
  const { method, params } = request;
  if (typeof method !== 'string' || !isAllowedMethod(method)) {
    return { ok: false, message: `method must be one of: ${ALLOWED_READ_METHODS.join(', ')}` };
  }
  const effectiveParams = params === undefined ? {} : params;
  if (
    effectiveParams === null ||
    typeof effectiveParams !== 'object' ||
    Array.isArray(effectiveParams)
  ) {
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
