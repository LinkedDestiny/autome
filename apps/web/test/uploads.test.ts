import { mkdtempSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Readable } from 'node:stream';
import { randomUUID } from 'node:crypto';
import { afterEach, expect, test, vi } from 'vitest';
import { createUploads } from '../server/uploads.ts';
import { createGateway } from '../server/gateway.ts';
import { UPLOAD_MAX_BYTES } from '../server/uploads-limits.ts';
import { uploadFile } from '../app/components/upload-client.ts';
const homes: string[] = [];
function setup() {
  const home = mkdtempSync(join(tmpdir(), 'uploads-')); homes.push(home);
  return { home, root: join(home, 'web/uploads'), uploads: createUploads(home) };
}
const input = (name = '截图.txt', data = '内容') => ({ name: encodeURIComponent(name), stream: Readable.from([Buffer.from(data)]) });
afterEach(() => { for (const home of homes.splice(0)) rmSync(home, { recursive: true, force: true }); });
test('1 receive publishes lowercase UUID with exact bytes, including empty files', async () => {
  const { uploads, root } = setup();
  const result = await uploads.receive(input());
  expect(result.id).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/);
  expect(uploads.resolve(result.id)).toBe(join(root, result.id, '截图.txt'));
  expect(readFileSync(uploads.resolve(result.id)!, 'utf8')).toBe('内容');
  expect(readdirSync(root)).toEqual([result.id]);
  expect((await uploads.receive(input('empty', ''))).size).toBe(0);
});
test('2 real gateway translates upload id to the persisted file', async () => {
  const { uploads, home } = setup(); const result = await uploads.receive(input());
  const core = { call: vi.fn().mockResolvedValue({}), restart: vi.fn() };
  const gateway = createGateway({ core, home, uploads });
  expect((await gateway.write({ op: 'task.create', params: { attachments: [result.id] } })).status).toBe(200);
  expect(core.call).toHaveBeenCalledExactlyOnceWith('task.create', { attachments: [uploads.resolve(result.id)] });
});
test('3 declared and streamed over-limit cleanup; exact limit succeeds; client precheck', async () => {
  const { uploads, root } = setup();
  await expect(uploads.receive({ ...input(), contentLength: UPLOAD_MAX_BYTES + 1 })).rejects.toMatchObject({ status: 413 });
  expect(() => readdirSync(root)).toThrow();
  await expect(uploads.receive({ name: 'a', stream: Readable.from([Buffer.alloc(UPLOAD_MAX_BYTES), Buffer.alloc(1)]) })).rejects.toMatchObject({ status: 413 });
  expect(readdirSync(root)).toEqual([]);
  const exact = await uploads.receive({ name: 'a', stream: Readable.from([Buffer.alloc(UPLOAD_MAX_BYTES)]), contentLength: UPLOAD_MAX_BYTES });
  expect(exact.size).toBe(UPLOAD_MAX_BYTES);
  const fetcher = vi.fn<typeof fetch>();
  expect(await uploadFile(fetcher, new File([Buffer.alloc(UPLOAD_MAX_BYTES + 1)], 'a'))).toMatchObject({ ok: false, error: expect.stringContaining('上限') });
  expect(fetcher).not.toHaveBeenCalled();
});
test('4 clean filenames and preserve extension within UTF-8 byte boundary', async () => {
  const { uploads, root } = setup();
  for (const [name, expected] of [['../../etc/passwd', 'passwd'], ['C:\\tmp\\a.txt', 'a.txt'], ['.\u0000a\u007f.txt', 'a.txt'], ['...hidden', 'hidden'], ['a'.repeat(196) + '.txt', 'a'.repeat(196) + '.txt'], ['中'.repeat(90) + '.log', '中'.repeat(65) + '.log']]) {
    const result = await uploads.receive(input(name));
    expect(result.name).toBe(expected); expect(Buffer.byteLength(result.name)).toBeLessThanOrEqual(200);
    expect(uploads.resolve(result.id)).toBe(join(root, result.id, expected));
  }
  for (const name of ['', '.', '..']) await expect(uploads.receive(input(name))).rejects.toMatchObject({ status: 400 });
  await expect(uploads.receive({ ...input(), name: '%' })).rejects.toMatchObject({ status: 400 });
});
test('5 resolve rejects malformed ids and directories without exactly one regular file', async () => {
  const { uploads, root } = setup(); const result = await uploads.receive(input());
  for (const id of [result.id.toUpperCase(), '../escape', randomUUID(), '.partial-' + result.id, '']) expect(uploads.resolve(id)).toBeNull();
  const empty = randomUUID(); mkdirSync(join(root, empty)); expect(uploads.resolve(empty)).toBeNull();
  writeFileSync(join(root, result.id, 'second'), 'x'); expect(uploads.resolve(result.id)).toBeNull();
  mkdirSync(join(root, empty, 'directory')); expect(uploads.resolve(empty)).toBeNull();
});
test('6 partial state is invisible, stream errors and disk errors clean up', async () => {
  const { uploads, root, home } = setup();
  async function* broken() {
    yield Buffer.from('first');
    const partial = readdirSync(root)[0]; expect(partial).toMatch(/^\.partial-/);
    expect(uploads.resolve(partial)).toBeNull(); expect(uploads.resolve(partial.replace('.partial-', ''))).toBeNull();
    throw Error('read failed');
  }
  await expect(uploads.receive({ name: 'a', stream: Readable.from(broken()) })).rejects.toMatchObject({ status: 400 });
  expect(readdirSync(root)).toEqual([]);
  const bad = join(home, 'file'); writeFileSync(bad, 'x');
  await expect(createUploads(bad).receive(input())).rejects.toMatchObject({ status: 400 });
  expect(createUploads(bad).describe(randomUUID())).toBeNull();
});
test('7 describe returns persisted name/size and null for unknown id', async () => {
  const { uploads } = setup(); const result = await uploads.receive(input());
  expect(uploads.describe(result.id)).toEqual({ name: '截图.txt', size: Buffer.byteLength('内容') });
  expect(uploads.describe(randomUUID())).toBeNull();
});
test('8 byte and file limits have a single production source', () => {
  const base = join(import.meta.dirname, '..');
  const sources = ['app', 'server'].flatMap(dir => readdirSync(join(base, dir), { recursive: true }).map(String).filter(p => /\.(ts|tsx)$/.test(p)).map(p => join(dir, p)));
  expect(sources.filter(p => /10\s*\*\s*1024\s*\*\s*1024/.test(readFileSync(join(base, p), 'utf8')))).toEqual(['server/uploads-limits.ts']);
  expect(sources.filter(p => /UPLOAD_MAX_FILES\s*=/.test(readFileSync(join(base, p), 'utf8')))).toEqual(['server/uploads-limits.ts']);
});
