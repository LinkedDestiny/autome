import { execFile } from 'node:child_process';
import { mkdtempSync, readdirSync, readFileSync, rmSync } from 'node:fs';
import { request } from 'node:http';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { promisify } from 'node:util';
import { expect, test, vi } from 'vitest';
import { createHttpServer } from '../server/http.ts';
import { startServer } from '../server/main.ts';
import { createUploads } from '../server/uploads.ts';
import { createGateway } from '../server/gateway.ts';
import { UPLOAD_MAX_BYTES } from '../server/uploads-limits.ts';
import { uploadFile } from '../app/components/upload-client.ts';

async function withServer(run: (url: string, home: string) => Promise<void>) {
  const home = mkdtempSync(join(tmpdir(), 'upload-http-'));
  const core = { call: vi.fn().mockResolvedValue({ accepted: true }), restart: vi.fn() };
  const uploads = createUploads(home), gateway = createGateway({ core, home, uploads });
  const server = await createHttpServer({ mode: 'development', gateway, uploads });
  try {
    await new Promise<void>((yes, no) => { server.server.once('error', no); server.server.listen(0, '127.0.0.1', yes); });
    const address = server.server.address(); if (!address || typeof address === 'string') throw Error('no address');
    await run(`http://127.0.0.1:${address.port}`, home);
  } finally {
    await server.closeMiddleware();
    await new Promise<void>(yes => { server.server.close(() => yes()); server.server.closeAllConnections(); });
    rmSync(home, { recursive: true, force: true });
  }
}
const headers = (url: string) => ({ Origin: url, 'Content-Type': 'application/octet-stream', 'X-Autome-Filename': encodeURIComponent('截图.txt') });
const dirs = (home: string) => { try { return readdirSync(join(home, 'web/uploads')); } catch { return []; } };
function raw(url: string, extra: Record<string, string>, chunks: Buffer[]) {
  return new Promise<{ status: number; body: { ok: boolean; error: string } }>((yes, no) => {
    const req = request(url + '/api/uploads', { method: 'POST', headers: { ...headers(url), ...extra } }, res => {
      let body = ''; res.setEncoding('utf8'); res.on('data', chunk => { body += chunk; });
      res.on('end', () => { try { yes({ status: res.statusCode!, body: JSON.parse(body) }); } catch (error) { no(error); } });
    });
    req.on('error', no); req.setTimeout(10_000, () => req.destroy(Error('upload timeout')));
    for (const chunk of chunks) req.write(chunk);
    req.end();
  });
}
test('1 upload endpoint returns id/name/size and exact persisted bytes', () => withServer(async (url, home) => {
  const response = await fetch(url + '/api/uploads', { method: 'POST', headers: headers(url), body: '内容' });
  expect(response.status).toBe(200);
  const body = await response.json(); expect(body).toEqual({ ok: true, payload: { id: expect.any(String), name: '截图.txt', size: 6 } });
  expect(readFileSync(createUploads(home).resolve(body.payload.id)!, 'utf8')).toBe('内容');
}));
test('2 wrong Origin and Host reject before any disk writes', () => withServer(async (url, home) => {
  for (const extra of [{ Origin: 'http://evil.test' }, { Host: 'evil.test' }] as Record<string, string>[] ) {
    expect((await raw(url, extra, [Buffer.from('x')])).status).toBe(403);
    expect(dirs(home)).toEqual([]);
  }
}));
test('3 declared oversize immediately returns Chinese 413 without reading full body', () => withServer(async (url, home) => {
  const response = await raw(url, { 'Content-Length': String(UPLOAD_MAX_BYTES + 1) }, []);
  expect(response.status).toBe(413); expect(response.body).toMatchObject({ ok: false, error: expect.stringContaining('超过单个') });
  expect(dirs(home)).toEqual([]);
}));
test('4 chunked oversize returns 413 and removes partial data', () => withServer(async (url, home) => {
  const response = await raw(url, { 'Transfer-Encoding': 'chunked' }, [Buffer.alloc(UPLOAD_MAX_BYTES), Buffer.alloc(1)]);
  expect(response.status).toBe(413); expect(dirs(home)).toEqual([]);
}));
test('5 JSON upload rejected with 415 while write JSON parsing remains intact', () => withServer(async (url) => {
  const jsonHeaders = { Origin: url, 'Content-Type': 'application/json' };
  expect((await fetch(url + '/api/uploads', { method: 'POST', headers: jsonHeaders, body: '{}' })).status).toBe(415);
  const response = await fetch(url + '/api/write', { method: 'POST', headers: jsonHeaders, body: JSON.stringify({ op: 'config.set_loop', params: { parallel: 2 } }) });
  expect(response.status).toBe(200); expect(await response.json()).toEqual({ ok: true, payload: { accepted: true } });
  const rejected = await uploadFile((path, init) => fetch(url + path, { ...init, headers: { ...init?.headers, Origin: 'http://evil.test' } }), new File(['x'], 'a'));
  expect(rejected).toEqual({ ok: false, error: '请求来源不是本服务，已拒绝' });
}));
test('6 real startServer wiring uploads through gateway creates a real draft', async () => {
  const started = performance.now();
  const root = mkdtempSync(join(tmpdir(), 'upload-real-')), home = join(root, 'home');
  const wt = resolve(import.meta.dirname, '../../..');
  let running: Awaited<ReturnType<typeof startServer>> | undefined;
  try {
    await promisify(execFile)('sh', [join(wt, 'apps/web/test/fixtures/w6-project.sh'), root, wt], { timeout: 30_000 });
    running = await startServer({ port: 0, mode: 'development', home, binaryPath: join(root, 'core-wrapper') });
    const address = running.server.address(); if (!address || typeof address === 'string') throw Error('no address');
    const url = `http://127.0.0.1:${address.port}`;
    await vi.waitFor(async () => expect((await running!.gateway.read({ method: 'project.list' })).status).toBe(200), { timeout: 20_000 });
    const projects = (await running.gateway.read({ method: 'project.list' })).body.payload as { projects: { project: { id: string } }[] };
    const project_id = projects.projects[0].project.id;
    const uploaded = await uploadFile((path, init) => fetch(url + path, { ...init, headers: { ...init?.headers, Origin: url } }), new File(['真实附件'], '线索.txt'));
    expect(uploaded.ok).toBe(true); if (!uploaded.ok) throw Error(uploaded.error);
    const response = await fetch(url + '/api/write', { method: 'POST', headers: { Origin: url, 'Content-Type': 'application/json' }, body: JSON.stringify({ op: 'task.create', params: { project_id, brief: '# 上传接线\n正文', kind: 'feature', draft: true, attachments: [uploaded.payload.id] } }) });
    expect(response.status).toBe(200);
    const created = await response.json(); expect(created.ok).toBe(true);
    const project = (await running.gateway.read({ method: 'project.get', params: { project_id } })).body.payload as { tasks: { title: string; state: { state: string } }[] };
    expect(project.tasks).toHaveLength(1); expect(project.tasks[0]).toMatchObject({ title: '上传接线', state: { state: 'draft' } });
    expect(createUploads(home).describe(uploaded.payload.id)).toEqual({ name: '线索.txt', size: 12 });
  } finally {
    await running?.close(); rmSync(root, { recursive: true, force: true });
    process.stdout.write(`W6 upload real setup/tests/cleanup ${Math.ceil(performance.now() - started)} ms\n`);
  }
}, 120_000);
