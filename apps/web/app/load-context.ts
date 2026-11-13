import { createContext, RouterContextProvider, type RouterContext } from 'react-router';
import type { GetLoadContextFunction } from '@react-router/express';
import type { createGateway } from '../server/gateway.ts';

import type { UploadService } from '../server/uploads.ts';
import type { openWebStore } from '../server/store.ts';
export type UnreadSource = Pick<ReturnType<typeof openWebStore>, 'unreadByTask'>;
export type KernelGateway = Pick<ReturnType<typeof createGateway>, 'read' | 'write'>;
export type WebServices = {
  gateway: KernelGateway | null;
  unread?: UnreadSource | null;
  uploads?: UploadService | null;
};

// Node, Vite SSR and the production bundle must use the same object key.
const key = Symbol.for('autome.web.services');
const registry = globalThis as typeof globalThis & { [key]: RouterContext<WebServices> | undefined };
export const servicesContext = registry[key] ??= createContext<WebServices>();

export function createLoadContext(services: WebServices) {
  return (() => {
    const context = new RouterContextProvider();
    context.set(servicesContext, services);
    return context;
  }) satisfies GetLoadContextFunction;
}
