import express from 'express';
import { securityMiddleware, installUpgradeGuard } from './security.ts';
import { createServer } from 'node:http';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { createRequestHandler } from '@react-router/express';
import type { ServerBuild } from 'react-router';

export type Mode = 'development' | 'production';
const root = resolve(import.meta.dirname, '..');

export async function createHttpServer({ mode }: { mode: Mode }) {
  const app = express();
  const server = createServer(app);
  const port = () => {
    const address = server.address();
    return address && typeof address === 'object' ? address.port : 0;
  };
  app.use(securityMiddleware(port));
  let closeMiddleware = async () => {};
  app.get('/healthz', (_req, res) => { res.json({ ok: true }); });
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
    }));
  } else {
    app.use('/assets', express.static(resolve(root, 'build/client/assets'), { immutable: true, maxAge: '1y' }));
    app.use(express.static(resolve(root, 'build/client'), { maxAge: '1h' }));
    const build = await import(pathToFileURL(resolve(root, 'build/server/index.js')).href);
    app.use(createRequestHandler({ build, mode }));
  }
  const registerUpgrade = installUpgradeGuard(server, port);
  return { server, closeMiddleware, registerUpgrade };
}
