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
  'project.probe',
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
  'protocol.get',
  'protocol.versions',
]);

const WITHHELD_READ_METHODS = Object.freeze([
  'events.since', // 浏览器经 WebSocket 收事件，账本拉取留给服务端。
  'env.detect', // 有副作用，保留显式写入口及其 Origin 检查。
]);
const HOME_PATH_PARAMS: Record<string, string> = { 'project.probe': 'path' };

function isHomeRelativePath(value: string) {
  return value === '~' || (value.startsWith('~/') && !value.includes('\\') &&
    !value.includes('\u0000') && !value.split('/').includes('..'));
}

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
  const homePathKey = HOME_PATH_PARAMS[method];
  if (homePathKey) {
    const value = (effectiveParams as Record<string, unknown>)[homePathKey];
    if (typeof value !== 'string' || !isHomeRelativePath(value)) {
      return { ok: false, message: 'project.probe 的 path 只能是 ~ 或 ~/ 开头的家目录路径' };
    }
  }
  for (const [key, value] of Object.entries(effectiveParams)) {
    if (key === homePathKey) continue;
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
  WITHHELD_READ_METHODS,
  isHomeRelativePath,
  MAX_PAYLOAD_JSON_LENGTH,
  validateReadRequest,
};
