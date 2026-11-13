import { relative, resolve, isAbsolute } from 'node:path';
import { validateReadRequest } from './gates/read-gate.ts';
import { validateWriteRequest } from './gates/write-gate.ts';
import type { CoreSupervisor } from './core/supervisor.ts';

import type { PathGuard } from './paths.ts';

export class LocalOpError extends Error {
  status: number;
  constructor(status: number, message: string) { super(message); this.status = status; }
}

type Result = { status: number; body: { ok: boolean; payload?: unknown; error?: string; code?: unknown } };
type Handler = (params: Record<string, unknown>) => Promise<unknown>;
const localOps = new Set(['project.pick', 'project.confirm', 'open.path', 'open.terminal', 'env.install', 'env.login']);
const rejected = (message: string): Result => ({ status: 400, body: { ok: false, error: `已拒绝：${message}` } });

export function createGateway({ core, home, paths, uploads = { resolve: () => null } }: {
  core: Pick<CoreSupervisor, 'call' | 'restart'>; home: string;
  paths?: PathGuard;
  uploads?: { resolve: (id: string) => string | null };
}) {
  const handlers = new Map<string, Handler>();
  async function execute(action: () => Promise<unknown>): Promise<Result> {
    try { return { status: 200, body: { ok: true, payload: await action() } }; }
    catch (error) {
      if (error instanceof LocalOpError) return { status: error.status, body: { ok: false, error: `已拒绝：${error.message}` } };
      const failure = error as Error & { code?: unknown; timeout?: boolean };
      const status = failure.timeout ? 504 : failure.code !== undefined ? 409 : 503;
      const message = status === 503 && !failure.message.startsWith('内核未连接：') ? `内核未连接：${failure.message}` : failure.message;
      return { status,
        body: { ok: false, error: message, ...(failure.code !== undefined ? { code: failure.code } : {}) } };
    }
  }
  return {
    core,
    registerLocalOp(op: string, handler: Handler) { handlers.set(op, handler); },
    async read(request: unknown): Promise<Result> {
      const checked = validateReadRequest(request);
      if (!checked.ok) return rejected(checked.message);
      const params = { ...checked.params } as Record<string, unknown>;
      if (checked.method === 'project.probe' && paths) {
        const path = paths.check(params.path);
        if (!path.ok) return rejected(path.message);
        params.path = path.absolute;
      }
      return execute(() => core.call(checked.method, params));
    },
    async write(request: unknown): Promise<Result> {
      const checked = validateWriteRequest(request);
      if (!checked.ok) return rejected(checked.message);
      const { op } = checked;
      const params = { ...checked.params } as Record<string, unknown>;
      if (op === 'core.restart') return execute(() => core.restart());
      if (localOps.has(op)) {
        const handler = handlers.get(op);
        return handler ? execute(() => handler(params)) : { status: 501, body: { ok: false, error: `网页端还不支持「${op}」` } };
      }
      if (op === 'task.create') {
        const paths: string[] = [];
        for (const id of (params.attachments ?? []) as string[]) {
          const path = uploads.resolve(id);
          const child = path ? relative(resolve(home, 'web/uploads'), path) : '';
          if (!path || !isAbsolute(path) || !child || child === '..' || child.startsWith('../') || isAbsolute(child)) {
            return rejected(`附件 ${id} 不存在或已过期，请重新上传`);
          }
          paths.push(path);
        }
        params.attachments = paths;
        delete params.doc_refs;
      }
      return execute(() => core.call(op, params));
    },
  };
}
