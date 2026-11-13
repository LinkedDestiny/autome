export const UPLOAD_MAX_BYTES = 10 * 1024 * 1024;
export const UPLOAD_MAX_FILES = 10;
export const uploadLimitError = (name: string) => `附件「${name}」超过单个 ${UPLOAD_MAX_BYTES / 1024 / 1024} MB 的上限`;
