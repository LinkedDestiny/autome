import { WebSocketServer } from 'ws';
import type { createEventBus } from './eventbus.ts';
import type { UpgradeHandler } from './security.ts';

export function createWsUpgrade(bus: ReturnType<typeof createEventBus>): UpgradeHandler {
  const server = new WebSocketServer({ noServer: true, maxPayload: 64 * 1024 });
  return (request, socket, head) => {
    server.handleUpgrade(request, socket, head, ws => {
      const disconnect = bus.connect(ws);
      ws.on('close', disconnect);
      ws.on('error', disconnect);
    });
  };
}
