import { lstatSync, realpathSync, readdirSync, statSync } from 'node:fs';
import { dirname, basename, join, resolve, sep } from 'node:path';

type Checked = { ok: true; absolute: string; display: string; isHome: boolean } | { ok: false; message: string };
type Completion = { base: string; entries: { name: string; path: string }[]; truncated: boolean } | { error: string };
export function resolveUserHome(env: NodeJS.ProcessEnv): string {
  return env.HOME?.replace(/\/+$/, '') || (env.HOME === '/' ? '/' : '');
}
export function createPathGuard({ home }: { home: string }) {
  let realHome = '';
  try { if (home) realHome = realpathSync(home); } catch { /* Every request fails closed below. */ }
  const inside = (path: string) => path === realHome || path.startsWith(realHome === '/' ? '/' : realHome + sep);
  function check(input: unknown): Checked {
    if (!home) return { ok: false, message: '服务进程没有 HOME 环境变量，不能校验路径' };
    if (!realHome) return { ok: false, message: '无法读取家目录，不能校验路径' };
    if (typeof input !== 'string' || !input.length || input.length > 512 || input.includes('\0') || input.includes('..') || input.includes('\\')) {
      return { ok: false, message: '路径须为 1..512 字符，不能含 ..、空字符或反斜杠' };
    }
    if (input !== '~' && !input.startsWith('~/') && !input.startsWith('/')) return { ok: false, message: '路径必须是 ~/ 开头或绝对路径' };
    let ancestor = resolve(input.startsWith('~') ? home + input.slice(1) : input);
    const missing: string[] = [];
    try {
      for (;;) {
        try { lstatSync(ancestor); break; } catch (error) {
          if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error;
          missing.unshift(basename(ancestor));
          ancestor = dirname(ancestor);
        }
      }
      const absolute = join(realpathSync(ancestor), ...missing);
      if (!inside(absolute)) return { ok: false, message: `路径不在家目录内：${input}` };
      return { ok: true, absolute, display: absolute === realHome ? '~' : '~' + (realHome === '/' ? absolute : absolute.slice(realHome.length)), isHome: absolute === realHome };
    } catch { return { ok: false, message: `无法读取路径：${input}` }; }
  }
  function complete(input: unknown): Completion {
    const value = input === '' || input === '~' ? '~/' : input;
    const checked = check(value);
    if (!checked.ok) return { error: checked.message };
    const text = value as string;
    const slash = text.lastIndexOf('/');
    const prefix = text.endsWith('/') ? '' : text.slice(slash + 1);
    const parent = check(text.endsWith('/') ? text : text.slice(0, slash + 1));
    if (!parent.ok) return { error: parent.message };
    const base = parent.display.replace(/\/$/, '') + '/';
    const entries: { name: string; path: string }[] = [];
    try {
      for (const name of readdirSync(parent.absolute).sort()) {
        if (!name.startsWith(prefix) || (!prefix.startsWith('.') && name.startsWith('.'))) continue;
        try {
          const target = realpathSync(join(parent.absolute, name));
          if (!inside(target) || !statSync(target).isDirectory()) continue;
        } catch { continue; }
        if (entries.length === 50) return { base, entries, truncated: true };
        entries.push({ name, path: base + name });
      }
      return { base, entries, truncated: false };
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') return { base, entries, truncated: false };
      return { error: `无法读取目录：${base}` };
    }
  }
  return { check, complete };
}
export type PathGuard = ReturnType<typeof createPathGuard>;
