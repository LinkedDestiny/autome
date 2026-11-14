import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import type { KernelBinary } from './binary.ts';

const execute = promisify(execFile);
export async function kernelBuildId(binary: KernelBinary, { env = process.env, timeout = 5000 } = {}) {
  let identity: string;
  try {
    const { stdout } = await execute(binary.path, ['--version'], { timeout, env });
    identity = stdout.match(/\((\d{7}-\d{8}T\d{6}Z-[0-9a-f]{12}|unknown-.+-\d{8}T\d{6}Z)\)/)?.[1]
      ?? '标识未知：输出里没有构建标识';
  } catch (error) {
    const failure = error as NodeJS.ErrnoException & { killed?: boolean; signal?: string };
    const reason = failure.code === 'ENOENT' ? '文件不存在'
      : failure.killed ? '--version 超时' : `--version 退出码 ${failure.code ?? failure.signal ?? '未知'}`;
    identity = `标识未知：${reason}`;
  }
  return identity;
}

export async function describeKernel(binary: KernelBinary, options = {}) {
  const identity = await kernelBuildId(binary, options);
  return binary.source === 'cargo' ? `正在使用源码构建的内核：${binary.path}（${identity}）`
    : `内核：${[binary.path, ...binary.args].join(' ')}（${identity}，来源：${binary.source}）`;
}
