import { execFile as nodeExecFile } from 'node:child_process';
import { promisify } from 'node:util';
export type TerminalLauncher = (command: string) => Promise<void>;
export class TerminalUnavailable extends Error {}
export const unavailableTerminal: TerminalLauncher = async () => { throw new TerminalUnavailable('这台机器上打不开终端'); };
export function createMacTerminal({ platform = process.platform, execFile = promisify(nodeExecFile) }: {
  platform?: string; execFile?: (file: string, args: string[]) => Promise<unknown>;
} = {}): TerminalLauncher {
  return async command => {
    if (platform !== 'darwin') throw new TerminalUnavailable('这台机器上打不开终端');
    const escaped = command.replaceAll('\\', '\\\\').replaceAll('"', '\\"');
    try { await execFile('osascript', ['-e', 'tell application "Terminal"', '-e', 'activate', '-e', `do script "${escaped}"`, '-e', 'end tell']); }
    catch { throw new TerminalUnavailable('无法打开 Terminal.app'); }
  };
}
