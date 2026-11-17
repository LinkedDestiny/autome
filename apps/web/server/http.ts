import { createOverviewSummary } from './overview-summary.ts';
import type { AdminServices } from './admin.ts';
import { UploadError, type UploadService } from './uploads.ts';
import { type TaskEventSource, type TaskDocReader, type UnreadSource, createLoadContext } from '../app/load-context.ts';
import type { createNotificationApi } from './notifier-api.ts';
import type { createGateway } from './gateway.ts';
import express from 'express';
import { securityMiddleware, installUpgradeGuard } from './security.ts';
import { createServer } from 'node:http';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { createRequestHandler } from '@react-router/express';
import type { ServerBuild } from 'react-router';

export type Mode = 'development' | 'production';
const root = resolve(import.meta.dirname, '..');

export async function createHttpServer({ mode, gateway, notifications, store, uploads, taskEvents, taskDocs, admin, drivers, identity, instanceStatus }: { drivers?: () => Promise<import('./drivers.ts').DriverStatus>; identity?: { instance: string; pid: number }; instanceStatus?: () => Promise<unknown>; admin?: AdminServices; taskEvents?: TaskEventSource; taskDocs?: TaskDocReader; mode: Mode; uploads?: UploadService; store?: UnreadSource; gateway?: ReturnType<typeof createGateway>; notifications?: ReturnType<typeof createNotificationApi> }) {
  const app = express();
  const overviewSummary = gateway ? createOverviewSummary((method, params) => gateway.core.call(method, params)) : undefined;
  const server = createServer(app);
  const port = () => {
    const address = server.address();
    return address && typeof address === 'object' ? address.port : 0;
  };
  app.use(securityMiddleware(port));
  if (uploads) app.post('/api/uploads', async (req, res) => {
    if (!req.is('application/octet-stream')) {
      res.status(415).json({ ok: false, error: '上传请求必须是文件原始字节' });
      return;
    }
    try {
      const payload = await uploads.receive({ name: req.get('X-Autome-Filename') ?? '', stream: req,
        contentLength: req.headers['content-length'] === undefined ? undefined : Number(req.headers['content-length']) });
      res.json({ ok: true, payload });
    } catch (error) {
      req.resume();
      res.status(error instanceof UploadError ? error.status : 400).json({ ok: false, error: error instanceof UploadError ? error.message : '附件上传失败，请重试' });
    }
  });
  let closeMiddleware = async () => {};
  app.get('/healthz', (_req, res) => { res.json({ ok: true, ...identity }); });
  if (instanceStatus) app.get('/api/instance', async (_req, res) => {
    try { res.json(await instanceStatus()); } catch (error) { res.status(503).json({ error: String(error) }); }
  });
  if (gateway || notifications || admin) {
    app.use('/api', express.json({ limit: '1mb' }));
    const jsonError: express.ErrorRequestHandler = (_error, _req, res, _next) => {
      void _next;
      res.status(400).json({ ok: false, error: '请求 JSON 无效或超过大小上限' });
    };
    app.use('/api', jsonError);
    if (admin) app.post('/api/paths', (req, res) => {
      const body = req.body;
      if (!body || typeof body !== 'object' || Array.isArray(body) || Object.keys(body).some(key => key !== 'input') || typeof body.input !== 'string') {
        res.status(400).json({ ok: false, error: '已拒绝：只接受 input 字符串' });
        return;
      }
      const result = admin.paths.complete(body.input);
      if ('error' in result) res.status(400).json({ ok: false, error: '已拒绝：' + result.error });
      else res.json({ ok: true, payload: result });
    });
    if (gateway) for (const channel of ['read', 'write'] as const) {
      app.post('/api/' + channel, async (req, res) => {
        const result = await gateway[channel](req.body);
        res.status(result.status).json(result.body);
      });
    }
  }
  if (notifications) {
    app.get('/api/notifications', (req, res) => {
      const result = notifications.list(req.query.limit);
      res.status(result.status).json(result.body);
    });
    app.post('/api/notifications/read', (req, res) => {
      const result = notifications.read(req.body);
      res.status(result.status).json(result.body);
    });
  }
  if (mode === 'development') {
    const { createServer: createViteServer } = await import('vite');
    const vite = await createViteServer({
      root,
      server: { middlewareMode: true, cors: false, hmr: { server } },
    });
    closeMiddleware = () => vite.close();
    app.use(vite.middlewares);
    app.use(createRequestHandler({
      build: () => vite.ssrLoadModule('virtual:react-router/server-build') as Promise<ServerBuild>,
      mode,
      getLoadContext: createLoadContext({ gateway: gateway ?? null, unread: store, uploads, taskEvents, taskDocs, admin, drivers, instanceStatus, overviewSummary }),
    }));
  } else {
    app.use('/assets', express.static(resolve(root, 'build/client/assets'), { immutable: true, maxAge: '1y' }));
    app.use(express.static(resolve(root, 'build/client'), { maxAge: '1h' }));
    const build = await import(pathToFileURL(resolve(root, 'build/server/index.js')).href);
    app.use(createRequestHandler({ build, mode, getLoadContext: createLoadContext({ gateway: gateway ?? null, unread: store, uploads, taskEvents, taskDocs, admin, drivers, instanceStatus, overviewSummary }) }));
  }
  const registerUpgrade = installUpgradeGuard(server, port);
  return { server, closeMiddleware, registerUpgrade };
}
