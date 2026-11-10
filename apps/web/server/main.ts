import { HOST, resolvePort } from './config.ts';
import { createHttpServer } from './http.ts';
import type { Mode } from './http.ts';

export async function startServer({ port, mode }: { port: number; mode: Mode }) {
  const { server, closeMiddleware } = await createHttpServer({ mode });
  try {
    await new Promise<void>((resolve, reject) => {
      server.once('error', reject);
      server.listen(port, HOST, () => {
        server.off('error', reject);
        resolve();
      });
    });
  } catch (error) {
    await closeMiddleware();
    throw error;
  }
  return {
    server,
    async close() {
      await closeMiddleware();
      await new Promise<void>((resolve, reject) => {
        server.close(error => error ? reject(error) : resolve());
        server.closeAllConnections();
      });
    },
  };
}

export async function runFromEnv() {
  let port: number | undefined;
  try {
    port = resolvePort(process.env.AUTOME_WEB_PORT);
    const mode = process.env.NODE_ENV === 'production' ? 'production' : 'development';
    const running = await startServer({ port, mode });
    const address = running.server.address();
    if (address && typeof address === 'object') {
      console.log(`autome-web 已启动：http://127.0.0.1:${address.port}（${mode === 'production' ? '生产' : '开发'}模式）`);
    }
    const shutdown = () => {
      process.off('SIGINT', shutdown);
      process.off('SIGTERM', shutdown);
      void running.close().catch(error => { console.error(error); process.exitCode = 1; });
    };
    process.on('SIGINT', shutdown);
    process.on('SIGTERM', shutdown);
  } catch (error) {
    console.error((error as NodeJS.ErrnoException).code === 'EADDRINUSE'
      ? `端口 ${port} 已被占用` : error instanceof Error ? error.message : error);
    process.exitCode = 1;
  }
}
