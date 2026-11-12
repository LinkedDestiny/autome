import { createLoadContext } from '../app/load-context.ts';
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

export async function createHttpServer({ mode, gateway, notifications }: { mode: Mode; gateway?: ReturnType<typeof createGateway>; notifications?: ReturnType<typeof createNotificationApi> }) {
  const app = express();
  const server = createServer(app);
  const port = () => {
    const address = server.address();
    return address && typeof address === 'object' ? address.port : 0;
  };
  app.use(securityMiddleware(port));
  let closeMiddleware = async () => {};
  app.get('/healthz', (_req, res) => { res.json({ ok: true }); });
  if (gateway || notifications) {
    app.use('/api', express.json({ limit: '1mb' }));
    const jsonError: express.ErrorRequestHandler = (_error, _req, res, _next) => {
      void _next;
      res.status(400).json({ ok: false, error: '请求 JSON 无效或超过大小上限' });
    };
    app.use('/api', jsonError);
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
      getLoadContext: createLoadContext({ gateway: gateway ?? null }),
    }));
  } else {
    app.use('/assets', express.static(resolve(root, 'build/client/assets'), { immutable: true, maxAge: '1y' }));
    app.use(express.static(resolve(root, 'build/client'), { maxAge: '1h' }));
    const build = await import(pathToFileURL(resolve(root, 'build/server/index.js')).href);
    app.use(createRequestHandler({ build, mode, getLoadContext: createLoadContext({ gateway: gateway ?? null }) }));
  }
  const registerUpgrade = installUpgradeGuard(server, port);
  return { server, closeMiddleware, registerUpgrade };
}
