import { lstat, open } from 'node:fs/promises';
import { constants } from 'node:fs';
import { looksLikeAPath } from './gates/write-gate.ts';
export const DOC_LIMIT_BYTES = 200 * 1024;
export type TaskDocument = { name: string; absolute: string; label?: string; size?: number; path?: string };
export function pickDocument(documents: TaskDocument[], name: string) {
  if (looksLikeAPath(name)) return { ok: false as const, message: '已拒绝：文档名不能是路径' };
  const entry = documents.find(document => document.name === name);
  return entry ? { ok: true as const, entry } : { ok: false as const, message: `这个任务没有名为 ${name} 的文档` };
}
export function createTaskDocs() {
  return {
    async read(absolute: string, maxBytes: number) {
      try {
        if (!Number.isSafeInteger(maxBytes) || maxBytes <= 0) throw new Error('文档读取上限必须是正整数');
        if (!(await lstat(absolute)).isFile()) throw new Error('文档不是普通文件');
        const file = await open(absolute, constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK);
        try {
          const stat = await file.stat();
          if (!stat.isFile()) throw new Error('文档不是普通文件');
          const buffer = Buffer.alloc(Math.min(stat.size, maxBytes));
          let offset = 0;
          while (offset < buffer.length) {
            const { bytesRead } = await file.read(buffer, offset, buffer.length - offset, offset);
            if (!bytesRead) break;
            offset += bytesRead;
          }
          const truncated = stat.size > maxBytes;
          const bytes = buffer.subarray(0, offset);
          const newline = bytes.lastIndexOf(10);
          const text = new TextDecoder('utf-8', { fatal: false }).decode(truncated ? bytes.subarray(0, newline < 0 ? offset : newline + 1) : bytes, { stream: truncated });
          return { text, size: stat.size, truncated };
        } finally { await file.close(); }
      } catch (error) {
        return { error: (error as NodeJS.ErrnoException).code === 'ENOENT'
          ? '文档已不在（可能刚被合并清理），刷新后重试' : error instanceof Error ? error.message : String(error) };
      }
    },
  };
}
