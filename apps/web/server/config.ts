export const HOST = '127.0.0.1';

export function resolvePort(value: string | undefined): number {
  if (value === undefined) return 7420;
  if (!/^\d+$/.test(value) || Number(value) > 65535) {
    throw new Error('端口必须是 0–65535 之间的整数');
  }
  return Number(value);
}
