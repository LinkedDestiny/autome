import { randomUUID } from 'node:crypto';
import { lstatSync, readdirSync } from 'node:fs';
import { mkdir, open, rename, rm } from 'node:fs/promises';
import { extname, join, resolve } from 'node:path';
import type { Readable } from 'node:stream';
import { UPLOAD_MAX_BYTES, uploadLimitError } from './uploads-limits.ts';

export class UploadError extends Error {
  status: number;
  constructor(status: number, message: string) { super(message); this.status = status; }
}
export type UploadInfo = { id: string; name: string; size: number };
export type UploadService = ReturnType<typeof createUploads>;
const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
function truncate(value: string, bytes: number) {
  let result = '';
  for (const char of value) {
    if (Buffer.byteLength(result + char) > bytes) break;
    result += char;
  }
  return result;
}
function filename(encoded: string) {
  let name: string;
  try { name = decodeURIComponent(encoded).split(/[/\\]/).pop()!; }
  catch { throw new UploadError(400, '文件名无效'); }
  name = Array.from(name).filter(char => char.charCodeAt(0) > 31 && char.charCodeAt(0) !== 127).join('').replace(/^\.+/, '');
  if (!name) throw new UploadError(400, '文件名无效');
  if (Buffer.byteLength(name) > 200) {
    const extension = extname(name);
    if (Buffer.byteLength(extension) >= 200) throw new UploadError(400, '文件名扩展名过长');
    name = truncate(name.slice(0, name.length - extension.length), 200 - Buffer.byteLength(extension)) + extension;
  }
  return name;
}
export function createUploads(home: string) {
  const root = resolve(home, 'web', 'uploads');
  function resolveUpload(id: string): string | null {
    if (!uuid.test(id)) return null;
    try {
      const dir = join(root, id);
      if (!lstatSync(dir).isDirectory()) return null;
      const entries = readdirSync(dir, { withFileTypes: true });
      return entries.length === 1 && entries[0].isFile() ? join(dir, entries[0].name) : null;
    } catch { return null; }
  }
  return {
    async receive({ name: encoded, stream, contentLength }: { name: string; stream: Readable; contentLength?: number }): Promise<UploadInfo> {
      const name = filename(encoded);
      if (contentLength !== undefined && contentLength > UPLOAD_MAX_BYTES) throw new UploadError(413, uploadLimitError(name));
      const id = randomUUID(), partial = join(root, `.partial-${id}`);
      let size = 0, created = false;
      try {
        await mkdir(partial, { recursive: true });
        created = true;
        const file = await open(join(partial, name), 'wx');
        try {
          // Do not destroy IncomingMessage on limit rejection: HTTP must still send its 413.
          for await (const chunk of stream.iterator({ destroyOnReturn: false })) {
            const bytes = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
            size += bytes.length;
            if (size > UPLOAD_MAX_BYTES) throw new UploadError(413, uploadLimitError(name));
            await file.writeFile(bytes);
          }
        } finally { await file.close(); }
        await rename(partial, join(root, id));
        return { id, name, size };
      } catch (error) {
        if (created) await rm(partial, { recursive: true, force: true });
        if (error instanceof UploadError) throw error;
        throw new UploadError(400, '附件上传失败，请重试');
      }
    },
    resolve: resolveUpload,
    describe(id: string): Omit<UploadInfo, 'id'> | null {
      const path = resolveUpload(id);
      if (!path) return null;
      try { return { name: path.split('/').pop()!, size: lstatSync(path).size }; }
      catch { return null; }
    },
  };
}
