import type { RequestHandler } from 'express';
import type { IncomingMessage, Server } from 'node:http';
import type { Duplex } from 'node:stream';

export const HOST_ERROR = 'Host 不是本机地址，已拒绝';
export const ORIGIN_ERROR = '请求来源不是本服务，已拒绝';
type Header = string | string[] | undefined;

export function acceptsHost(host: Header, port: number): boolean {
  return typeof host === 'string' &&
    [ `127.0.0.1:${port}`, `localhost:${port}` ].includes(host.toLowerCase());
}

export function acceptsOrigin(origin: Header, port: number): boolean {
  return typeof origin === 'string' &&
    [ `http://127.0.0.1:${port}`, `http://localhost:${port}` ].includes(origin);
}

export function securityMiddleware(port: () => number): RequestHandler {
  return (req, res, next) => {
    const error = !acceptsHost(req.headers.host, port()) ? HOST_ERROR
      : req.method !== 'GET' && req.method !== 'HEAD' && !acceptsOrigin(req.headers.origin, port())
        ? ORIGIN_ERROR : null;
    if (!error) { next(); return; }
    res.status(403);
    if (req.path.startsWith('/api/')) res.json({ ok: false, error });
    else res.type('text/plain; charset=utf-8').send(error);
  };
}

export type UpgradeHandler = (req: IncomingMessage, socket: Duplex, head: Buffer) => void;

export function installUpgradeGuard(server: Server, port: () => number) {
  const listeners = server.listeners('upgrade') as UpgradeHandler[];
  const handlers = new Map<string, UpgradeHandler>();
  server.removeAllListeners('upgrade');
  server.on('upgrade', (req, socket, head) => {
    const error = !acceptsHost(req.headers.host, port()) ? HOST_ERROR
      : !acceptsOrigin(req.headers.origin, port()) ? ORIGIN_ERROR : null;
    const reject = (status: string, reason: string) => {
      socket.write(`HTTP/1.1 ${status}\r\nContent-Type: text/plain; charset=utf-8\r\nConnection: close\r\nContent-Length: ${Buffer.byteLength(reason)}\r\n\r\n${reason}`);
      socket.destroy();
    };
    if (error) { reject('403 Forbidden', error); return; }
    const handler = handlers.get((req.url ?? '').split('?')[0]);
    if (!handler && listeners.length === 0) { reject('404 Not Found', '未找到升级处理器'); return; }
    for (const listener of listeners) listener.call(server, req, socket, head);
    handler?.(req, socket, head);
  });
  return (path: string, handler: UpgradeHandler) => { handlers.set(path, handler); };
}
