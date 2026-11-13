import { UPLOAD_MAX_BYTES, uploadLimitError } from '../../server/uploads-limits.ts';
import type { UploadInfo } from '../../server/uploads.ts';
export type UploadResult = { ok: true; payload: UploadInfo } | { ok: false; error: string };
export async function uploadFile(fetcher: typeof fetch, file: File): Promise<UploadResult> {
  if (file.size > UPLOAD_MAX_BYTES) return { ok: false, error: uploadLimitError(file.name) };
  try {
    const response = await fetcher('/api/uploads', {
      method: 'POST', headers: { 'Content-Type': 'application/octet-stream', 'X-Autome-Filename': encodeURIComponent(file.name) }, body: file,
    });
    return await response.json() as UploadResult;
  } catch { return { ok: false, error: '附件上传失败，请重试' }; }
}
