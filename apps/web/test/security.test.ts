import { createServer } from 'node:http';
import type { IncomingMessage } from 'node:http';
import type { Duplex } from 'node:stream';
import { readFileSync, readdirSync } from 'node:fs';
import { resolve } from 'node:path';
import type { Request, Response, NextFunction } from 'express';
import { expect, test, vi } from 'vitest';
import { acceptsHost, acceptsOrigin, securityMiddleware, installUpgradeGuard, HOST_ERROR, ORIGIN_ERROR } from '../server/security.ts';
import { createHttpServer } from '../server/http.ts';

const port = 7420;
const host = `localhost:${port}`;
const origin = `http://${host}`;
const badHosts = [undefined, '', 'localhost', 'localhost:7421', '[::1]:7420', 'localhost.evil.com:7420', 'evil.example:7420', ' localhost:7420', 'localhost:07420', ['localhost:7420']];
const badOrigins = [undefined, '', 'null', 'https://127.0.0.1:7420', 'http://localhost:7421', `${origin}/`, 'autome://app', 'http://localhost.evil.com:7420', 'http://LOCALHOST:7420', `${origin}/path`, [origin]];
function request(method = 'POST', path = '/api/write', headers: Record<string, unknown> = { host, origin }) {
  const res = { status: vi.fn().mockReturnThis(), json: vi.fn().mockReturnThis(), type: vi.fn().mockReturnThis(), send: vi.fn().mockReturnThis(), setHeader: vi.fn() };
  const next = vi.fn();
  securityMiddleware(() => port)({ method, path, url: path, headers } as Request, res as unknown as Response, next as NextFunction);
  return { res, next };
}
function socket() { return { write: vi.fn(), destroy: vi.fn() }; }
function upgrade(server: ReturnType<typeof createServer>, headers: Record<string, unknown>, url = '/ws?q=1') {
  const peer = socket();
  const req = { headers, url } as IncomingMessage;
  const head = Buffer.from('head');
  server.emit('upgrade', req, peer as unknown as Duplex, head);
  return { peer, req, head };
}

test('Host 接受两个本机地址及大写，端口来自实际监听值', () => {
  for (const p of [1, 7420, 65535]) {
    for (const h of ['127.0.0.1', 'localhost', 'LOCALHOST']) expect(acceptsHost(`${h}:${p}`, p)).toBe(true);
    expect(acceptsHost(`${host}`, p + 1)).toBe(false);
  }
});
test('Host 拒绝表包含缺失、端口、IPv6、后缀、空白和数组', () => {
  for (const h of badHosts) expect(acceptsHost(h, port), String(h)).toBe(false);
});
test('Origin 只接受两个可信源', () => {
  for (const h of ['127.0.0.1', 'localhost']) expect(acceptsOrigin(`http://${h}:${port}`, port)).toBe(true);
});
test('Origin 拒绝表包含空输入、协议、路径、端口和数组', () => {
  for (const o of badOrigins) expect(acceptsOrigin(o, port), String(o)).toBe(false);
});
test('GET HEAD 只查 Host，healthz 同样拒绝恶意 Host', () => {
  for (const method of ['GET', 'HEAD']) {
    for (const o of [undefined, 'https://evil.example']) expect(request(method, '/healthz', { host, origin: o }).next).toHaveBeenCalledOnce();
    const { res, next } = request(method, '/healthz', { host: 'evil.example:7420', origin });
    expect(res.status).toHaveBeenCalledWith(403);
    expect(next).not.toHaveBeenCalled();
  }
});
test('其它方法在 API 与页面均检查 Origin，拒绝格式按路径区分', () => {
  for (const method of ['POST', 'PUT', 'PATCH', 'DELETE', 'OPTIONS']) {
    for (const path of ['/api/write', '/']) {
      expect(request(method, path).next).toHaveBeenCalledOnce();
      for (const o of [undefined, 'https://evil.example']) {
        const { res, next } = request(method, path, { host, origin: o });
        expect(next).not.toHaveBeenCalled();
        expect(res.status).toHaveBeenCalledWith(403);
        if (path.startsWith('/api/')) expect(res.json).toHaveBeenCalledWith({ ok: false, error: ORIGIN_ERROR });
        else {
          expect(res.type).toHaveBeenCalledWith('text/plain; charset=utf-8');
          expect(res.send).toHaveBeenCalledWith(ORIGIN_ERROR);
        }
      }
    }
  }
});
test('无任何凭据的 POST 放行且不设置 Cookie', () => {
  const { res, next } = request();
  expect(next).toHaveBeenCalledOnce();
  expect(res.status).not.toHaveBeenCalled();
  expect(res.setHeader).not.toHaveBeenCalled();
});
test('查询参数不触发兑换跳转，Cookie 不能绕过 Origin', () => {
  const allowed = request('GET', '/?t=abc', { host });
  expect(allowed.next).toHaveBeenCalledOnce();
  expect(allowed.res.status).not.toHaveBeenCalled();
  const denied = request('POST', '/', { host, origin: 'https://evil.example', cookie: 'session=abc' });
  expect(denied.res.status).toHaveBeenCalledWith(403);
  expect(denied.next).not.toHaveBeenCalled();
});
test('拒绝响应均不添加 CORS 或 Cookie 头，Host 优先于 Origin', () => {
  for (const path of ['/api/write', '/']) {
    for (const headers of [{ host: 'evil', origin: 'evil' }, { host, origin: 'evil' }]) {
      const { res } = request('POST', path, headers);
      expect(res.status).toHaveBeenCalledWith(403);
      expect(res.setHeader).not.toHaveBeenCalled();
      if (path === '/api/write' && headers.host === 'evil') expect(res.json).toHaveBeenCalledWith({ ok: false, error: HOST_ERROR });
    }
  }
});
test('upgrade 的 Host 错误或 Origin 缺失错误都在转发前拒绝', () => {
  const server = createServer();
  const listener = vi.fn();
  server.on('upgrade', listener);
  const registered = vi.fn();
  installUpgradeGuard(server, () => port)('/ws', registered);
  for (const headers of [{ host: 'evil', origin }, { host }, { host, origin: 'evil' }]) {
    const { peer } = upgrade(server, headers);
    expect(peer.write).toHaveBeenCalledWith(expect.stringContaining('HTTP/1.1 403 Forbidden'));
    expect(peer.destroy).toHaveBeenCalledOnce();
  }
  expect(listener).not.toHaveBeenCalled();
  expect(registered).not.toHaveBeenCalled();
});
test('upgrade 转发既有监听器及路径处理器，无处理器时回 404', () => {
  for (const existing of [false, true]) {
    for (const registered of [false, true]) {
      const server = createServer();
      const listener = vi.fn();
      if (existing) server.on('upgrade', listener);
      const register = installUpgradeGuard(server, () => port);
      const handler = vi.fn();
      if (registered) register('/ws', handler);
      expect(server.listenerCount('upgrade')).toBe(1);
      const { req, peer, head } = upgrade(server, { host, origin });
      expect(listener).toHaveBeenCalledTimes(Number(existing));
      expect(handler).toHaveBeenCalledTimes(Number(registered));
      if (existing) expect(listener).toHaveBeenCalledWith(req, peer, head);
      if (registered) expect(handler).toHaveBeenCalledWith(req, peer, head);
      if (!existing && !registered) {
        expect(peer.write).toHaveBeenCalledWith(expect.stringContaining('404 Not Found'));
        expect(peer.destroy).toHaveBeenCalledOnce();
      } else expect(peer.destroy).not.toHaveBeenCalled();
      if (!existing && registered) expect(upgrade(server, { host, origin }, '/other').peer.destroy).toHaveBeenCalledOnce();
    }
  }
});
test('开发服务只保留一个 upgrade 守卫且关闭 Vite CORS', async () => {
  const running = await createHttpServer({ mode: 'development' });
  try {
    expect(running.server.listenerCount('upgrade')).toBe(1);
    expect(readFileSync(new URL('../server/http.ts', import.meta.url), 'utf8')).toMatch(/cors:\s*false/);
  } finally { await running.closeMiddleware(); }
});
test('所有拒绝原因都有中文', () => {
  for (const reason of [HOST_ERROR, ORIGIN_ERROR]) expect(reason).toMatch(/[\u4e00-\u9fff]/);
});
test('服务端源码不存在 Cookie、令牌或兑换逻辑', () => {
  const root = resolve(import.meta.dirname, '../server');
  for (const file of readdirSync(root, { recursive: true }) as string[]) {
    if (file.endsWith('.ts')) expect(readFileSync(resolve(root, file), 'utf8'), file).not.toMatch(/set-cookie|\.cookie\(|req\.cookies|timingSafeEqual|web\/token|[?&]t=/i);
  }
});
