function looksLikeAPath(value: string) {
  return (
    value.includes('/') ||
    value.includes('\\') ||
    value.startsWith('~') ||
    value.includes('..') ||
    value.includes('\u0000')
  );
}

const ALLOWED_READ_METHODS = Object.freeze([
  'project.list',
  'project.get',
  'project.onboarding.artefacts',
  'task.get',
  'task.changes',
  'session.log',
  'dashboard.get',
  'config.get',
  'env.get',
  'env.install_recipe',
  'skills.list',
  'protocol.get',
  'protocol.versions',
]);

const MAX_PAYLOAD_JSON_LENGTH = 4096;

function isAllowedMethod(method: string) {
  return ALLOWED_READ_METHODS.includes(method);
}

function validateReadRequest(request: unknown): { ok: false; message: string; method?: never; params?: never } | { ok: true; message?: never; method: string; params: object } {
  if (request === null || typeof request !== 'object' || Array.isArray(request)) {
    return { ok: false, message: '请求必须是 JSON 对象' };
  }
  const { method, params } = request as Record<string, unknown>;
  if (typeof method !== 'string' || !isAllowedMethod(method)) {
    return { ok: false, message: `method 必须是以下之一： ${ALLOWED_READ_METHODS.join(', ')}` };
  }
  const effectiveParams = params === undefined ? {} : params;
  if (
    effectiveParams === null ||
    typeof effectiveParams !== 'object' ||
    Array.isArray(effectiveParams)
  ) {
    return { ok: false, message: 'params 必须是 JSON 对象' };
  }
  let serialized;
  try {
    serialized = JSON.stringify(effectiveParams);
  } catch {
    return { ok: false, message: 'params 必须能序列化为 JSON' };
  }
  if (serialized.length > MAX_PAYLOAD_JSON_LENGTH) {
    return { ok: false, message: `params 超过 ${MAX_PAYLOAD_JSON_LENGTH} 字符上限` };
  }
  for (const value of Object.values(effectiveParams)) {
    if (typeof value === 'string' && looksLikeAPath(value)) {
      return { ok: false, message: 'params 不能包含路径' };
    }
    if (typeof value !== 'string' && typeof value !== 'number' && typeof value !== 'boolean') {
      return { ok: false, message: 'params 的值必须是字符串、数字或布尔值' };
    }
  }
  return { ok: true, method, params: effectiveParams };
}

export {
  ALLOWED_READ_METHODS,
  MAX_PAYLOAD_JSON_LENGTH,
  validateReadRequest,
};
