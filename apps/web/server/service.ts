import { mkdir, writeFile, rename, unlink, access } from 'node:fs/promises';
import { join, resolve, isAbsolute } from 'node:path';
import { execFileSync } from 'node:child_process';
import { resolveAutomeHome } from './home.ts';

export function agentsDirectory(env: NodeJS.ProcessEnv = process.env) {
  return env.AUTOME_LAUNCH_AGENTS_DIR ?? join(env.HOME ?? '', 'Library/LaunchAgents');
}
function xml(value: string) {
  return value.replace(/[&<>"']/g, ch => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&apos;' })[ch]!);
}
export async function service(action: string, { env = process.env, platform = process.platform, label = 'com.autome.web' }: { env?: NodeJS.ProcessEnv; platform?: string; label?: string } = {}) {
  if (platform !== 'darwin') throw new Error('launchd 自启动只支持 macOS');
  if (label !== 'com.autome.web') throw new Error('只允许管理 com.autome.web');
  if (!['install', 'uninstall'].includes(action)) throw new Error('用法：service install|uninstall');
  const directory = agentsDirectory(env);
  const file = join(directory, `${label}.plist`);
  const domain = `gui/${process.getuid!()}`;
  const run = (args: string[]) => execFileSync(env.AUTOME_LAUNCHCTL ?? '/bin/launchctl', args, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
  const bootout = () => {
    try { run(['bootout', `${domain}/${label}`]); }
    catch (error) {
      // launchctl reports an absent service as ESRCH (3) or EIO (5).
      const status = (error as { status?: number }).status;
      if (status !== 3 && status !== 5) throw error;
    }
  };
  if (action === 'uninstall') {
    try { await access(file); } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error;
      console.log('没有安装自启动'); return 0;
    }
    bootout(); await unlink(file); console.log('已卸载自启动'); return 0;
  }
  const core = env.AUTOME_BUNDLED_CORE;
  if (!core || !isAbsolute(core)) throw new Error('请通过 autome web service install 安装自启动（需要内核绝对路径）');
  const home = resolve(resolveAutomeHome(env));
  const log = join(home, 'web/logs/server.log');
  const vars = { AUTOME_NODE: process.execPath, AUTOME_WEB_ROOT: resolve(env.AUTOME_WEB_ROOT ?? resolve(import.meta.dirname, '..')), AUTOME_HOME: home };
  const content = `<?xml version="1.0" encoding="UTF-8"?>\n<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">\n<plist version="1.0"><dict>
<key>Label</key><string>${label}</string>
<key>ProgramArguments</key><array>${[core, 'web', 'serve'].map(v => `<string>${xml(v)}</string>`).join('')}</array>
<key>EnvironmentVariables</key><dict>${Object.entries(vars).map(([k, v]) => `<key>${k}</key><string>${xml(v)}</string>`).join('')}</dict>
<key>RunAtLoad</key><true/><key>KeepAlive</key><dict><key>SuccessfulExit</key><false/></dict>
<key>StandardOutPath</key><string>${xml(log)}</string><key>StandardErrorPath</key><string>${xml(log)}</string>
</dict></plist>\n`;
  await mkdir(directory, { recursive: true });
  await mkdir(join(home, 'web/logs'), { recursive: true });
  const temp = `${file}.${process.pid}.tmp`;
  try { await writeFile(temp, content, { mode: 0o600 }); await rename(temp, file); }
  finally { await unlink(temp).catch(error => { if (error.code !== 'ENOENT') throw error; }); }
  bootout(); run(['bootstrap', domain, file]); console.log('已安装自启动'); return 0;
}
