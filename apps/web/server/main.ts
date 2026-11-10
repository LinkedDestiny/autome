import { CoreSupervisor } from './core/supervisor.ts';
import { resolveAutomedBinary } from './core/binary.ts';
import { resolveAutomeHome } from './home.ts';
import { createGateway } from './gateway.ts';
import { createTicker } from './ticker.ts';
import { HOST, resolvePort } from './config.ts';
import { createHttpServer } from './http.ts';
import type { Mode } from './http.ts';

export async function startServer({ port, mode, home = resolveAutomeHome(process.env), binaryPath = resolveAutomedBinary().path }: { port: number; mode: Mode; home?: string; binaryPath?: string }) {
  const core = new CoreSupervisor({ home, binaryPath, log: console.error });
  const gateway = createGateway({ core, home });
  const { server, closeMiddleware, registerUpgrade } = await createHttpServer({ mode, gateway });
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
  void core.start().catch(error => console.error(String(error)));
  const ticker = createTicker({ call: () => core.tick(), isConnected: () => core.isConnected(), isBusy: () => core.isTickBusy(), log: console.error });
  return {
    server, gateway, registerUpgrade,
    async close() {
      ticker.stop();
      await core.stop();
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
