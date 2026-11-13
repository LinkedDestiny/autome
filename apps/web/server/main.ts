import { createTaskEvents } from './task-events.ts';
import { createTaskDocs } from './task-docs.ts';
import { createMacTerminal, unavailableTerminal, type TerminalLauncher } from './terminal.ts';
import { createAdmin } from './admin.ts';
import { resolveUserHome } from './paths.ts';
import { createUploads } from './uploads.ts';
import { openWebStore } from './store.ts';
import { createNotifier } from './notifier.ts';
import { attachNotifier, createNotificationApi } from './notifier-api.ts';
import { createEventBus } from './eventbus.ts';
import { createLogtail } from './logtail.ts';
import { createWsUpgrade } from './eventbus-ws.ts';
import { CoreSupervisor } from './core/supervisor.ts';
import { describeKernel } from './core/identity.ts';
import { resolveAutomedBinary } from './core/binary.ts';
import { resolveAutomeHome } from './home.ts';
import { createGateway } from './gateway.ts';
import { createTicker } from './ticker.ts';
import { HOST, resolvePort } from './config.ts';
import { createHttpServer } from './http.ts';
import type { Mode } from './http.ts';

export async function startServer({ port, mode, home = resolveAutomeHome(process.env), userHome = resolveUserHome(process.env), binaryPath, binaryArgs, kernelEnv, terminal = unavailableTerminal }: { port: number; mode: Mode; home?: string; userHome?: string; terminal?: TerminalLauncher; binaryPath?: string; binaryArgs?: string[]; kernelEnv?: NodeJS.ProcessEnv }) {
  const binary = binaryPath ? { path: binaryPath, args: binaryArgs ?? [], source: 'env' as const } : resolveAutomedBinary();
  if (binaryArgs) binary.args = binaryArgs;
  console.error(await describeKernel(binary, { env: { ...process.env, ...kernelEnv, AUTOME_HOME: home } }));
  const store = openWebStore(home);
  const bus = createEventBus({ call: (method, params) => core.call(method, params), log: console.error });
  const logtail = createLogtail({ bus, call: (method, params) => core.call(method, params), log: console.error });
  const core = new CoreSupervisor({ home, binaryPath: binary.path, binaryArgs: binary.args, env: kernelEnv, log: console.error, onStatus: status => attached.onStatus(status) });
  const notifier = createNotifier({ store, call: (method, params) => core.call(method, params), publish: message => bus.broadcast(message), log: console.error });
  const attached = attachNotifier({ bus, notifier });
  const notifications = createNotificationApi({ store, publish: message => bus.broadcast(message) });
  const uploads = createUploads(home);
  const taskEvents = createTaskEvents({ call: (method, params) => core.call(method, params) });
  const taskDocs = createTaskDocs();
  const admin = createAdmin({ core, automeHome: home, userHome, terminal, publish: message => bus.broadcast(message) });
  const gateway = createGateway({ core, home, uploads, paths: admin.paths });
  admin.register(gateway);
  const { server, closeMiddleware, registerUpgrade } = await createHttpServer({ mode, gateway, notifications, store, uploads, admin, taskEvents, taskDocs }).catch(error => { store.close(); throw error; });
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
    store.close();
    throw error;
  }
  registerUpgrade('/ws', createWsUpgrade(bus));
  void core.start().catch(error => console.error(String(error)));
  const ticker = createTicker({ call: () => core.tick(), canTick: () => core.canTick(), isSlow: () => core.isSlow(), isBusy: () => core.isTickBusy(), log: console.error, onTick: report => { attached.onTick(); admin.onTick(report); } });
  return {
    server, gateway, core, registerUpgrade, bus, logtail,
    async close() {
      ticker.stop();
      notifier.close();
      await logtail.close();
      bus.close();
      taskEvents.close();
      await core.stop();
      store.close();
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
    const running = await startServer({ port, mode, terminal: createMacTerminal() });
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
